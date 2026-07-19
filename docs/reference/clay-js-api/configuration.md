# Clay Configuration System

Clay configuration is JavaScript loaded from `~/.config/clay/init.js`. The configuration system is part of the Clay JS API surface: every configurable option, command binding, and behavior-changing setting should be represented by a documented Clay JS API and included in the Markdown documentation registry.

## Configuration Entry Point

- Default file: `~/.config/clay/init.js`
- The file is loaded by Clay's server-side JavaScript runtime during server startup or explicit configuration reload work.
- Phase 13 runtime-backs this entry point on the server: it evaluates supported local configuration JavaScript through documented `clay:*` facades while still never executing JavaScript in the Rust client.
- `init.js` may load other local configuration files through [`loadConfigurationModule`](configuration/load-configuration-module.md) so users can keep settings modular.
- App/help/agent surfaces can inspect the documented entry point shape through [`getConfigurationState`](configuration/get-configuration-state.md) once runtime state exists.
- Configuration code must use documented Clay JS APIs instead of raw ops or Rust internals.

## Configuration as Clay JS APIs

Each configuration option is exposed as a Clay JS API. That means it must have:

- A stable `id` and JS facade export.
- A searchable `user_facing_name`.
- `key_bindings`, using an empty list when no default binding exists.
- `custom_properties`, listing every behavior-changing setting the API accepts.
- Markdown documentation linked from `docs/index.md` when the API becomes a public registry surface.
- Generated registry and lookup coverage for public registry surfaces.
- Security and authority notes.

## Phase 18.16.5 typography configuration

[`clay.theme.setTypography`](theme/set-typography.md) atomically configures user-owned monospace, proportional, and UI family stacks and logical-pixel sizes. No call is required for defaults.

```js
import { setTypography } from "clay:theme";

setTypography({
  monospace: { families: ["JetBrains Mono", "monospace"], size: 16 },
  proportional: { families: ["Inter", "sans-serif"], size: 17 },
  ui: { families: ["system-ui"], size: 13 },
});
```

All three profiles are required and validated before replacement. Failed startup/reload evaluation preserves the previous complete state; removing the call and successfully reloading restores defaults. `init.js` may place the call in a local module loaded through `loadConfigurationModule`. There are no independent profile setters or hidden JSON/TOML keys.

Configuration runs outside interaction hot paths. One changed complete value produces one bounded client installation; paint/input/layout consume cached profiles. Clay does not validate installed fonts on the server, open/fetch/download fonts, or grant filesystem, network, shell, package, extension, raw-op, or client-side JavaScript authority. Packages select semantic roles only and cannot override concrete user families or sizes.

## Phase 18.17 range diagnostics configuration review

Phase 18.17 reviewed range diagnostics and syntax-error highlighting and did **not** promote a new user-facing diagnostic toggle, squiggle geometry setting, per-severity preference, or `clay:configuration` API. Default outcome: syntax-error publication follows the active syntax engine; severity colors come from the active theme.

User-visible configuration reuses existing Clay JS APIs:

```js
import { setTheme } from "clay:theme";
import { setSyntaxEnginePreference } from "clay:syntax";

setTheme("@clay/theme-gruvbox-material-dark");
// optional: force parser tier that produces syntax diagnostics
setSyntaxEnginePreference("rust", "wasm");
```

[`clay.theme.setTheme`](theme/set-theme.md) selects a first-party theme whose `textStyles` already include `diagnosticError`, `diagnosticWarning`, and `diagnosticInfo`. [`clay.syntax.setSyntaxEnginePreference`](syntax/set-syntax-engine-preference.md) selects the parser tier; no separate diagnostics preference exists. [`clay.diagnostics.serverPublishDiagnostics`](diagnostics/server-publish-diagnostics.md) is a package publication API gated by `render-decorations`, not an `init.js` user setting.

Compiled budgets (`DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`, `DIAGNOSTIC_MAX_SPANS_PER_SET`, `DIAGNOSTIC_CACHE_BUDGET_BYTES`, and related field caps) and Clay-owned squiggle amplitude/period/stroke constants are security/performance boundaries, not hidden `init.js` keys. Hidden/ad hoc keys rejected by policy include `diagnostics.enabled`, `diagnostics.enable`, `diagnostics.squiggleWidth`, `diagnostics.amplitude`, `diagnostics.severity`, `syntaxError.highlight`, `treeSitter.showErrors`, and parallel JSON/TOML diagnostic preference blobs.

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work only. Ordinary keypress, paint, layout, scroll, pointer, text-event handling, edit acknowledgement, parse-result publication, and diagnostic/decoration rendering paths do not execute user configuration JavaScript or recompute diagnostic preferences. This review grants no parser, filesystem, network, shell, LSP, AI, package-manager, WASM, raw-op, native-widget, client-side JavaScript, CSS, or client-render authority.

## Phase 17 package and mode configuration review

Phase 17 reviewed package loading, mode selection, decoration transport, parse coordination, and package-owned SDUI contributions for concrete user-visible settings.

- Package enable/disable remains a privileged package service or CLI operation, not `init.js` configuration and not a side effect of package options.
- `clay.configuration.setPackageOption`, `clay.configuration.setModePreference`, `clay.configuration.setDecorationTheme`, and `clay.configuration.setParsePolicy` are preserved as planned `clay:configuration` facade exports and inventory entries. Their inventory records include custom properties, hot-path policy, permission/security notes, and planned op paths, but they are not linked as public registry docs until server-side validators and concrete behavior-changing settings are promoted.
- Phase 17 did not introduce concrete user-facing SDUI panel visibility or layout settings. Package-owned SDUI region/layout data remains inert package contribution metadata validated at enable/load time.
- `clay:sdui.queryUiState` remains deferred. `SduiObservableSnapshot` and `SduiStatusObservation` stay internal observability/test infrastructure until a package-tooling, help, or agent workflow requires a public live-UI query API with full docs, registry, permissions/privacy notes, and tests.

## Phase 18.2/18.3/18.4 shell/layout and package UI configuration review

Compatibility summary for existing guards: Phase 18.2/18.3 shell/layout and package UI configuration review; Phase 18.2 does **not** promote any new runtime-backed or user-visible shell/layout configuration API; Phase 18.3 promotes package UI declaration APIs; Phase 18.3 promotes `clay.ui.serverRegisterThemeToken` to a runtime-backed package declaration API; Phase 18.3 does not promote user-visible panel visibility, default-slot, component-style, theme-token override, or layout behavior configuration APIs; `clay.ui.serverSetLayoutOverride` is the planned `PackageLayoutOverride` surface; `clay.configuration.setPackageOption` remains the planned package-owned option surface.

Phase 18.1 defined the shell/layout architecture contract, and Phase 18.2 implements internal Rust shell layout state for `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, the `ClayShellWidget` root, inert local layout updates, and structural shell observability. Phase 18.2 does **not** promote any new runtime-backed or user-visible shell/layout configuration API. Phase 18.3 promotes package UI declaration APIs for panels, components, overlays, and theme tokens. Historical Phase 18.3 status: Phase 18.3 promotes package UI declaration APIs but does not promote user-visible panel visibility, default-slot, component-style, theme-token override, or layout behavior configuration APIs; those surfaces were not user-visible override APIs and `clay.ui.serverSetLayoutOverride` and `clay.configuration.setPackageOption` stay non-registry-public inventory rows in that phase. Phase 18.4 promotes package input declarations, UI state-scope schema/lifecycle declarations, package layout overrides, and package-owned options. State-value mutation is still not promoted.

Implemented Phase 18.4 configuration APIs:

- [`clay.configuration.setPackageOption`](configuration/set-package-option.md) is runtime-backed for package-prefixed typed options: `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback`. Its inventory entry is `status = "runtime-backed"`, `registry_public = true`, uses `op_clay_configuration_set_package_option`, lists `custom_properties` for `packagePrefix`, `option`, `value`, and `source`, and is linked from `docs/index.md` and the generated registry.
- [`clay.ui.serverSetLayoutOverride`](ui/server-set-layout-override.md) is runtime-backed for validated layout/input/action/theme overrides: `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, and `fallback`. It validates source precedence (`user-config`, active major mode, compatible minor mode, global package, package default), target IDs, registered input/action/theme-token references, same-type theme-token remaps, payload size, and prohibited authority.
- `clay.ui.serverRegisterUiStateScope` remains a runtime-backed inert schema/lifecycle declaration API. It is not a state-value mutation API and does not create durable workspace/document/user-config persistence by itself.

Implemented examples:

```ts
import { setPackageOption } from "clay:configuration";
import { serverSetLayoutOverride } from "clay:ui";

setPackageOption({
  packagePrefix: "markdown",
  option: "markdown.layout.defaultSlot",
  value: "right",
  source: "init-js",
});
setPackageOption({
  packagePrefix: "markdown",
  option: "markdown.layout.defaultVisibility",
  value: "hidden",
  source: "init-js",
});
serverSetLayoutOverride({
  targetId: "markdown.preview",
  property: "themeToken",
  value: { token: "markdown.heading.1", fallback: "text.primary" },
  source: "user-config",
});
```

Historical planned examples used hidden-looking names such as `layout.preview.defaultSlot`, `layout.preview.defaultVisibility`, or `theme.markdown.heading.1`; those names are not valid Phase 18.4 package option names unless passed through the documented API with the package-owned prefix and supported option schema. Component style variables, `defaultVisibility`, and slot fields inside package UI declarations are package-load/configuration-time declarations. All hidden JSON/TOML/ad hoc layout, panel, style, input, action, state, or theme keys are rejected by policy, including keys with names such as `layout.preview.defaultSlot`, `layout.preview.defaultVisibility`, `preview.position`, `preview.defaultVisibility`, `theme.markdown.heading.1`, raw token override keys, ad hoc style keys, or unregistered actions when they appear outside documented Clay JS APIs. User configuration cannot implicitly grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw Deno ops / raw ops, native widget handles, direct Masonry widgets, raw CSS, renderer callbacks, or client-side JavaScript authority.

Configuration evaluation for shell/layout remains startup, package-load, configuration-change, or explicit setting-change work. Ordinary typing, Masonry paint/layout, pointer, scroll, keypress, text-event handling, and editor hot paths read already-validated inert state and must not execute package JavaScript, wait on IPC, mutate native layout from package code, or recompute layout from user JavaScript. Deferred surfaces remain explicit planned/deferred work: direct working-area/split/pane-slot mutation, pane selector syntax, multi-panel ordering, overlay z-order, cross-window layout, package enable/disable from configuration, durable state-value mutation, and persisted workspace/document/user-config state storage.

## Phase 18.5 large-file Markdown configuration review

Phase 18.5 reviewed Markdown large-file behavior and did **not** promote any new user-facing configuration API for the first-party Markdown thresholds. The current Markdown package owns fixed defaults for full/windowed/degraded/plain-text-fallback behavior: full highlighting through `1 MiB`, windowed highlighting above `1 MiB`, large-file behavior above `5 MiB`, `64 KiB` parse windows, `4 KiB` guard ranges, `30 MiB` retained syntax/decor cache budget, and `50 ms` parser timeout.

Those values are documented package defaults, not hidden `init.js` keys. The package registers bounded parser metadata through the existing [`serverRegisterParseHandler`](parse/server-register-parse-handler.md) Clay JS API, whose behavior-changing parser policy fields are listed in `custom_properties` and validated by the server before scheduling parser work. File-size thresholds and degraded-mode labels remain package-owned constants until a later phase implements concrete Markdown option schemas through the now-runtime-backed `clay.configuration.setPackageOption` or a future concrete `clay.configuration.setParsePolicy` validator with registry docs, custom-property metadata, and explicit security tests.

Configuration evaluation remains load-time or explicit setting-change work only. Markdown large-file policy must not be recomputed from user JavaScript during keypress, paint, scroll, layout, text-event handling, or parse-result publication. The existing `setModePreference`, `setDecorationTheme`, and `setParsePolicy` facades remain unavailable stubs; `setPackageOption` is runtime-backed only for the documented Phase 18.4 package option names. None of these APIs grant package enable/disable, filesystem, network, shell, extension loading, AI mutation, workspace mutation, WASM, raw-op, or client-side JavaScript authority.

## Phase 18.7 persistent runtime and parse bridge configuration review

Phase 18.7 reviewed the persistent server runtime, generic selected-file open activation, and token-backed JS parse handler bridge. It does **not** promote a new user-tunable configuration API.

The default end-user configuration remains the existing one-line package load:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
```

That call runs once per persistent runtime lifetime, registers mode activation metadata through `clay:modes`, and registers the package parser through [`clay.parse.serverRegisterParseHandler`](parse/server-register-parse-handler.md). Selected-file open then reuses those resident declarations through `serverActivateClassifiedMode` and `ParseCoordinator`; it does not create per-open runtime roots, copy package `dist/` files, or require hidden `init.js` keys.

Parse budgets introduced or exercised by the bridge are package-author registration fields, not user configuration knobs: `timeoutMs`, `maxWindowBytes`, `guardBytes`, `memoryBudgetBytes`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, and `SYNTAX_CACHE_BUDGET_BYTES` are documented on the parse API or as compiled server budgets. `clay.runtime.timeout` is a runtime diagnostic emitted when a configuration, package load, or parse-handler evaluation exceeds its validated guard; it is not a callable `clay:configuration` API and cannot be raised, lowered, or disabled from `init.js`. The planned `clay.configuration.setParsePolicy` facade remains unavailable until a future phase defines concrete user-facing validators, persistence, registry docs, and security tests.

Configuration remains startup/package-load/explicit setting-change work only. Ordinary typing, edit acknowledgement, local paint, viewport scrolling, selected-file parse scheduling, parse-result publication, and decoration rendering do not execute user configuration JavaScript. This review adds no filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw-op, client-side JavaScript, executable callback, or parser-authority grant.

## Phase 18.10 syntax grammar configuration review

Phase 18.10 reviewed package-provided Tree-sitter syntax grammars and does **not** promote a new user-facing syntax configuration API. The only end-user configuration needed in this phase is explicit first-party package loading from `~/.config/clay/init.js`:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
```

That call loads an installed, validated, first-party grammar-only package and lets the package load entry register inert grammar metadata through [`clay.syntax.serverRegisterSyntaxGrammar`](syntax/server-register-syntax-grammar.md). Active syntax grammar selection, grammar/query/style-map validation, and Tree-sitter parse/highlight scheduling are package-load/open/reload/reclassification work. They are not recomputed from user JavaScript during keypress, paint, layout, scroll, pointer, text-event handling, edit acknowledgement, parse-result publication, or decoration rendering.

No hidden JSON/TOML/ad hoc syntax keys are valid in this phase. Rejected examples include `syntax.preferredGrammar`, `treeSitter.grammarPath`, `syntax.styleMap`, `syntax.captureStyles`, `syntax.autoLoad`, `autoLoadSyntaxPackages`, and raw grammar path/style-map override blobs. Grammar artifacts, query paths, file patterns, and style maps are package-owned manifest metadata validated by `assemble_package_record` and `clay:syntax`, not end-user configuration knobs. Grammar package enablement is explicit `loadPackage("@clay/<language>")`, not automatic core loading and not an auto-load flag.

This review adds no filesystem, network, shell, package-manager, AI, WASM, raw-op, native-widget, client-runtime, package-control, package-enable/disable, third-party grammar, native artifact, or client-side JavaScript authority. If a future phase adds syntax preferences, grammar overrides, or theme/style configuration, each behavior-changing option must be a documented Clay JS API with custom properties, Markdown docs, generated registry coverage, hot-path policy, and security tests.

## Phase 18.16 syntax engine configuration review

Phase 18.16 promotes exactly one syntax-engine configuration API: [`clay.syntax.setSyntaxEnginePreference`](syntax/set-syntax-engine-preference.md). No call is needed for normal use. The default end-user setup remains explicit package loading from `~/.config/clay/init.js`; no preference is required for normal first-party highlighting:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/markdown");
```

Use `setSyntaxEnginePreference` only when a user intentionally forces an engine tier for a language/package during `init.js` or package-load setup:

```js
import { setSyntaxEnginePreference } from "clay:syntax";

setSyntaxEnginePreference("rust", "wasm");
setSyntaxEnginePreference("markdown", "javascript");
```

Default behavior with no preference is Tier 1 native for first-party Rust, TypeScript, TSX, JavaScript, and Markdown; explicit `wasm` enables the Tier 2 artifact path for an already-validated first-party package; explicit `javascript`/`js` suppresses syntax-grammar selection so package JS parse handlers remain the Tier 3 fallback. Preference targets are lowercase language ids, package API prefixes, or first-party package names such as `rust`, `markdown`, or `@clay/rust`. This API has no default key binding, no file watcher, no hidden config file key, and no automatic package loading behavior.

The public registry entry lists `custom_properties` for `target` and `tier`, empty `key_bindings`, lookup tags including `configuration` and `syntax`, and security metadata. Hidden JSON/TOML/ad hoc syntax-engine keys remain invalid, including `syntax.engine`, `syntax.preferredEngine`, `syntax.preferredGrammar`, `treeSitter.engine`, `treeSitter.grammarPath`, `treeSitter.wasmPath`, `syntax.styleMap`, `syntax.captureStyles`, `syntax.autoLoad`, and `autoLoadSyntaxPackages`.

Configuration evaluation and preference lookup happen only during startup, package load, document open, reload, or reclassification work. Ordinary keypress, paint, layout, scroll, pointer, text-event handling, edit acknowledgement, parse-result publication, and decoration rendering paths do not execute user configuration JavaScript, recompute engine choices, or run package JavaScript.

This API grants no filesystem, network, shell, package-manager, native-library, extension loading, AI mutation, workspace, package enable/disable, arbitrary third-party grammar, raw-op, WASM artifact, native-widget, client-runtime, client-side JavaScript, raw CSS/color, or parser callback authority. It only records user-initiated engine preference for already-validated first-party syntax packages; packages cannot silently promote themselves over native tier.

## Plan 056 low-latency syntax configuration review

Plan 056 does **not** promote a new user-facing `clay:configuration` API. One parse/capture pass per stable window/version, coordinator coalescing, Tree-sitter reuse and changed-range querying, 128-byte decoration fan-out, empty authoritative replacement chunks, and optimistic span interpolation are correctness and latency internals, not user policy choices.

No configuration call is needed for normal syntax behavior. [`clay.syntax.setSyntaxEnginePreference`](syntax/set-syntax-engine-preference.md) remains the only relevant user engine-selection surface, with documented `target` and `tier` properties; it selects an already-validated engine tier but does not tune scheduling. Existing package parser registration fields remain package metadata validated at load time, not `init.js` latency controls.

The stable-window cap, query/parse fallback rules, decoration payload/cache budgets, output chunk size, cancellation/coalescing, and interpolation policy remain compiled and validated by Clay. Hidden/ad hoc keys are invalid, including `syntaxDebounceMs`, `syntaxWordBoundaryOnly`, `syntaxParseWindowBytes`, `syntaxDecorationChunkBytes`, `syntaxInterpolation`, and `clientSyntaxParser`; no `clay.configuration.setSyntaxDebounce`, `clay.configuration.setSyntaxWindow`, `clay.configuration.setSyntaxChunkSize`, or `clay.configuration.setClientSyntaxParser` API exists.

Configuration evaluation remains startup, package-load, reload, or explicit documented setting-change work. Keypress, text edits, edit acknowledgement, parse scheduling/publication, paint, layout, and scroll cannot run configuration JavaScript or dynamically raise parser/cache/payload limits. This review grants no parser callback, filesystem, network, shell, extension loading, AI mutation, raw-op, package, workspace, WASM, or client-side JavaScript authority.

## Phase 18.18 first-party language package configuration review

Phase 18.18 promoted four first-party language packages from grammar-only metadata to full-mode contracts: Tier 1 native grammar with vocabulary styleMaps, expanded editor behavior (indent/electric/pairs/comment/autocomplete triggers), priority-0 base completion providers carrying bounded static keyword items, importable inert status items, and decoupled Markdown native-decoration-vs-package-JS-SDUI-preview. This review did **not** promote a new user-facing `clay:configuration` API.

Every user-visible Phase 18.18 behavior flows through existing phase-appropriate Clay JS APIs. No per-language configuration toggle, user-preference key, or hidden `init.js` key was introduced for Rust, TypeScript, JavaScript, or Markdown behavior.

User-visible Phase 18.18 configuration surfaces:

| Behavior | API / surface | Notes |
|---|---|---|
| Package loading | [`clay.packages.loadPackage`](packages/load-package.md) | One call per language; no auto-load, no hidden `autoLoadLanguagePackages` key |
| Active theme (color/style resolution) | [`clay.theme.setTheme`](theme/set-theme.md) | Theme `tokenType` + `modifiers` rules resolve all vocabulary token colors; no per-language color overrides |
| Engine tier override | [`clay.syntax.setSyntaxEnginePreference`](syntax/set-syntax-engine-preference.md) | Optional explicit tier choice; native is the default for all four first-party languages |
| Typography (font families/sizes) | [`clay.theme.setTypography`](theme/set-typography.md) | User-owned families and sizes; packages declare semantic roles only |
| Editor behavior (indent/pairs/electric/comment/autocomplete triggers) | [`clay.behavior.buildCodeEditingManifest`](behavior/build-code-editing-manifest.md) | Package-owned manifest declared at load time through `serverRegisterModePattern`; no per-language behavior toggle |
| Completion keyword/snippet items | [`clay.completion.serverRegisterCompletionProvider`](completion/server-register-completion-provider.md) | Package-owned validated static items; no user-facing keyword-list toggle or per-language item filter |
| Markdown preview/status panel visibility | [`clay.configuration.setPackageOption`](configuration/set-package-option.md) / [`clay.ui.serverSetLayoutOverride`](ui/server-set-layout-override.md) | Existing `markdown.layout.defaultVisibility` option and `markdown.preview` `visibility` override; no new panel-display API |
| Package enable/disable | `PackageService` (CLI, not `init.js`) | Privileged operation, not a user-configuration key |

Default end-user configuration:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/markdown");
```

No additional configuration call is required. Package load entries register editor behavior, completion providers, commands, and status items through existing generic facades; theme resolution, engine selection, and typography remain independently settable through their respective APIs.

Hidden/ad hoc configuration keys that are rejected by policy and are not valid unless expressed through a documented API above:

- `language.enable`, `language.disable`, `enableRust`, `enableTypeScript`, `enableJavaScript`, `enableMarkdown`
- `language.indentWidth`, `language.tabSize`, `rust.indentSize`, `typescript.indentSize`, `javascript.indentSize`, `markdown.indentSize`
- `language.lineComment`, `language.pairs`, `language.electricOutdent`, `language.autocompleteTriggers`
- `completion.keywords`, `completion.snippets`, `completion.items`, `completion.enable`, `completion.disable`
- `markdown.preview`, `markdown.preview.enabled`, `markdown.decoration.engine`, `markdown.highlight`
- `syntax.styleMap`, `syntax.captureStyles`, `syntax.vocabulary`, `syntax.tokenType`, `language.tokens`
- `language.behavior`, `language.mode`, `language.command`, `language.statusItem`
- Per-language auto-load keys, implicit language-enable flags, theme-token override blobs, or ad hoc parser-policy blobs

All editor behavior, completion keyword lists, syntax mappings, and mode patterns are package-owned inert metadata validated at package load time. Theme colors and font families remain owned by the user through `setTheme` and `setTypography`. Engine tier is selected through `setSyntaxEnginePreference`. Package enable/disable authority remains outside `init.js`.

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work. Ordinary keypress, Masonry paint/layout, pointer, scroll, text-event handling, edit acknowledgement, parse scheduling, parse-result publication, decoration rendering, and completion result construction do not execute user configuration JavaScript or recompute language-specific behavior settings. This review grants no parser, filesystem, network, shell, LSP, AI, package-manager, WASM, raw-op, native-widget, client-side JavaScript, CSS, package enable/disable, grammar-artifact, third-party grammar, or client-render authority.

## Phase 19 persistent-runtime hot reload configuration review

Phase 19 promotes exactly one built-in reload command: `clay.runtime.reloadConfiguration` (**Reload Configuration and Packages**). There is no `clay:configuration` JS facade for reload; the command is invoked through the existing key-routing, Control Center, SDUI action, and transient-menu paths after an explicit user binding or discovery action.

### Command metadata

| Property | Value |
|---|---|
| Command ID | `clay.runtime.reloadConfiguration` |
| User-facing name | Reload Configuration and Packages |
| Routing policy | `ServerFirstWithLock { lock_scope: Behavior }` |
| Default key bindings | None (empty) |
| Permissions | None (empty) |
| Package provenance | Clay-owned built-in (`package_name = "clay"`) |
| JS facade | None — not callable from `clay:commands`, `clay:configuration`, or any package JS |

### Explicit binding (optional)

Users may expose reload through a key binding, the Control Center, or an SDUI action — all three route through the same inert behavior manifest and revalidate through `CommandExecutor` before any side effect:

```js
import { bindKey } from "clay:keybindings";

// Optional: bind a chord to the built-in reload command.
bindKey("Ctrl+Shift+R", "clay.runtime.reloadConfiguration", { scope: "global" });
```

No default binding exists. Control Center discovers the command from the built-in command table; its metadata lists empty `key_bindings` and empty `permissions`.

### Compiled budgets (not configurable)

Reload grace ceilings, snapshot bounds, and broadcast capacity are compiled server budgets, not `init.js` keys:

| Budget | Constant | Value |
|---|---|---|
| Stale-edit grace window | `PREVIOUS_BEHAVIOR_GRACE_MS` | 2 000 ms |
| Max grace transactions | `PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS` | 256 |
| Snapshot max documents | `RUNTIME_STATE_SNAPSHOT_MAX_DOCUMENTS` | 64 |
| Snapshot max diagnostics | `RUNTIME_STATE_SNAPSHOT_MAX_DIAGNOSTICS` | 32 |
| Broadcast capacity | `RUNTIME_STATE_BROADCAST_CAPACITY` | 16 |
| Frame ceiling | `DEFAULT_MAX_FRAME_SIZE` | 1 MiB |
| Diff-upgrade trigger | p95 payload ≥ 768 KiB or p95 client install ≥ 16 ms | Measured, not configured |

These are security/performance boundaries defined in `src/perf/budgets.rs`. Raising them from `init.js` would undermine the very boundary they enforce.

### Rejected hidden configuration keys

No hidden JSON/TOML/ad hoc keys are valid for reload. Rejected examples include:

- `hotReload`, `hot_reload`, `reloadOnSave`, `autoReload`, `reloadPackages`
- `reload.keybinding`, `reload.trigger`, `reload.watch`, `reload.debounce`
- `reload.graceMs`, `reload.maxGraceTransactions`, `reload.snapshotMaxDocuments`
- File-watcher paths, inotify/FSEvents flags, polling intervals
- Auto-reload-on-save, reload-after-package-install, reload-after-config-change

The `IpcServer::trigger_developer_hot_reload`, `RuntimeReloadOutcome`, and `ReloadedDocumentRefresh` Rust helpers remain `#[doc(hidden)]` test/developer surfaces; they are not exported from a Clay JS facade, not listed in the public API registry, and not callable from `~/.config/clay/init.js`. `pub(crate) async fn reload_runtime_generation` is the shared implementation, never directly invoked from package or configuration JavaScript.

### Security

Reload reruns `~/.config/clay/init.js` in a fresh generation with an empty `globalThis.__clayLoadedPackages` cache and a rebuilt `PackageLoadEntryAllowlist`. Candidate evaluation happens outside the behavior lock; the lock is acquired only for the bounded compare-and-swap commit. Validation, snapshot construction, and document refresh preparation are internal to the server.

Reload does not broaden package source trust or permissions. Exact language-server grants must be re-declared through `authorizeLanguageServer` in the fresh generation. Old-generation workers, sessions, and child processes are cleaned after commit. A concurrent reload trigger returns `ReloadInProgress`; it does not queue another evaluation.

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work only. Ordinary keypress, paint, layout, scroll, pointer, text-event handling, edit acknowledgement, parse scheduling, parse-result publication, decoration rendering, and completion result construction do not execute configuration JavaScript or check reload settings. This review grants no package-manager, arbitrary filesystem, network, shell, extension loading, package-control, AI mutation, workspace expansion, WASM, raw-op, client-side JavaScript, or native widget authority beyond user-approved package capabilities.

## Phase 19 Windows open-dialog configuration review

Phase 19 reviewed the Windows Markdown open-dialog smoke path and did **not** promote a new dialog-settings configuration API. The configurable behavior is the key binding itself, expressed through the existing [`bindKey`](keybindings/bind-key.md) Clay JS API:

```js
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

`clay.documents.clientOpenFileDialog` is a fixed Clay command ID that can be routed by inert behavior manifests after configuration evaluation. No default `Ctrl+O` shortcut in Rust exists; without an `init.js` binding or fixture binding, `Ctrl+O` is not treated as the open-file command.

Dialog behavior uses fixed defaults, not hidden `init.js` keys: native dialog support on Windows, Linux (xdg-desktop-portal), and macOS (`NSOpenPanel`), Markdown filters for `.md`, `.markdown`, and `.mdown`, an all-files fallback, cancellation as a non-error no-op, and selected-file-only server validation/granting through existing capability tokens. The `windows-markdown-open` development fixture uses normal package, SDUI, parse/decorations, and `bindKey` APIs; it does not introduce ad hoc keys such as dialog filters, default directories, package enablement settings, or callable client-side hooks.

Configuration remains server startup/load-time work. Pressing the configured key uses client-local manifest routing and then an explicit native UI command; ordinary keypress, paint, scroll, layout, text-event, edit acknowledgement, and Markdown decoration rendering paths do not execute configuration JavaScript. This configuration route does not grant arbitrary filesystem authority, package installation or enable/disable authority, shell, network, AI, WASM, raw Deno ops, workspace expansion, or client-side JavaScript authority.

## Phase 18.20 language-server configuration review

Phase 18.20 promotes exactly one configuration API: [`clay.language-server.authorizeLanguageServer`](language-server/authorize-language-server.md). This is a configuration-only grant API callable only from `~/.config/clay/init.js` during configuration root evaluation **before** the first `loadPackage` call seals authority.

Default end-user configuration with a language-server bridge package:

```js
// ~/.config/clay/init.js
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});

await loadPackage("@clay/lsp-rust");
```

`authorizeLanguageServer` binds exact package provenance (name/version/source), contribution descriptor fingerprint, resolved canonical executable, declared inheritance environment names, and current directory workspace-root ids. The grant starts **no process** — process spawn happens later when `startLanguageServerSession` is called with a matching package, contribution, and an approved workspace root id.

Grants are accepted only during configuration root evaluation (`init.js`). The first `loadPackage` call atomically seals authority mutation for the runtime generation. Loaded package code cannot self-grant even though it can import the same `clay:language-server` facade. Bundled `@clay/*` package auto-authorization explicitly excludes `language-server` unless an exact current grant already exists.

| Behavior | API / surface | Notes |
|---|---|---|
| Language-server grant | [`clay.language-server.authorizeLanguageServer`](language-server/authorize-language-server.md) | Configuration-only; sealed before first `loadPackage` |
| Session start/read/write/stop | [`clay.language-server.startLanguageServerSession`](language-server/start-language-server-session.md) | Runtime-backed; requires prior grant with matching contribution/root |
| Package contribution metadata | `clay.contributions.languageServers` in `package.json` | Fixed at package install time; validated descriptor with id/executable/args/inheritEnvironment |
| Workspace roots | [`clay.workspace.clientOpenFolderDialog`](workspace/client-open-folder-dialog.md) (client UI) + server state | Roots are workspace-state metadata, not `init.js` keys |

Hidden/ad hoc configuration keys rejected by policy:

- `languageServer.enable`, `languageServer.disable`, `enableLanguageServer`, `autoStartLanguageServer`
- `languageServer.binary`, `languageServer.command`, `languageServer.cwd`, `languageServer.env`
- `lsp.enable`, `lsp.disable`, `lsp.server`, `lsp.binary`, `lsp.autoStart`
- Raw executable path strings, shell strings, ad hoc environment variable blobs, or unvalidated contribution identifiers outside the documented `authorizeLanguageServer` API

Server-owned security/performance ceilings (`LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES`, `LANGUAGE_SERVER_STDERR_BUDGET_BYTES`, `LANGUAGE_SERVER_MAX_SESSIONS`, `LANGUAGE_SERVER_READ_TIMEOUT_MS`) are compiled budgets in `src/perf/budgets.rs`, not `init.js` configuration knobs.

Configuration evaluation remains startup or explicit reload work only. Ordinary keypress, paint, layout, scroll, pointer, text-event handling, edit acknowledgement, parse-result publication, decoration rendering, and completion result construction do not execute configuration JavaScript, start language servers, or revalidate grants. This API grants no filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw-op, native-widget, client-side JavaScript, or arbitrary process authority beyond the exact fixed validated contribution descriptor and approved roots.

## Phase 18.21 LSP bridge package configuration review

Phase 18.21 ships four first-party LSP bridge packages (`@clay/lsp-rust`, `@clay/lsp-typescript`, `@clay/lsp-javascript`, `@clay/lsp-markdown`) and promotes **no new user-facing `clay:configuration` API**. Every user-visible configuration surface reuses existing Phase 18.20 APIs.

### Default end-user configuration

Configure all four LSP bridge packages:

```js
// ~/.config/clay/init.js
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

// Grant each bridge before any loadPackage seals authority.
await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
await authorizeLanguageServer({
  package: "@clay/lsp-typescript",
  contribution: "lsp-typescript.server",
  workspaceRootIds: [1],
});
await authorizeLanguageServer({
  package: "@clay/lsp-javascript",
  contribution: "lsp-javascript.server",
  workspaceRootIds: [1],
});
await authorizeLanguageServer({
  package: "@clay/lsp-markdown",
  contribution: "lsp-markdown.server",
  workspaceRootIds: [1],
});

// Load base grammar packages before their LSP bridges.
await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
await loadPackage("@clay/markdown");

// Then load the LSP bridge packages.
await loadPackage("@clay/lsp-rust");
await loadPackage("@clay/lsp-typescript");
await loadPackage("@clay/lsp-javascript");
await loadPackage("@clay/lsp-markdown");
```

Single-bridge minimal setup (Rust only):

```js
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
await loadPackage("@clay/rust");
await loadPackage("@clay/lsp-rust");
```

### Configuration surfaces

| Behavior | API / surface | Notes |
|---|---|---|
| Language-server grant | [`clay.language-server.authorizeLanguageServer`](language-server/authorize-language-server.md) | Configuration-only; sealed before first `loadPackage` |
| Package loading | [`clay.packages.loadPackage`](packages/load-package.md) | One call per package; base before bridge |
| Completion provider disable | [`clay.completion.serverDisableCompletion`](completion/server-disable-completion.md) | Disable per-bridge or per-package-prefix; LSP completion defaults to priority 100 non-exclusive |
| Document analyzer registration | [`clay.language.serverRegisterDocumentAnalyzer`](language/server-register-document-analyzer.md) | Package-load-time only; called from bridge `dist/load.js`, not init.js directly |
| Language intelligence registration | [`clay.language.serverRegisterLanguageIntelligenceProvider`](language/server-register-language-intelligence-provider.md) | Package-load-time; no separate per-server config needed |

### Rejected hidden configuration keys

No hidden JSON/TOML/ad hoc keys are valid for LSP bridge configuration. Rejected examples include:

- `lsp.enable`, `lsp.disable`, `lsp.autoStart`, `enableLsp`, `enableRustAnalyzer`, `enableTypescriptServer`, `enableMarksman`
- `lsp.rust.binary`, `lsp.typescript.path`, `lsp.markdown.config`, `lsp.binary`, `lsp.command`, `lsp.args`, `lsp.env`
- `rustAnalyzer.path`, `typescriptServer.path`, `typescript.tsserverPath`, `marksman.config`
- Per-server environment variable keys (`TSSERVER_PATH`, `RUST_ANALYZER_PATH`, `MARKSMAN_PATH`)
- Hidden workspace-root configuration blobs, per-language LSP on/off toggles, automatic package-load flags

Executable paths, arguments, environment inheritance, and working directories are fixed in each package's `clay.contributions.languageServers` manifest descriptor — validated at install time, revalidated on every session operation, and never user-tunable from `init.js`.

### Compiled budgets (not configurable)

All LSP bridge security/performance ceilings are compiled into the server binary and are intentionally not exposed as `clay:configuration` APIs:

| Budget | Constant | Value |
|---|---|---|
| Message budget | `LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES` | 1 MiB |
| Stderr budget | `LANGUAGE_SERVER_STDERR_BUDGET_BYTES` | 64 KiB |
| Max sessions | `LANGUAGE_SERVER_MAX_SESSIONS` | 16 |
| Read timeout | `LANGUAGE_SERVER_READ_TIMEOUT_MS` | 3 000 ms |
| Max workers | `DOCUMENT_ANALYSIS_MAX_WORKERS` | 4 |
| Worker heap | `DOCUMENT_ANALYSIS_WORKER_HEAP_BYTES` | 64 MiB |
| Max docs/worker | `DOCUMENT_ANALYSIS_MAX_DOCUMENTS_PER_WORKER` | 32 |
| Max text/worker | `DOCUMENT_ANALYSIS_MAX_TEXT_BYTES_PER_WORKER` | 8 MiB |
| Max doc bytes | `DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES` | 256 KiB |
| Input mailbox | `DOCUMENT_ANALYSIS_INPUT_MAX_DELTAS` / `DOCUMENT_ANALYSIS_INPUT_MAX_BYTES` | 64 / 2 MiB |
| Output queue | `DOCUMENT_ANALYSIS_OUTPUT_MAX_EVENTS` / `DOCUMENT_ANALYSIS_OUTPUT_MAX_BYTES` | 64 / 512 KiB |
| Decoration payload | `DECORATION_PAYLOAD_BUDGET_BYTES` | 8 KiB |
| Diagnostic payload | `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES` | 8 KiB |
| Result payload | `RESULT_PAYLOAD_BUDGET_BYTES` | 16 KiB |

These are security boundaries defined in `src/perf/budgets.rs` and enforced by the server before delivery. Raising them from `init.js` would undermine the very boundary they enforce.

### Diagnostic composition and completion precedence

LSP diagnostics use source-keyed replacement: each LSP bridge publishes diagnostics for its own source, and applying a new set replaces only the previous set from that same (package, source, document, version) key. Current LSP error/warning spans suppress overlapping Tree-sitter recovery diagnostics; non-overlapping Tree-sitter diagnostics and all Info-level LSP diagnostics remain additive. Composition runs once per `EditorDiagnosticState.apply_set`, never on the paint hot path.

LSP completion providers register at priority 100 non-exclusive, merging with the base keyword completion provider (priority 0). Use [`clay.completion.serverDisableCompletion`](completion/server-disable-completion.md) to suppress a bridge's completion provider while keeping its diagnostics and intelligence:

```js
import { serverDisableCompletion } from "clay:completion";

// Suppress only the TypeScript bridge completion, keep diagnostics + intelligence.
serverDisableCompletion({ packagePrefix: "lsp-typescript" });
```

### Security

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work. Ordinary keypress, paint, layout, scroll, pointer, text-event handling, edit acknowledgement, parse-result publication, decoration rendering, and completion result construction do not execute configuration JavaScript, start language servers, spawn child processes, or revalidate grants.

This configuration path grants no filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw-op, native-widget, client-side JavaScript, or arbitrary process authority. The only process authority is the exact fixed validated contribution descriptor and approved roots granted through `authorizeLanguageServer`. LSP framing, sync, capabilities, position encoding, URI conversion, and cancellation remain package-owned JS — Rust core stays LSP-wire neutral with zero `Content-Length`, `jsonrpc`, `textDocument/*`, or `$/cancelRequest` markers.

## Phase 18.5 Markdown end-user loading configuration audit

Phase 18.5 task 8 (plan `plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md`) closes the configuration audit for Markdown end-user loading. Every behavior-changing Markdown configuration surface from the replan is either a fully documented runtime-backed Clay JS API or an explicitly planned/unavailable API. No undocumented configuration keys are introduced.

Markdown need → Clay JS API mapping:

| Markdown need | Clay JS API | Status | Custom properties |
|---|---|---|---|
| Markdown package options (`layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, `fallback`) | [`clay.configuration.setPackageOption`](configuration/set-package-option.md) | runtime-backed | `packagePrefix`, `option`, `value`, `source` |
| Markdown layout overrides (`slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, `fallback`) for targets such as `markdown.preview` | [`clay.ui.serverSetLayoutOverride`](ui/server-set-layout-override.md) | runtime-backed | `targetId`, `property`, `value`, `source` |
| Markdown theme-token declarations (`markdown.preview.background`, `markdown.preview.padding`, `markdown.heading.1`, ...) | [`clay.ui.serverRegisterThemeToken`](ui/server-register-theme-token.md) | runtime-backed | `token`, `type`, `fallback`, `description`, `source` |
| Markdown panel visibility defaults (e.g., `markdown.preview` with `defaultVisibility: "hidden"`) | [`clay.ui.serverRegisterPanelContribution`](ui/server-register-panel-contribution.md) | runtime-backed | `id`, `slot`, `kind`, `defaultVisibility`, `component`, `actionTargets` |
| Markdown input routing defaults | [`clay.ui.serverRegisterInputContribution`](ui/server-register-input-contribution.md) | runtime-backed | `id`, `scope`, `componentId`, `pointer.*`, `focus.*`, `actionTargets` |
| Markdown UI state scopes | [`clay.ui.serverRegisterUiStateScope`](ui/server-register-ui-state-scope.md) | runtime-backed | `id`, `scope`, `owner`, `lifetime`, `persistence`, `valueSchema.kind` |
| Markdown file-dialog key binding | [`clay.keybindings.bindKey`](keybindings/bind-key.md) | runtime-backed | `key`, `commandId`, `scope` |
| Markdown mode activation preference | `clay.configuration.setModePreference` | planned (unavailable) | n/a |
| Markdown decoration theme preference | `clay.configuration.setDecorationTheme` | planned (unavailable) | n/a |
| Markdown parse policy (timeout, windows, memory budget) | `clay.configuration.setParsePolicy` | planned (unavailable); parse-handler policy fields are validated through [`clay.parse.serverRegisterParseHandler`](parse/server-register-parse-handler.md) | n/a |
| One-line package loading | `clay.packages.loadPackage` | implemented (Plan 029, Phase 18.6; Plan 035 generalizes to source-aware loading of bundled and installed user-authorized packages); see `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` for the unified authority model | n/a |

Markdown-specific hidden/ad hoc configuration keys that are rejected by policy and are not valid unless expressed through one of the documented APIs above with the package-owned prefix and a supported option/property name:

- `preview.position`, `preview.defaultVisibility`, `preview.slot`
- `layout.preview.defaultSlot`, `layout.preview.defaultVisibility`, `layout.preview.splitRatio`
- `theme.markdown.heading.1`, `theme.markdown.preview.background`, raw token override keys
- `markdown.sidebar.width`, ad hoc sidebar/layout/style keys
- Unregistered action keys, ad hoc input routing keys, ad hoc state-blob keys

The default Markdown load path (through `clay.packages.loadPackage("@clay/markdown")`, implemented by Plan 029) does not publish a default side panel: the optional Markdown preview is a package `PanelContribution` with `defaultVisibility: "hidden"` targeting the `right` slot, shown only through `setPackageOption` or `serverSetLayoutOverride`. The package-owned `markdownLoadMode()` remains available as a convenience alias for per-load options. Markdown package options such as `markdown.layout.defaultVisibility` and `markdown.layout.defaultSlot` go through `clay.configuration.setPackageOption`; Markdown layout overrides such as `markdown.preview` `visibility`/`themeToken` go through `clay.ui.serverSetLayoutOverride`; Markdown theme tokens go through `clay.ui.serverRegisterThemeToken`. None of these APIs grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw Deno ops / raw ops, native widget handles, direct Masonry widgets, raw CSS, renderer callbacks, or client-side JavaScript authority.

Configuration evaluation for Markdown end-user loading remains startup, package-load, configuration-change, or explicit setting-change work only. Markdown keypress, paint, scroll, layout, text-event, edit acknowledgement, parse-result publication, and decoration rendering paths do not execute configuration JavaScript, recompute package options from user code, or mutate native layout from package code.

## Example Configuration

```js
// ~/.config/clay/init.js
import { loadConfigurationModule } from "clay:configuration";
import { bindKey } from "clay:keybindings";
import { defineEditorView, defineFlex, definePanel, publishTree } from "clay:sdui";

await loadConfigurationModule({ path: "./keys.js" });

bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });

await publishTree(
  defineFlex({
    direction: "row",
    children: [
      definePanel({ title: "Workspace", children: [] }),
      defineEditorView({ documentId: 1 }),
    ],
  }),
);
```

## Security Boundary

Configuration can customize documented Clay behavior through Clay JS APIs. It must not implicitly grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority. Modular loading is constrained to local configuration files under the configuration root; it is not a package manager, extension loader, workspace scanner, network fetcher, shell runner, or client-side JavaScript execution hook. Permission-bearing APIs still require explicit documented permissions and server-side validation.

## Plan 035 unified package authority configuration review

Plan 035 unified first-party and third-party package authority. The configuration-relevant surfaces are user authorization/capability grants, runtime profile choices, package graph relations, package-control overrides, and conflict resolution overrides. Each must be a documented Clay JS/config API or explicitly documented CLI/UI state — never a hidden JSON/TOML/ad hoc key.

Unified package authority configuration surfaces:

| Surface | Current status | Surface / API | Custom properties |
|---|---|---|---|
| User authorization / capability grants | Rust primitive implemented (`PackageService::authorize_package`); no callable end-user JS/CLI surface yet — planned | `clay.packages.authorize` (planned inventory entry, `registry_public = false`) | `package`, `capabilities`, `runtimeProfile`, `source`, `approvedBy` |
| Runtime profile selection | Rust primitive implemented (`RuntimeProfile` enum); bound to the authorization grant | `clay.packages.authorize` `runtimeProfile` parameter | `runtimeProfile:enum=native-trust\|sandboxed\|restricted` |
| User conflict override | Rust primitive implemented (`PackageService::set_conflict_override`); no callable end-user JS/CLI surface yet — planned | `clay.packages.setConflictOverride` (planned inventory entry, `registry_public = false`) | `contributionId`, `winnerPackage` |
| Package graph relations (`dependsOn`/`extends`/`disables`/`replaces`) | Manifest-declared by package authors; validated and evaluated at enable/load/reload time | `package.json` `clay.dependsOn`/`clay.extends`/`clay.disables`/`clay.replaces` arrays | manifest metadata, not `init.js` configuration |
| Package-control (`disables`/`replaces`) authority | Requires explicit `package-control` capability grant; fails closed with `MissingPackageControlGrant` | `clay.capabilities`/`clay.permissions` manifest array | capability string `package-control` |
| Bundled package auto-authorization | Implemented: the `loadPackage` resolver auto-authorizes bundled `@clay/*` packages with `native-trust` and `approvedBy = "clay-bundled-default"` | internal to resolver; not an `init.js` key | n/a |
| Authorization inspection | Implemented: `inspect` reports `requestedCapabilities`, `approvedCapabilities`, and `runtimeProfile` | `clay package inspect <name>` CLI | CLI state, not `init.js` configuration |

The intended end-user authorization shape is:

```javascript
import { authorize } from "clay:packages"; // planned, not yet callable

await authorize({
  package: "@vendor/foo",
  capabilities: ["network"],
  runtimeProfile: "sandboxed",
  approvedBy: "user",
});
```

Until the `clay.packages.authorize` and `clay.packages.setConflictOverride` facades/CLI commands ship, the only current grant path is the bundled-package auto-authorization inside the resolver, plus the Rust primitives exercised by tests. A user-installed package that requests a powerful capability (filesystem, network, shell, wasm, ai-tools, workspace-mutation, native-ui, client-runtime, raw-ops, package-control) cannot currently be granted through an end-user surface — enable fails closed with `MissingCapabilityGrant`. This is a documented implementation gap, not a hidden configuration shortcut; there is no `allowThirdPartyPackages`, `trusted`, `grant`, or capability-grant JSON/TOML key in `init.js`.

`@clay/*` means shipped by Clay, not more capable. The configuration surfaces above apply identically to bundled and user-installed packages; no config primitive branches on package source. Capability grants can grant powerful capabilities only through the explicit authorization flow above, with provenance (package identity/source/version/integrity), visibility (inspectable grants), and revocation (`disable`/`revoke` withdraws the grant and its contributions through `PackageRevocationRecord`).

Configuration evaluation for unified package authority is startup/install/enable/load/reload/explicit-user-command work only. Grant lookup at the enable/load/registration/request boundary is a cheap check against already-loaded authorization state. No source resolution, package-manager call, authorization prompt, grant recording, graph traversal, conflict resolution, or capability evaluation runs from keypress, paint, layout, scroll, text-event, edit-ack, pointer, or Masonry hot paths. See `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` for the authority model.

## Plan 030 security budgets are intentionally not Clay JS APIs

Plan 030 (code-review remediation) hardened several server-side limits. These are **security boundaries**, not user configuration, so they are intentionally **not** exposed as `clay:configuration` APIs and cannot be raised, lowered, or disabled from `init.js`. Raising any of them from user JavaScript would undermine the very boundary it enforces (e.g. a malicious `init.js` could lift the JS evaluation timeout to defeat the watchdog, or raise the openable-file ceiling to exhaust memory). They are compiled into the server binary in `src/perf/budgets.rs` and reviewed through code review and decisions rather than tuned at runtime.

- **JS runtime evaluation timeout** — `JS_RUNTIME_EVALUATION_TIMEOUT_MS` (5000 ms, default). A watchdog thread terminates the V8 isolate when the budget elapses; surfaced as `clay.runtime.timeout`. Not configurable from `init.js`.
- **JS runtime heap limit** — `JS_RUNTIME_HEAP_LIMIT_BYTES` (128 MiB). The persistent runtime is created with `v8::CreateParams::heap_limits`; the near-heap callback terminates execution and surfaces `clay.runtime.heap_limit`. Not configurable from `init.js`.
- **Openable file size** — `MAX_OPENABLE_FILE_BYTES` (768 KiB). Server-side file-open path rejects files above this before allocating full text, with headroom under the 1 MiB codec frame limit. Not configurable from `init.js`.
- **Runtime SDUI tree budgets** — `RUNTIME_SDUI_TREE_PAYLOAD_BUDGET_BYTES` (16 KiB), `RUNTIME_SDUI_TREE_MAX_NODES` (128), `RUNTIME_SDUI_TREE_MAX_DEPTH` (16), `RUNTIME_SDUI_TREE_MAX_NODE_TEXT_CHARS` (4096). Enforced before/during `op_clay_sdui_publish_tree`; rejected with `clay.sdui.invalid_tree`. Not configurable from `init.js`.
- **Large-file resident memory budget** — `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB` (256 MiB). Resident-memory ceiling for editor caches; not a per-open tunable.
- **Package install lifecycle-script suppression** — `pnpm add` runs with `--ignore-scripts` by default. The opt-in is a **CLI flag / env var**, not a Clay JS API: `clay package add --allow-scripts` or `CLAY_ALLOW_LIFECYCLE_SCRIPTS=1`. This is a process-level supply-chain control, not an `init.js` configuration option, and is documented in `docs/reference/primitives/package-loading.md`.
- **File-open capability gate** — `OpenSelectedFile` requires a server-minted single-use capability token issued after the `Hello` handshake; not a configuration option. See `docs/wiki/modules/server-ipc-skeleton.md`.
- **IPC endpoint ownership/permissions** — Unix socket `0o600` + parent-directory ownership and Windows named-pipe current-user-only DACL are OS-level hardening, not Clay JS configuration.

## Plan 034 persistent-runtime hardening is intentionally not configurable

Plan 034 added first-party runtime hardening and a minimal separate-process sandbox harness. These controls are server-owned security boundaries, not user customization. They do **not** promote a new `clay:configuration` API, hidden `init.js` key, JSON/TOML setting, command-line user preference, package option, or package-declared permission that can weaken the runtime boundary.

- **Heap guard** — `JS_RUNTIME_HEAP_LIMIT_BYTES` remains a compiled budget. `clay.runtime.heap_limit` is a diagnostic code, not a callable configuration API.
- **Timeout guard** — `JS_RUNTIME_EVALUATION_TIMEOUT_MS` remains a compiled budget. `clay.runtime.timeout` is a diagnostic code, not a mutable setting.
- **Sandbox supervision** — sandbox child spawn, handshake, payload budget, timeout kill, and restart policy are internal supervisor behavior. There is no `setSandboxDisabled`, `setSandboxTimeout`, or `enableSandboxBypass` configuration surface.
- **Denied authorities** — filesystem, network, shell, WASM, AI mutation, package-manager execution, native-widget handles, raw-op access, and client-side JavaScript remain powerful capabilities that require explicit user-authorized grants under the unified package authority model. They are not categorically denied for non-`@clay/*` packages, but they are never granted implicitly from `init.js`; they flow only through the documented [`clay.packages.authorize`](#plan-035-unified-package-authority-configuration-review) surface with provenance and revocation.
- **Third-party execution gate** — non-`@clay/*` packages now load through the same source-aware `loadPackage` resolver as bundled packages after install and user authorization (see Plan 035). There is no `enableThirdPartyPackages` or `allowThirdPartyPackages` configuration shortcut; capability grants are explicit, visible, and revocable per-package.

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work. Ordinary keypress, paint, layout, scroll, edit acknowledgement, text-event handling, parse-result publication, and decoration rendering paths do not execute configuration JavaScript, wait on sandbox round trips, or re-check runtime hardening knobs.


## Phase 18.8 command execution and transient menu configuration review

Phase 18.8 added the server-owned `CommandExecution` boundary, the generic `TransientMenuSession` state model, and the first Control Center consumer. This review did **not** promote a new user-facing `clay:configuration` API for menu placement, control-center behavior, command filtering, default key bindings, or package action customization. The user-visible configuration surfaces reuse the existing Clay JS APIs; menu/session internals are kept `pub(crate)`/internal.

User-visible Phase 18.8 configuration surfaces:

| Surface | Status | API / mechanism | Notes |
|---|---|---|---|
| Control Center launch key binding | reused, runtime-backed | [`clay.keybindings.bindKey`](keybindings/bind-key.md) | Bind a key to the built-in command `clay.controlCenter.open`; no default chord exists in Rust, so the Control Center is only reachable when `init.js` binds a key |
| Control Center command id | built-in server command | `clay.controlCenter.open` (registered through `builtin_server_command`, `RoutingPolicy::ServerFirst`) | A fixed Clay command ID routed by inert behavior manifests after configuration evaluation; not an `init.js` key |
| Built-in server commands (`workspace.refresh`, `document.focus_active`, `document.open_recent`) | built-in server command | `builtin_server_command_ids` / `builtin_server_command` | Fixed Clay command IDs, not user configuration |
| Package command/action customization | reused, runtime-backed | [`clay.commands.serverRegisterCommand`](commands/server-register-command.md), [`clay.ui.serverRegisterPanelContribution`](ui/server-register-panel-contribution.md), [`clay.ui.serverRegisterInputContribution`](ui/server-register-input-contribution.md), [`clay.configuration.setPackageOption`](configuration/set-package-option.md) | Package commands, action targets, and `action.default`/`input.default` overrides flow through phase 18.3/18.4 package UI/configuration APIs |
| Transient menu session state | internal | `TransientMenuSession` (`src/shell/transient_menu.rs`, `pub(crate)`) | Clay-owned session state: prompt, query, bounded items, selection, status, focus policy, inert activation actions; not user configuration |
| Control Center menu building | internal | `ControlCenter` (`src/server/control_center.rs`, `pub(crate)`) | Filters the registered command snapshot, excludes client-first/client-ui commands, and appends built-ins; not user configuration |
| Command execution validation | internal | `CommandExecutor` (`src/server/command_execution.rs`, `pub(crate)`) | Validates command id, routing policy, provenance, permissions, argument budget, target context, and session/action freshness per request; not user configuration |

The expected end-user Control Center configuration is a normal `~/.config/clay/init.js` binding:

```js
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+P", "clay.controlCenter.open", { scope: "editor" });
```

`clay.controlCenter.open` is a fixed Clay command ID that can be routed by inert behavior manifests after configuration evaluation. No default `Ctrl+Shift+P` shortcut in Rust exists; without an `init.js` binding (or test/fixture binding) the Control Center is not bound by default. `bindKey` is the documented configuration surface — the transient menu is not a callable `clay:configuration` API and cannot be styled, positioned, filtered, or dismissed through `init.js`. Menu geometry, item count limit (`MAX_ITEMS = 256`), query/label/detail/accessibility bounds, focus policy, and built-in command membership are Clay-owned compiled/internal constants, not hidden `init.js` keys.

Hidden/ad hoc configuration keys that are rejected by policy and are not valid unless expressed through a documented API above:

- `controlCenter.key`, `controlCenter.defaultKey`, `controlCenter.shortcut`
- `menu.position`, `menu.alignment`, `menu.maxItems`, `menu.height`, `menu.width`
- `transientMenu.focusPolicy`, `transientMenu.maxItems`, `transientMenu.queryCharLimit`
- `commandExecution.timeout`, `commandExecution.argumentBudget`, `commandExecution.allowBypass`
- `builtins.controlCenter`, ad hoc built-in command injection keys
- Unregistered command ids bound to keys, ad hoc package action routing keys, ad hoc menu filter keys

Package command/action registration through [`clay.commands.serverRegisterCommand`](commands/server-register-command.md) declares routing policy, permissions, key bindings, custom properties, and lookup tags at package-load time; it does not grant execution authority. Command execution authority is re-validated per activation through `CommandExecutor` and never granted by registration, menu inclusion, or configuration. Packages may declare commands and expose them in transient menus; they cannot execute commands directly from UI callbacks, bypass command permission/provenance validation, run command handlers in the Rust client, or grant themselves filesystem, network, shell, AI mutation, WASM, workspace mutation, package-manager, package installation, package enable/disable, native widget, raw-op, or client-side JavaScript authority.

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work only. Command registration, action validation, transient menu filtering over installed bounded metadata, and command-id binding through `bindKey` are load/configuration/update-time work. Activating a selected command enqueues a server-first `CommandExecution` request; ordinary keypress routing, Masonry paint/layout, pointer, scroll, text-event handling, edit acknowledgement, and decoration rendering paths do not execute configuration JavaScript, wait on IPC, recompute package action defaults from user code, or run command handlers. This review adds no filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw-op, client-side JavaScript, executable callback, or command-authority grant.

## Phase 18.11 completion provider configuration review

Phase 18.11 added the `CompletionTriggerAndResult` primitive, the server-side completion provider framework, the built-in `core.bufferWords` provider, and `TransientMenuSession`-based completion display/acceptance. This review did **not** promote a new user-facing `clay:configuration` API for provider priority, provider enable/disable, trigger characters, buffer-word limits, completion menu placement, commit behavior, or result item budgets. The user-visible configuration surfaces reuse the existing Clay JS APIs; provider/coordinator/menu/acceptance internals are kept `pub(crate)`/internal.

User-visible Phase 18.11 configuration surfaces:

| Surface | Status | API / mechanism | Notes |
|---|---|---|---|
| Manual completion trigger key binding | reused, runtime-backed | [`clay.keybindings.bindKey`](keybindings/bind-key.md) | Bind a key to the built-in `UiReactivePriority` command `completion.trigger`; no default chord exists in Rust, so manual completion is only reachable when `init.js` binds a key (e.g. `Ctrl+Space`) |
| Completion trigger command id | built-in server command | `completion.trigger` (registered through `CommandDeclaration::ui_reactive`, `RoutingPolicy::UiReactivePriority`) | A fixed Clay command ID routed by inert behavior manifests after configuration evaluation; not an `init.js` key |
| Autocomplete trigger characters | package manifest metadata | `clay.contributions.autocompleteTriggers` | Inert single-character manifest entries classified locally by `ClientBehaviorState`; not user configuration |
| Completion provider metadata registration | runtime-backed package load entry | [`clay.completion.serverRegisterCompletionProvider`](completion/server-register-completion-provider.md) | Package-prefixed provider id, priority, inert trigger characters, inert word-boundary chars, bounded `timeoutMs`/`maxItems`; metadata-only in Phase 18.11 |
| Completion provider enable/disable | package load/disable | `clay.packages.loadPackage` / `PackageService` disable | Provider enablement is tied to package load/disable, not a hidden config key; the built-in `core.bufferWords` provider is always available and is not removed by package disable/reload |
| Completion result/item budgets | compiled security boundaries | `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`, `COMPLETION_RESULT_MAX_ITEMS`, per-field char caps in `src/perf/budgets.rs` | Enforced before client publication; not tunable from `init.js` |
| Completion request payload budget | compiled security boundary | `COMPLETION_REQUEST_PAYLOAD_BUDGET_BYTES` in `src/perf/budgets.rs` | Not tunable from `init.js` |
| Completion coordinator/menu state | internal | `CompletionCoordinator` (`src/server/completion.rs`, `pub(crate)`), `TransientMenuSession` completion projection (`src/shell/transient_menu.rs`, `pub(crate)`) | Clay-owned scheduling/cancellation/stale-drop/menu state; not user configuration |
| Completion acceptance | internal | `EditorSurface::accept_completion_with_event` (`src/editor/surface.rs`, `pub(crate)`) | Commits a validated text replacement in the active document only; never executes a command, raw op, or provider code |

The expected end-user manual completion configuration is a normal `~/.config/clay/init.js` binding:

```js
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Space", "completion.trigger", { scope: "editor" });
```

`completion.trigger` is a fixed Clay command ID that can be routed by inert behavior manifests after configuration evaluation. No default `Ctrl+Space` shortcut in Rust exists; without an `init.js` binding (or test/fixture binding) manual completion is not bound by default. `bindKey` is the documented configuration surface — the completion menu is not a callable `clay:configuration` API and cannot be styled, positioned, filtered, or dismissed through `init.js`. Menu geometry, item count limit, query/label/detail/accessibility bounds, focus policy, commit-character handling, and built-in provider membership are Clay-owned compiled/internal constants, not hidden `init.js` keys.

Provider priority, provider enablement, trigger characters, buffer-word limits, completion menu placement, and commit behavior are not hidden JSON/TOML/ad hoc `init.js` keys. Hidden/ad hoc configuration keys that are rejected by policy and are not valid unless expressed through a documented API above:

- `completion.trigger`, `completion.defaultKey`, `completion.shortcut`, `autocomplete.key`
- `completion.providerPriority`, `completion.providers`, `completion.enabledProviders`
- `completion.triggerCharacters`, `completion.wordBoundaryChars`, `completion.bufferWordLimit`
- `completion.menuPlacement`, `completion.menu.height`, `completion.menu.maxItems`, `completion.commitCharacters`
- `completion.timeout`, `completion.maxItems`, `completion.payloadBudget`
- Unregistered provider ids, ad hoc provider-enable/disable keys, ad hoc trigger-character override keys

Package completion provider registration through [`clay.completion.serverRegisterCompletionProvider`](completion/server-register-completion-provider.md) declares package-prefixed provider id, priority, inert trigger characters, inert word-boundary chars, and bounded `timeoutMs`/`maxItems` at package-load time; it does not grant execution authority. Phase 18.11 is metadata-only: Clay rejects `handler`/`callback`/`complete`/`function`/`module` executable values, raw ops, native handles, client JavaScript, snippets/commands, URLs, shell/network/AI/WASM/native/package-manager fields, duplicate ids, reserved `clay.*` ids, and oversize metadata. Providers may read only Clay-provided open-document content/windows; completion grants no filesystem, network, shell, AI mutation, extension loading, workspace mutation, package enable/disable, WASM, raw-op, native-widget, client-JS, or provider execution authority without later documented APIs and an approved decision log.

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work only. Provider metadata registration, trigger classification over installed inert manifest state, completion request enqueueing through a bounded non-blocking channel, and command-id binding through `bindKey` are load/configuration/update-time work. Provider execution runs server-side on a cancellable `UiReactivePriority` lane that aborts or stale-drops older in-flight requests and validates results against the current document/behavior version and provider generation before publication; ordinary keypress routing, local text mutation, Masonry paint/layout, pointer, scroll, text-event handling, edit acknowledgement, and decoration rendering paths do not execute configuration JavaScript, wait on IPC, run provider code, recompute provider metadata from user code, or mutate native layout from package code. This review adds no filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw-op, client-side JavaScript, executable callback, or provider-authority grant.

## Phase 18.12 workspace file-browser configuration review

Phase 18.12 added server-owned workspace-root discovery, bounded directory listing, a Clay-owned left file-browser panel, a bottom transient fuzzy-open route, and server-authoritative open/reveal command routing. This review did **not** promote a new user-facing `clay:configuration` API for file-browser visibility, file-browser slot placement, fuzzy-open key binding defaults, workspace root markers, ignore-list overrides, listing depth/count limits, tree refresh policy, reveal behavior, or raw file-open paths. User-visible configuration reuses existing Clay JS APIs: `clay.keybindings.bindKey` for command chords and the Phase 18.12 `clay:workspace` / `clay:commands` APIs for explicit workspace actions. Phase 20 daily-editing chords (cut/paste/undo/redo/save/open-documents/recovery) are documented in the [Phase 20 configuration review](#phase-20-daily-editing-product-hardening-configuration-review).

User-visible Phase 18.12 configuration surfaces:

| Surface | Status | API / mechanism | Notes |
|---|---|---|---|
| Fuzzy-open key binding | reused, runtime-backed | [`clay.keybindings.bindKey`](keybindings/bind-key.md) | Bind a key to the built-in server-first command `clay.workspace.openFuzzyFile`; no default chord exists in Rust, so fuzzy open is only reachable when `init.js` binds a key or another Clay-owned action opens it |
| File-browser toggle key binding | reused, runtime-backed | [`clay.keybindings.bindKey`](keybindings/bind-key.md) | Bind a key to `clay.workspace.toggleFileBrowser`; the command is validated by `CommandExecutor`, not a hidden panel-visibility key |
| Native folder picker binding | reused, runtime-backed | [`clay.keybindings.bindKey`](keybindings/bind-key.md), `clay.workspace.clientOpenFolderDialog` | Bind a key to the fixed client UI command id; native selection still goes through selected-path capability and server root validation |
| Copy current selection binding | reused, runtime-backed | [`clay.keybindings.bindKey`](keybindings/bind-key.md), `clay.editor.clientCopySelection` | Bind an alternate key to copy the current native editor selection |
| File open/reveal commands | runtime-backed command APIs | [`clay.commands.serverOpenFile`](commands/server-open-file.md), [`clay.commands.serverRevealInTree`](commands/server-reveal-in-tree.md), [`clay.commands.serverExecuteCommand`](commands/server-execute-command.md) | Open and reveal route through server workspace APIs, root-relative paths, selected-file grants, and open-document metadata validation |
| Workspace roots and discovery | runtime-backed workspace APIs | [`clay.workspace.serverAddWorkspaceRoot`](workspace/server-add-workspace-root.md), [`clay.workspace.serverDiscoverWorkspaceRootForPath`](workspace/server-discover-workspace-root-for-path.md), [`clay.workspace.serverListWorkspaceRoots`](workspace/server-list-workspace-roots.md) | Roots and grants are explicit server-authoritative workspace APIs, not configuration keys |
| Directory listing | runtime-backed workspace APIs | [`clay.workspace.serverListDirectory`](workspace/server-list-directory.md), [`clay.workspace.serverCreateListingCancelToken`](workspace/server-create-listing-cancel-token.md), [`clay.workspace.serverCancelListing`](workspace/server-cancel-listing.md) | Listing uses server validation, bounded depth/count, compiled ignore defaults, optional cancellation tokens, and diagnostics |
| Left file-browser panel visibility/slot | Clay-owned shell state | `src/shell/file_browser.rs::FileBrowserState`; `FixedSlotId::Left` via SDUI composition | The first-party left panel is Clay-owned UI, not package or user configuration in this phase |
| Marker file set | compiled workspace boundary | `KNOWN_PROJECT_MARKERS` in `src/server/workspace.rs` | Closed Clay-owned marker table (`.git`, `Cargo.toml`, `package.json`); packages/users cannot extend it through `init.js` |
| Ignore defaults and list budgets | compiled listing boundary | `DEFAULT_IGNORED_NAMES`, `MAX_LIST_DIRECTORY_DEPTH`, `MAX_LIST_DIRECTORY_ENTRIES`, `MAX_LEFT_PANEL_ENTRIES`, `MAX_FUZZY_ITEMS` | Bounded security/performance constants, not hidden `init.js` keys |

The expected end-user fuzzy-open configuration is a normal `~/.config/clay/init.js` binding:

```js
import { bindKey } from "clay:keybindings";
import { clientCopySelection } from "clay:editor";
import { clientOpenFolderDialog } from "clay:workspace";

bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
bindKey("Ctrl+P", "clay.workspace.openFuzzyFile", { scope: "editor" });
bindKey("Ctrl+B", "clay.workspace.toggleFileBrowser", { scope: "editor" });
bindKey("Ctrl+Shift+C", clientCopySelection(), { scope: "editor" });
```

`clay.workspace.openFuzzyFile` and `clay.workspace.toggleFileBrowser` are fixed Clay command IDs validated by `CommandExecutor`. `clay.workspace.clientOpenFolderDialog` and `clay.editor.clientCopySelection` are fixed client UI command IDs returned by synchronous Clay JS helpers. No default `Ctrl+P` or `Ctrl+B` shortcut in Rust exists for Phase 18.12 fuzzy/toggle routes; no default `Ctrl+Shift+O` or `Ctrl+Shift+C` shortcut in Rust exists for folder/copy workflow routes. Native copy (`Ctrl/Cmd+C`) is handled directly by the editor. `bindKey` is the documented configuration surface — the file-browser panel, fuzzy-open menu, workspace discovery scanner, directory listing service, ignore set, marker set, listing budgets, folder-picker backend, and clipboard backend are not callable `clay:configuration` APIs and cannot be styled, repositioned, resized, widened, filtered, granted extra workspace authority, or expose package/configuration/AI clipboard-contents APIs through `init.js`.

Hidden/ad hoc configuration keys that are rejected by policy and are not valid unless expressed through a documented API above:

- `fileBrowser.defaultVisibility`, `fileBrowser.visible`, `fileBrowser.leftPanelDefault`, `workspace.fileBrowser.leftPanelDefault`
- `fileBrowser.slot`, `fileBrowser.position`, `fileBrowser.width`, `workspace.fileBrowser.width`
- `fuzzyOpen.key`, `fuzzyOpen.defaultKey`, `fileBrowser.fuzzyOpenKey`, `workspace.fuzzyOpenKey`
- `workspace.markers`, `workspace.markerFiles`, `workspace.rootMarkers`, `workspace.discoveryDepth`
- `workspace.ignore`, `workspace.ignoreRules`, `fileBrowser.ignore`, `fileBrowser.exclude`
- `fileBrowser.maxDepth`, `fileBrowser.maxEntries`, `fileBrowser.maxItems`, `fileBrowser.refreshInterval`
- `workspace.rawPath`, `workspace.allowArbitraryPath`, `workspace.allowOutsideRoot`, ad hoc selected-file/folder grant keys
- `clipboard.text`, `clipboard.writeText`, `clipboard.readText`, `copySelection.text`, arbitrary clipboard strings, package/config clipboard-contents keys

File-browser listing/open/reveal authority is server-owned. Root discovery scans only bounded ancestry with a closed marker set; directory listing stays inside known roots and uses bounded ignore/depth/count limits; open file commands route through `WorkspaceState::open_existing_file` or selected-file grants through `WorkspaceState::open_selected_file`; reveal validates open document metadata. Configuration cannot grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw-op, native widget, direct Masonry widget, arbitrary root marker, arbitrary ignore-rule, arbitrary path passthrough, or client-side JavaScript authority.

## Phase 20 daily editing product hardening configuration review

Phase 20 (plan `plans/055-Phase20-Daily-Editing-Product-Hardening.md`) ships clipboard cut/paste, inverse-edit undo/redo, IME preedit, multi-document retain/switch, save/conflict recovery menus, pending-edit/disconnect/resync recovery chrome, cross-platform file-open dialogs, and accessibility/theme polish. This review did **not** promote a new user-facing `clay:configuration` API. Every user-visible Phase 20 behavior reuses existing Clay JS command helpers plus [`clay.keybindings.bindKey`](keybindings/bind-key.md). Command helpers keep empty `custom_properties` because there are no user-tunable setting fields — only fixed command IDs.

### User-visible Phase 20 configuration surfaces

| Surface | Status | API / mechanism | Notes |
|---|---|---|---|
| Open Markdown file dialog | reused, runtime-backed | [`bindKey`](keybindings/bind-key.md), [`clay.documents.clientOpenFileDialog`](documents/client-open-file-dialog.md) | No default `Ctrl+O` in Rust; native dialogs on Windows, Linux (xdg-desktop-portal), and macOS (`NSOpenPanel`) use fixed Markdown/all-files filters |
| Save active document | reused, runtime-backed | [`bindKey`](keybindings/bind-key.md), [`clay.documents.serverSaveDocument`](documents/server-save-document.md) | Recommended `Ctrl+S` binding; client intercepts the intent and enqueues `SaveDocument`; dirty chrome + stale-metadata recovery stay Clay-owned |
| Reload active document | reused, runtime-backed | [`bindKey`](keybindings/bind-key.md), [`clay.documents.serverReloadDocument`](documents/server-reload-document.md) | Optional binding; dirty-reload conflicts open Clay-owned recovery menus |
| Cut current selection | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`clay.editor.clientCutSelection`](editor/client-cut-selection.md) | Alternate chord; native `Ctrl/Cmd+X` remains editor-handled |
| Paste clipboard text | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`clay.editor.clientPasteClipboard`](editor/client-paste-clipboard.md) | Alternate chord; native `Ctrl/Cmd+V` remains editor-handled |
| Undo latest local edit | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`clay.editor.clientUndo`](editor/client-undo.md) | Alternate chord; native `Ctrl/Cmd+Z` remains editor-handled |
| Redo latest undone edit | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`clay.editor.clientRedo`](editor/client-redo.md) | Alternate chord; native `Ctrl/Cmd+Shift+Z` / non-macOS `Ctrl+Y` remain editor-handled |
| Open-documents switcher | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`clay.editor.clientShowOpenDocuments`](editor/client-show-open-documents.md) | Lists retained sessions and activates one locally; no tabstrip configuration API |
| Request resync | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`clay.editor.clientRequestResync`](editor/client-request-resync.md) | Enqueues `RequestResync` for the active document |
| Dismiss recovery chrome | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`clay.editor.clientDismissRecovery`](editor/client-dismiss-recovery.md) | Clears disconnect/rejection recovery menus and sanitized diagnostics |
| Theme selection | reused (Phase 18.15) | [`clay.theme.setTheme`](theme/set-theme.md) | Phase 20 does not rebuild themes; only verifies contrast/status-label polish |

### Recommended daily-editing `init.js` bindings

```js
// ~/.config/clay/init.js
import { bindKey } from "clay:keybindings";
import {
  clientCutSelection,
  clientPasteClipboard,
  clientUndo,
  clientRedo,
  clientShowOpenDocuments,
  clientRequestResync,
  clientDismissRecovery,
} from "clay:editor";
import { clientOpenFileDialog } from "clay:documents";

bindKey("Ctrl+O", clientOpenFileDialog(), { scope: "editor" });
bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
bindKey("Ctrl+Shift+X", clientCutSelection(), { scope: "editor" });
bindKey("Ctrl+Shift+V", clientPasteClipboard(), { scope: "editor" });
bindKey("Alt+Backspace", clientUndo(), { scope: "editor" });
bindKey("Ctrl+Y", clientRedo(), { scope: "editor" });
bindKey("Ctrl+Shift+E", clientShowOpenDocuments(), { scope: "editor" });
bindKey("Ctrl+Shift+R", clientRequestResync(), { scope: "editor" });
bindKey("Ctrl+Shift+D", clientDismissRecovery(), { scope: "editor" });
```

No default `Ctrl+O` or `Ctrl+S` shortcut exists in Rust. Without an `init.js` (or fixture) binding, those chords do not route to open/save. Native cut/copy/paste and undo/redo chords remain editor-handled even when alternate `bindKey` routes exist. IME preedit overlay, dirty/conflict recovery menus, pending-edit status chrome, and multi-document session retention are Clay-owned runtime behavior — not callable `clay:configuration` APIs and not package-tunable through hidden keys.

### Compiled budgets (not configurable)

Phase 20 security/performance ceilings are compiled constants, not `init.js` keys:

| Budget | Constant | Value | Owner |
|---|---|---|---|
| Undo/redo stack depth | `EDIT_HISTORY_MAX_DEPTH` | 256 entries | `src/perf/budgets.rs` |
| Undo/redo entry payload | `EDIT_HISTORY_MAX_ENTRY_BYTES` | 64 KiB (oversized entries clear both stacks) | `src/perf/budgets.rs` |
| Retained multi-document sessions | `CLIENT_DOCUMENT_SESSION_MAX` | 64 (aligned with `RUNTIME_STATE_SNAPSHOT_MAX_DOCUMENTS`) | `src/perf/budgets.rs` |
| Accessibility display-name budget | `ACCESSIBILITY_DISPLAY_NAME_MAX_CHARS` | 64 | `src/editor/accessibility.rs` |
| Accessibility recovery-summary budget | `ACCESSIBILITY_RECOVERY_SUMMARY_MAX_CHARS` | 256 | `src/editor/accessibility.rs` |
| Status chrome contrast floor | `STATUS_CHROME_MIN_CONTRAST` | 4.5:1 (WCAG AA) | `src/editor/theme.rs` |

Raising these from configuration would undermine the memory, observability, and authority boundaries they enforce. Empty `custom_properties` on the Phase 20 command docs record that no user-tunable setting fields exist for these ceilings.

### Rejected hidden configuration keys

No hidden JSON/TOML/ad hoc keys are valid for Phase 20 daily editing. Rejected examples include:

- `undo.depth`, `undo.maxDepth`, `redo.stackSize`, `editHistory.maxEntries`, `history.maxEntryBytes`
- `documentSession.max`, `multiDocument.maxSessions`, `openDocuments.max`, `tabs.max`
- `recovery.autoResync`, `recovery.prompts.enabled`, `disconnect.autoReconnect`, `pendingEdits.maxVisible`
- `save.autoSave`, `save.onFocusLost`, `conflict.autoResolve`, `dirty.autoClear`
- `ime.preedit.enabled`, `composition.showOverlay`, `composition.commitOnBlur`
- `clipboard.text`, `clipboard.writeText`, `clipboard.readText`, package/config/AI clipboard-contents keys
- `dialog.filters`, `dialog.defaultDirectory`, `openFile.extensions`, `fileDialog.backend`
- `accessibility.labelTemplate`, `status.dirtyMarker`, theme rebuild keys that bypass `clay.theme.setTheme`

### Security

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work only. Ordinary keypress routing, Masonry paint/layout, pointer, scroll, text-event handling, IME preedit paint, edit acknowledgement, pending-edit observation, and recovery-menu presentation do not execute configuration JavaScript.

Phase 20 configuration does **not** invent clipboard-exfiltration, arbitrary filesystem, network, shell, package-manager, WASM, raw-op, or client-side JavaScript authority APIs. Broader package/configuration/AI authority over clipboard, filesystem, shell, network, and raw ops remains deferred (`decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`). Binding a Phase 20 command through `bindKey` installs only an inert user-mediated route; clipboard cut/paste stay client-local after explicit user action, save/reload still consume server grants/leases, open dialogs still return selected-file capabilities only, and recovery menus only reuse existing `RequestResync` / save / reload / dismiss primitives.
