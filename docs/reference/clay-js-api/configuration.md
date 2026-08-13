# Clay Configuration System

Clay configuration is JavaScript loaded from `~/.config/clay/init.js`. The configuration system is part of the Clay JS API surface: every configurable option, command binding, and behavior-changing setting should be represented by a documented Clay JS API and included in the Markdown documentation registry.

## Configuration Entry Point

- Default file: `~/.config/clay/init.js`
- Canonical example: the `examples/` tree in the Clay repository (`init.js` base config plus `packages/first-party.js` and `packages/third-party.js` modules) demonstrates every supported configuration surface with all documented options annotated; copy the whole tree (`cp -r examples/. ~/.config/clay/`) and adjust. Plans that add configuration surfaces must keep it current.
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

[`theme.setTypography`](theme/set-typography.md) atomically configures user-owned monospace, proportional, and UI family stacks, logical-pixel sizes, and optional per-role ligature/feature policies (Plan 071 task 7). No call is required for defaults; absent `ligatures` keeps standard (`liga`+`clig`) and contextual (`calt`) ligatures enabled.

```js
import { setTypography } from "clay:theme";

setTypography({
  monospace: {
    families: ["JetBrains Mono", "monospace"],
    size: 16,
    // Optional: disable contextual alternates for code, keep standard ligatures.
    ligatures: { enableStandard: true, enableContextual: false },
  },
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

[`theme.setTheme`](theme/set-theme.md) selects a first-party theme whose `textStyles` already include `diagnosticError`, `diagnosticWarning`, and `diagnosticInfo`. [`syntax.setSyntaxEnginePreference`](syntax/set-syntax-engine-preference.md) selects the parser tier; no separate diagnostics preference exists. [`diagnostics.serverPublishDiagnostics`](diagnostics/server-publish-diagnostics.md) is a package publication API gated by `render-decorations`, not an `init.js` user setting.

Compiled budgets (`DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`, `DIAGNOSTIC_MAX_SPANS_PER_SET`, `DIAGNOSTIC_CACHE_BUDGET_BYTES`, and related field caps) and Clay-owned squiggle amplitude/period/stroke constants are security/performance boundaries, not hidden `init.js` keys. Hidden/ad hoc keys rejected by policy include `diagnostics.enabled`, `diagnostics.enable`, `diagnostics.squiggleWidth`, `diagnostics.amplitude`, `diagnostics.severity`, `syntaxError.highlight`, `treeSitter.showErrors`, and parallel JSON/TOML diagnostic preference blobs.

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work only. Ordinary keypress, paint, layout, scroll, pointer, text-event handling, edit acknowledgement, parse-result publication, and diagnostic/decoration rendering paths do not execute user configuration JavaScript or recompute diagnostic preferences. This review grants no parser, filesystem, network, shell, LSP, AI, package-manager, WASM, raw-op, native-widget, client-side JavaScript, CSS, or client-render authority.

## Phase 17 package and mode configuration review

Phase 17 reviewed package loading, mode selection, decoration transport, parse coordination, and package-owned SDUI contributions for concrete user-visible settings.

- Package enable/disable remains a privileged package service or CLI operation, not `init.js` configuration and not a side effect of package options.
- `configuration.setPackageOption`, `configuration.setModePreference`, `configuration.setDecorationTheme`, and `configuration.setParsePolicy` are preserved as planned `clay:configuration` facade exports and inventory entries. Their inventory records include custom properties, hot-path policy, permission/security notes, and planned op paths, but they are not linked as public registry docs until server-side validators and concrete behavior-changing settings are promoted.
- Phase 17 did not introduce concrete user-facing SDUI panel visibility or layout settings. Package-owned SDUI region/layout data remains inert package contribution metadata validated at enable/load time.
- `clay:sdui.queryUiState` remains deferred. `SduiObservableSnapshot` and `SduiStatusObservation` stay internal observability/test infrastructure until a package-tooling, help, or agent workflow requires a public live-UI query API with full docs, registry, permissions/privacy notes, and tests.

## Phase 18.2/18.3/18.4 shell/layout and package UI configuration review

Compatibility summary for existing guards: Phase 18.2/18.3 shell/layout and package UI configuration review; Phase 18.2 does **not** promote any new runtime-backed or user-visible shell/layout configuration API; Phase 18.3 promotes package UI declaration APIs; Phase 18.3 promotes `ui.serverRegisterThemeToken` to a runtime-backed package declaration API; Phase 18.3 does not promote user-visible panel visibility, default-slot, component-style, theme-token override, or layout behavior configuration APIs; `ui.serverSetLayoutOverride` is the planned `PackageLayoutOverride` surface; `configuration.setPackageOption` remains the planned package-owned option surface.

Phase 18.1 defined the shell/layout architecture contract, and Phase 18.2 implements internal Rust shell layout state for `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, the `ClayShellWidget` root, inert local layout updates, and structural shell observability. Phase 18.2 does **not** promote any new runtime-backed or user-visible shell/layout configuration API. Phase 18.3 promotes package UI declaration APIs for panels, components, overlays, and theme tokens. Historical Phase 18.3 status: Phase 18.3 promotes package UI declaration APIs but does not promote user-visible panel visibility, default-slot, component-style, theme-token override, or layout behavior configuration APIs; those surfaces were not user-visible override APIs and `ui.serverSetLayoutOverride` and `configuration.setPackageOption` stay non-registry-public inventory rows in that phase. Phase 18.4 promotes package input declarations, UI state-scope schema/lifecycle declarations, package layout overrides, and package-owned options. State-value mutation is still not promoted.

Implemented Phase 18.4 configuration APIs:

- [`configuration.setPackageOption`](configuration/set-package-option.md) is runtime-backed for package-prefixed typed options: `layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, and `fallback`. Its inventory entry is `status = "runtime-backed"`, `registry_public = true`, uses `op_clay_configuration_set_package_option`, lists `custom_properties` for `packagePrefix`, `option`, `value`, and `source`, and is linked from `docs/index.md` and the generated registry.
- [`ui.serverSetLayoutOverride`](ui/server-set-layout-override.md) is runtime-backed for validated layout/input/action/theme overrides: `slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, and `fallback`. It validates source precedence (`user-config`, active major mode, compatible minor mode, global package, package default), target IDs, registered input/action/theme-token references, same-type theme-token remaps, payload size, and prohibited authority.
- `ui.serverRegisterUiStateScope` remains a runtime-backed inert schema/lifecycle declaration API. It is not a state-value mutation API and does not create durable workspace/document/user-config persistence by itself.

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

Those values are documented package defaults, not hidden `init.js` keys. The package registers bounded parser metadata through the existing [`serverRegisterParseHandler`](parse/server-register-parse-handler.md) Clay JS API, whose behavior-changing parser policy fields are listed in `custom_properties` and validated by the server before scheduling parser work. File-size thresholds and degraded-mode labels remain package-owned constants until a later phase implements concrete Markdown option schemas through the now-runtime-backed `configuration.setPackageOption` or a future concrete `configuration.setParsePolicy` validator with registry docs, custom-property metadata, and explicit security tests.

Configuration evaluation remains load-time or explicit setting-change work only. Markdown large-file policy must not be recomputed from user JavaScript during keypress, paint, scroll, layout, text-event handling, or parse-result publication. The existing `setModePreference`, `setDecorationTheme`, and `setParsePolicy` facades remain unavailable stubs; `setPackageOption` is runtime-backed only for the documented Phase 18.4 package option names. None of these APIs grant package enable/disable, filesystem, network, shell, extension loading, AI mutation, workspace mutation, WASM, raw-op, or client-side JavaScript authority.

## Phase 18.7 persistent runtime and parse bridge configuration review

Phase 18.7 reviewed the persistent server runtime, generic selected-file open activation, and token-backed JS parse handler bridge. It does **not** promote a new user-tunable configuration API.

The default end-user configuration remains the existing one-line package load:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
```

That call runs once per persistent runtime lifetime, registers mode activation metadata through `clay:modes`, and registers the package parser through [`parse.serverRegisterParseHandler`](parse/server-register-parse-handler.md). Selected-file open then reuses those resident declarations through `serverActivateClassifiedMode` and `ParseCoordinator`; it does not create per-open runtime roots, copy package `dist/` files, or require hidden `init.js` keys.

Parse budgets introduced or exercised by the bridge are package-author registration fields, not user configuration knobs: `timeoutMs`, `maxWindowBytes`, `guardBytes`, `memoryBudgetBytes`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, and `SYNTAX_CACHE_BUDGET_BYTES` are documented on the parse API or as compiled server budgets. `runtime.timeout` is a runtime diagnostic emitted when a configuration, package load, or parse-handler evaluation exceeds its validated guard; it is not a callable `clay:configuration` API and cannot be raised, lowered, or disabled from `init.js`. The planned `configuration.setParsePolicy` facade remains unavailable until a future phase defines concrete user-facing validators, persistence, registry docs, and security tests.

Configuration remains startup/package-load/explicit setting-change work only. Ordinary typing, edit acknowledgement, local paint, viewport scrolling, selected-file parse scheduling, parse-result publication, and decoration rendering do not execute user configuration JavaScript. This review adds no filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw-op, client-side JavaScript, executable callback, or parser-authority grant.

## Phase 18.10 syntax grammar configuration review

Phase 18.10 reviewed package-provided Tree-sitter syntax grammars and does **not** promote a new user-facing syntax configuration API. The only end-user configuration needed in this phase is explicit first-party package loading from `~/.config/clay/init.js`:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
```

That call loads an installed, validated, first-party grammar-only package and lets the package load entry register inert grammar metadata through [`syntax.serverRegisterSyntaxGrammar`](syntax/server-register-syntax-grammar.md). Active syntax grammar selection, grammar/query/style-map validation, and Tree-sitter parse/highlight scheduling are package-load/open/reload/reclassification work. They are not recomputed from user JavaScript during keypress, paint, layout, scroll, pointer, text-event handling, edit acknowledgement, parse-result publication, or decoration rendering.

No hidden JSON/TOML/ad hoc syntax keys are valid in this phase. Rejected examples include `syntax.preferredGrammar`, `treeSitter.grammarPath`, `syntax.styleMap`, `syntax.captureStyles`, `syntax.autoLoad`, `autoLoadSyntaxPackages`, and raw grammar path/style-map override blobs. Grammar artifacts, query paths, file patterns, and style maps are package-owned manifest metadata validated by `assemble_package_record` and `clay:syntax`, not end-user configuration knobs. Grammar package enablement is explicit `loadPackage("@clay/<language>")`, not automatic core loading and not an auto-load flag.

This review adds no filesystem, network, shell, package-manager, AI, WASM, raw-op, native-widget, client-runtime, package-control, package-enable/disable, third-party grammar, native artifact, or client-side JavaScript authority. If a future phase adds syntax preferences, grammar overrides, or theme/style configuration, each behavior-changing option must be a documented Clay JS API with custom properties, Markdown docs, generated registry coverage, hot-path policy, and security tests.

## Phase 18.16 syntax engine configuration review

Phase 18.16 promotes exactly one syntax-engine configuration API: [`syntax.setSyntaxEnginePreference`](syntax/set-syntax-engine-preference.md). No call is needed for normal use. The default end-user setup remains explicit package loading from `~/.config/clay/init.js`; no preference is required for normal first-party highlighting:

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

No configuration call is needed for normal syntax behavior. [`syntax.setSyntaxEnginePreference`](syntax/set-syntax-engine-preference.md) remains the only relevant user engine-selection surface, with documented `target` and `tier` properties; it selects an already-validated engine tier but does not tune scheduling. Existing package parser registration fields remain package metadata validated at load time, not `init.js` latency controls.

The stable-window cap, query/parse fallback rules, decoration payload/cache budgets, output chunk size, cancellation/coalescing, and interpolation policy remain compiled and validated by Clay. Hidden/ad hoc keys are invalid, including `syntaxDebounceMs`, `syntaxWordBoundaryOnly`, `syntaxParseWindowBytes`, `syntaxDecorationChunkBytes`, `syntaxInterpolation`, and `clientSyntaxParser`; no `configuration.setSyntaxDebounce`, `configuration.setSyntaxWindow`, `configuration.setSyntaxChunkSize`, or `configuration.setClientSyntaxParser` API exists.

Configuration evaluation remains startup, package-load, reload, or explicit documented setting-change work. Keypress, text edits, edit acknowledgement, parse scheduling/publication, paint, layout, and scroll cannot run configuration JavaScript or dynamically raise parser/cache/payload limits. This review grants no parser callback, filesystem, network, shell, extension loading, AI mutation, raw-op, package, workspace, WASM, or client-side JavaScript authority.

## Plan 057 syntax-decoration continuity and replacement correctness configuration review

Plan 057 does **not** promote a new user-facing `clay:configuration` API. Complete authoritative replacement chunks (query coverage == replacement coverage, UTF-8-safe chunk grid), same-word narrow-syntax provisional inheritance (Unicode alphanumeric/underscore extends at token end, whitespace/newline/punctuation stops), and unchanged broad-syntax edge behavior are correctness fixes, not user policy choices.

No configuration call is needed. [`syntax.setSyntaxEnginePreference`](syntax/set-syntax-engine-preference.md) remains the sole relevant user engine-selection surface. The same-word boundary predicate (`is_alphanumeric()` or `_`) and the shared 128-byte replacement chunk grid are compiled invariants validated by tests.

Hidden/ad hoc keys are invalid, including any of `syntaxSameWordBoundary`, `syntaxReplacementChunkGrid`, `syntaxWordInheritance`, `syntaxCompletionWordCharacter`, `syntaxChunkQueryCoverage`, `syntaxProvisionalInheritance`, `syntaxCompleteReplacement`, and `syntaxUtf8ChunkGrid`. No `configuration.setSyntax*` API exists for these names.

Configuration evaluation stays outside keypress, text-edit, edit-acknowledgement, parse, publication, paint, layout, and scroll paths. This review grants no parser callback, filesystem, network, shell, or client-side JavaScript authority.

## Plan 058 exact-range provisional decoration replacement configuration review

Plan 058 does **not** promote a new user-facing `clay:configuration` API. Exact-range authoritative viewport subtraction (`subtract_half_open_range`, `subtract_provisional_chunk`), local provisional residual coalescing (`coalesce_local_residual`, `coalesce_compatible_spans`), and bounded residual chunk-count invariants are correctness fixes, not user policy choices.

No configuration call is needed. [`syntax.setSyntaxEnginePreference`](syntax/set-syntax-engine-preference.md) remains the sole relevant user engine-selection surface. The half-open interval subtraction, per-package/layer residual isolation, and bounded 512-cycle coalescing ceiling are compiled invariants validated by tests.

Hidden/ad hoc keys are invalid, including any of `syntaxExactRangeReplacement`, `syntaxProvisionalSubtraction`, `syntaxResidualCoalescing`, `syntaxSubtractionCoalescing`, `syntaxExactRangeSubtraction`, `syntaxProvisionalResidual`, and `syntaxCoalescingStrategy`. No `configuration.setSyntax*` API exists for these names.

Configuration evaluation stays outside keypress, text-edit, edit-acknowledgement, parse, publication, paint, layout, and scroll paths. Authoritative subtraction and coalescing run only in `apply_set`, not in local-edit or paint hot paths. This review grants no parser callback, filesystem, network, shell, or client-side JavaScript authority.

## Phase 18.18 first-party language package configuration review

Phase 18.18 promoted four first-party language packages from grammar-only metadata to full-mode contracts: Tier 1 native grammar with vocabulary styleMaps, expanded editor behavior (indent/electric/pairs/comment/autocomplete triggers), priority-0 base completion providers carrying bounded static keyword items, importable inert status items, and decoupled Markdown native-decoration-vs-package-JS-SDUI-preview. This review did **not** promote a new user-facing `clay:configuration` API.

Every user-visible Phase 18.18 behavior flows through existing phase-appropriate Clay JS APIs. No per-language configuration toggle, user-preference key, or hidden `init.js` key was introduced for Rust, TypeScript, JavaScript, or Markdown behavior.

User-visible Phase 18.18 configuration surfaces:

| Behavior | API / surface | Notes |
|---|---|---|
| Package loading | [`packages.loadPackage`](packages/load-package.md) | One call per language; no auto-load, no hidden `autoLoadLanguagePackages` key |
| Active theme (color/style resolution) | [`theme.setTheme`](theme/set-theme.md) | Theme `tokenType` + `modifiers` rules resolve all vocabulary token colors; no per-language color overrides |
| Engine tier override | [`syntax.setSyntaxEnginePreference`](syntax/set-syntax-engine-preference.md) | Optional explicit tier choice; native is the default for all four first-party languages |
| Typography (font families/sizes/ligatures) | [`theme.setTypography`](theme/set-typography.md) | User-owned families, sizes, and per-role ligature policy; packages declare semantic roles only and a mode's font role selects which policy applies |
| Editor behavior (indent/pairs/electric/comment/autocomplete triggers) | [`behavior.buildCodeEditingManifest`](behavior/build-code-editing-manifest.md) | Package-owned manifest declared at load time through `serverRegisterModePattern`; no per-language behavior toggle |
| Per-mode movement and caret appearance | `editorRules.movement` / `editorRules.caretStyle` via [`serverRegisterModePattern`](modes/server-register-mode-pattern.md) | Plan 071 tasks 4/6/11: package-declared inert manifest data validated server-side; absent fields fall back to the code-editing/editor defaults; no hidden keys |
| Runtime caret override | [`editor.clientSetCursorStyle`](editor/client-set-cursor-style.md) | User-level caret shape/blink override from `init.js`; takes precedence over per-mode manifest values |
| Completion keyword/snippet items | [`completion.serverRegisterCompletionProvider`](completion/server-register-completion-provider.md) | Package-owned validated static items; no user-facing keyword-list toggle or per-language item filter |
| Markdown preview/status panel visibility | [`configuration.setPackageOption`](configuration/set-package-option.md) / [`ui.serverSetLayoutOverride`](ui/server-set-layout-override.md) | Existing `markdown.layout.defaultVisibility` option and `markdown.preview` `visibility` override; no new panel-display API |
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

Phase 19 promotes exactly one built-in reload command: `runtime.reloadConfiguration` (**Reload Configuration and Packages**). There is no `clay:configuration` JS facade for reload; the command is invoked through the existing key-routing, Control Center, SDUI action, and transient-menu paths. It ships with a global `Ctrl+Shift+R` key binding; explicit configuration can override or unbind that default.

### Command metadata

| Property | Value |
|---|---|
| Command ID | `runtime.reloadConfiguration` |
| User-facing name | Reload Configuration and Packages |
| Routing policy | `ServerFirstWithLock { lock_scope: Behavior }` |
| Default key bindings | Global `Ctrl+Shift+R` |
| Permissions | None (empty) |
| Package provenance | Clay-owned built-in (`package_name = "clay"`) |
| JS facade | None — not callable from `clay:commands`, `clay:configuration`, or any package JS |

### Default and explicit binding

The global `Ctrl+Shift+R` default is available without configuration. Users may
redeclare, override, or unbind it through `bindKey`/`unbindKey`; the Control
Center also discovers the command and displays its default chord. All routes
use the same inert behavior manifest and revalidate through `CommandExecutor`
before any side effect:

```js
import { bindKey, unbindKey } from "clay:keybindings";

// Idempotently restore the shipped default:
bindKey("Ctrl+Shift+R", "runtime.reloadConfiguration", { scope: "global" });
// Or remove it and choose another chord:
unbindKey("Ctrl+Shift+R", { scope: "global" });
bindKey("Ctrl+Alt+R", "runtime.reloadConfiguration", { scope: "global" });
```

### Automatic configuration-root watch (server behavior, no JS API)

While the server runs with an effective configuration root, a bounded polling
watcher (≈1 s interval, quiet-period debounce) detects created, modified, or
deleted `.js` files and `preferences.json` anywhere under that root and
schedules the same serialized `runtime.reloadConfiguration` reload path — no
command intent or JS API is needed. There is deliberately **no** `watch*`
configuration JS API: watching is automatic server behavior. Failed reloads
keep the previous generation active and record bounded runtime diagnostics;
a successful reload re-baselines the watch snapshot so the watch never loops.

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

bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" });
```

`documents.clientOpenFileDialog` is a fixed Clay command ID that can be routed by inert behavior manifests after configuration evaluation. No default `Ctrl+O` shortcut in Rust exists; without an `init.js` binding or fixture binding, `Ctrl+O` is not treated as the open-file command.

Dialog behavior uses fixed defaults, not hidden `init.js` keys: native dialog support on Windows, Linux (xdg-desktop-portal), and macOS (`NSOpenPanel`), Markdown filters for `.md`, `.markdown`, and `.mdown`, an all-files fallback, cancellation as a non-error no-op, and selected-file-only server validation/granting through existing capability tokens. The `windows-markdown-open` development fixture uses normal package, SDUI, parse/decorations, and `bindKey` APIs; it does not introduce ad hoc keys such as dialog filters, default directories, package enablement settings, or callable client-side hooks.

Configuration remains server startup/load-time work. Pressing the configured key uses client-local manifest routing and then an explicit native UI command; ordinary keypress, paint, scroll, layout, text-event, edit acknowledgement, and Markdown decoration rendering paths do not execute configuration JavaScript. This configuration route does not grant arbitrary filesystem authority, package installation or enable/disable authority, shell, network, AI, WASM, raw Deno ops, workspace expansion, or client-side JavaScript authority.

## Phase 18.20 language-server configuration review

Phase 18.20 promotes exactly one configuration API: [`language-server.authorizeLanguageServer`](language-server/authorize-language-server.md). This is a configuration-only grant API callable only from `~/.config/clay/init.js` during configuration root evaluation **before** the first `loadPackage` call seals authority.

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
| Language-server grant | [`language-server.authorizeLanguageServer`](language-server/authorize-language-server.md) | Configuration-only; sealed before first `loadPackage` |
| Session start/read/write/stop | [`language-server.startLanguageServerSession`](language-server/start-language-server-session.md) | Runtime-backed; requires prior grant with matching contribution/root |
| Package contribution metadata | `clay.contributions.languageServers` in `package.json` | Fixed at package install time; validated descriptor with id/executable/args/inheritEnvironment |
| Workspace roots | [`workspace.clientOpenFolderDialog`](workspace/client-open-folder-dialog.md) (client UI) + server state | Roots are workspace-state metadata, not `init.js` keys |

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
| Language-server grant | [`language-server.authorizeLanguageServer`](language-server/authorize-language-server.md) | Configuration-only; sealed before first `loadPackage` |
| Package loading | [`packages.loadPackage`](packages/load-package.md) | One call per package; base before bridge |
| Completion provider disable | [`completion.serverDisableCompletion`](completion/server-disable-completion.md) | Disable per-bridge or per-package-prefix; LSP completion defaults to priority 100 non-exclusive |
| Document analyzer registration | [`language.serverRegisterDocumentAnalyzer`](language/server-register-document-analyzer.md) | Package-load-time only; called from bridge `dist/load.js`, not init.js directly |
| Language intelligence registration | [`language.serverRegisterLanguageIntelligenceProvider`](language/server-register-language-intelligence-provider.md) | Package-load-time; no separate per-server config needed |

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

LSP completion providers register at priority 100 non-exclusive, merging with the base keyword completion provider (priority 0). Use [`completion.serverDisableCompletion`](completion/server-disable-completion.md) to suppress a bridge's completion provider while keeping its diagnostics and intelligence:

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
| Markdown package options (`layout.defaultVisibility`, `layout.defaultSlot`, `layout.splitRatio`, `input.default`, `action.default`, `themeTokenRemap`, `fallback`) | [`configuration.setPackageOption`](configuration/set-package-option.md) | runtime-backed | `packagePrefix`, `option`, `value`, `source` |
| Markdown layout overrides (`slot`, `visibility`, `splitRatio`, `themeToken`, `inputDefault`, `actionDefault`, `fallback`) for targets such as `markdown.preview` | [`ui.serverSetLayoutOverride`](ui/server-set-layout-override.md) | runtime-backed | `targetId`, `property`, `value`, `source` |
| Markdown theme-token declarations (`markdown.preview.background`, `markdown.preview.padding`, `markdown.heading.1`, ...) | [`ui.serverRegisterThemeToken`](ui/server-register-theme-token.md) | runtime-backed | `token`, `type`, `fallback`, `description`, `source` |
| Markdown panel visibility defaults (e.g., `markdown.preview` with `defaultVisibility: "hidden"`) | [`ui.serverRegisterPanelContribution`](ui/server-register-panel-contribution.md) | runtime-backed | `id`, `slot`, `kind`, `defaultVisibility`, `component`, `actionTargets` |
| Markdown input routing defaults | [`ui.serverRegisterInputContribution`](ui/server-register-input-contribution.md) | runtime-backed | `id`, `scope`, `componentId`, `pointer.*`, `focus.*`, `actionTargets` |
| Markdown UI state scopes | [`ui.serverRegisterUiStateScope`](ui/server-register-ui-state-scope.md) | runtime-backed | `id`, `scope`, `owner`, `lifetime`, `persistence`, `valueSchema.kind` |
| Markdown file-dialog key binding | [`keybindings.bindKey`](keybindings/bind-key.md) | runtime-backed | `key`, `commandId`, `scope` |
| Markdown mode activation preference | `configuration.setModePreference` | planned (unavailable) | n/a |
| Markdown decoration theme preference | `configuration.setDecorationTheme` | planned (unavailable) | n/a |
| Markdown parse policy (timeout, windows, memory budget) | `configuration.setParsePolicy` | planned (unavailable); parse-handler policy fields are validated through [`parse.serverRegisterParseHandler`](parse/server-register-parse-handler.md) | n/a |
| One-line package loading | `packages.loadPackage` | implemented (Plan 029, Phase 18.6; Plan 035 generalizes to source-aware loading of bundled and installed user-authorized packages); see `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` for the unified authority model | n/a |

Markdown-specific hidden/ad hoc configuration keys that are rejected by policy and are not valid unless expressed through one of the documented APIs above with the package-owned prefix and a supported option/property name:

- `preview.position`, `preview.defaultVisibility`, `preview.slot`
- `layout.preview.defaultSlot`, `layout.preview.defaultVisibility`, `layout.preview.splitRatio`
- `theme.markdown.heading.1`, `theme.markdown.preview.background`, raw token override keys
- `markdown.sidebar.width`, ad hoc sidebar/layout/style keys
- Unregistered action keys, ad hoc input routing keys, ad hoc state-blob keys

The default Markdown load path (through `packages.loadPackage("@clay/markdown")`, implemented by Plan 029) does not publish a default side panel: the optional Markdown preview is a package `PanelContribution` with `defaultVisibility: "hidden"` targeting the `right` slot, shown only through `setPackageOption` or `serverSetLayoutOverride`. The package-owned `markdownLoadMode()` remains available as a convenience alias for per-load options. Markdown package options such as `markdown.layout.defaultVisibility` and `markdown.layout.defaultSlot` go through `configuration.setPackageOption`; Markdown layout overrides such as `markdown.preview` `visibility`/`themeToken` go through `ui.serverSetLayoutOverride`; Markdown theme tokens go through `ui.serverRegisterThemeToken`. None of these APIs grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw Deno ops / raw ops, native widget handles, direct Masonry widgets, raw CSS, renderer callbacks, or client-side JavaScript authority.

Configuration evaluation for Markdown end-user loading remains startup, package-load, configuration-change, or explicit setting-change work only. Markdown keypress, paint, scroll, layout, text-event, edit acknowledgement, parse-result publication, and decoration rendering paths do not execute configuration JavaScript, recompute package options from user code, or mutate native layout from package code.

## Example Configuration

```js
// ~/.config/clay/init.js
import { loadConfigurationModule } from "clay:configuration";
import { bindKey } from "clay:keybindings";
import { defineEditorView, defineFlex, definePanel, publishTree } from "clay:sdui";

await loadConfigurationModule({ path: "./keys.js" });

bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });

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

## Phase 20.1 UI design language, typed token catalog, and typography hierarchy configuration review

Phase 20.1 expanded the typed token catalog from five domains to ten and added user-owned typography hierarchy scales, all additively through existing Clay JS APIs. No new `clay:configuration` API was promoted; no hidden configuration key was introduced.

### What changed

- **Typed token catalog**: five new domains (`dimension`, `elevation`, `motion-duration`, `z-level`, `density`) joined the lexical five (`color-role`, `spacing`, `radius`, `typography`, `opacity`) in `ThemeTokenType`. The core fallback catalog grew from ~21 to 61 tokens additively; no legacy token was renamed or repurposed.
- **Typography hierarchy**: seven semantic `UiTextVariant` tokens (`typography.body`, `typography.title`, `typography.status`, `typography.display`, `typography.section`, `typography.detail`, `typography.caption`) with user-owned `UiTypographyHierarchy` scale ratios, delivered atomically through the existing [`theme.setTypography`](theme/set-typography.md) API via an optional `hierarchy` object. Omission preserves Clay defaults; partial hierarchies are rejected atomically.
- **Typed UI design-token overrides**: `ActiveTheme` gained a `design_tokens` field carrying validated typed UI overrides (dimension, elevation, motion-duration, z-level, density, color-role, spacing, radius, opacity) from `clay.contributions.designTokens`. These are validated server-side against core token types and domain bounds, then resolved client-side into `ResolvedUiTheme` — a cached registry serving paint/layout hot paths with no per-frame parsing or IPC. Phase 24.4 adds three core tokens overridable the same way: `surface.scrim`, `opacity.scrim`, and `dimension.overlay.centered.width` (the centered Command Centre surface is Clay-internal; only its token values are customizable, and invalid types/values fail closed before install).
- **Token-backed panel/sidebar/density defaults**: the legacy hardcoded panel/sidebar dimension constants and density default moved behind typed tokens (`dimension.sidebar.default`, `dimension.panel.side.*`, `dimension.panel.vertical.*`, `density.default`), resolved through `ResolvedUiTheme::panel_defaults()` and `ResolvedUiTheme::density()`. Dimension ordering is validated with fallback to Clay constants on invalid order; density scales only the token-owned UI spacing rhythm (Phase 20.4 component uplift), never panel dimensions or document typography.

### Configuration surfaces

| Behavior | API / surface | Notes |
|---|---|---|
| Theme selection with typed UI overrides | [`theme.setTheme`](theme/set-theme.md) | `ActiveTheme` now carries `design_tokens` alongside `textStyles`; existing Gruvbox themes unchanged |
| Typography profiles and hierarchy | [`theme.setTypography`](theme/set-typography.md) | Optional `hierarchy` object with seven bounded scale fields; omission = defaults |
| Package theme token declarations | [`ui.serverRegisterThemeToken`](ui/server-register-theme-token.md) | Now accepts all ten token types; fallback must be same-typed Clay core token |
| Package design-token overrides | `clay.contributions.designTokens` (manifest) | Ship typed UI overrides inside theme packages; validated against core types and bounds |
| Panel layout / density settings | **Deferred beyond Phase 20.6** | No `init.js` API exists for panel-size, density, elevation-level, motion-duration, or z-level preferences; density defaults flow through theme `designTokens` |
| Live appearance mode (light/dark) | [`theme.setAppearance`](theme/set-appearance.md) | Phase 20.6: bounded `light`\|`dark`\|`system` enum drives the canonical Modus Operandi/Vivendi default; explicit `setTheme` always wins; `system` follows the OS signal with a dark fallback. See [Phase 20.6 precedence and persistence](#phase-206-themetypographyappearance-precedence-and-persistence) |

### Rejected hidden configuration keys

No hidden JSON/TOML/ad hoc keys are valid for Phase 20.1 token, hierarchy, or layout behavior. Rejected examples include:

- `tokens.dimension.sidebar`, `design.sidebar.px`, `sidebarPixelWidth`, `panelSideDefaultPx`
- `hierarchy.display`, `hierarchy.title`, `hierarchy.section`, `hierarchy.body`, `hierarchy.status`, `hierarchy.detail`, `hierarchy.caption` when expressed as top-level `init.js` keys outside `setTypography`
- `density`, `density.level`, `density.compact`, `density.spacious`, `density.scale`
- `elevation.raised`, `motion.fast`, `z.overlay`, `z.table` — must go through `designTokens` in a theme package, not `init.js` configuration
- `fontScale`, `typography.hierarchy.display`, `typography.scale`, `typeScale`, `typeScaleRatio`
- Raw panel dimension keys (`leftPanelWidth`, `rightPanelWidth`, `sidebar.width`, `sidebarWidth`, `panel.side.default`, `panel.vertical.default`)
- Ad hoc theme token override keys, raw color injection, inline CSS blobs, or renderer callback hooks

All token-backed values (panel dimensions, density, elevation, motion, z-level) are resolved from `ActiveTheme.design_tokens` or core fallbacks. The user-owned hierarchy is configured through `setTypography`. Theme selection remains `setTheme`. No new parallel API, JSON key, TOML key, or environment variable exists for these values.

### Compiled budgets (not configurable)

| Budget | Constant | Value |
|---|---|---|
| Max dimension | `MAX_DIMENSION_PX` | 8192 |
| Max motion duration | `MotionDuration::MAX_MILLIS` | 1000 ms |
| Max hierarchy scale | `HIERARCHY_SCALE_MAX` | 4.0 |
| Typography payload budget | `TYPOGRAPHY_PAYLOAD_BUDGET_BYTES` | 1024 |

These are compiled security/performance boundaries, not `init.js` keys.

### Security

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work only. Ordinary keypress, paint, layout, scroll, pointer, text-event handling, edit acknowledgement, parse-result publication, decoration rendering, and completion result construction do not execute configuration JavaScript, compute token overrides, or revalidate UI design tokens. `ResolvedUiTheme` is a cached read-only registry installed at theme/configuration time; its hot-path accessors perform no parsing, IPC, or JavaScript.

This review grants no filesystem, network, shell, package-manager, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw-op, native-widget, client-side JavaScript, CSS, renderer callback, raw value injection, or token-mutation authority. Typography hierarchy scales remain user-owned; packages cannot supply concrete hierarchy values even through `designTokens`.

## Historical Plan 035 unified package authority configuration review (superseded)

> **Superseded by Plan 061 and the Plan 060 review below.** This section records the earlier single-runtime `RuntimeProfile` design only. Current trust classification uses two fixed runtime domains, exact bundled provenance/integrity, and durable out-of-band third-party adoption; normal configuration cannot select a runtime profile, promote a package, or authorize capabilities.

Plan 035 unified first-party and third-party package authority. The configuration-relevant surfaces are user authorization/capability grants, runtime profile choices, package graph relations, package-control overrides, and conflict resolution overrides. Each must be a documented Clay JS/config API or explicitly documented CLI/UI state — never a hidden JSON/TOML/ad hoc key.

Unified package authority configuration surfaces:

| Surface | Current status | Surface / API | Custom properties |
|---|---|---|---|
| User authorization / capability grants | Rust primitive implemented (`PackageService::authorize_package`); no callable end-user JS/CLI surface yet — planned | `packages.authorize` (planned inventory entry, `registry_public = false`) | `package`, `capabilities`, `runtimeProfile`, `source`, `approvedBy` |
| Runtime profile selection | Rust primitive implemented (`RuntimeProfile` enum); bound to the authorization grant | `packages.authorize` `runtimeProfile` parameter | `runtimeProfile:enum=native-trust\|sandboxed\|restricted` |
| User conflict override | Rust primitive implemented (`PackageService::set_conflict_override`); no callable end-user JS/CLI surface yet — planned | `packages.setConflictOverride` (planned inventory entry, `registry_public = false`) | `contributionId`, `winnerPackage` |
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

Until the `packages.authorize` and `packages.setConflictOverride` facades/CLI commands ship, the only current grant path is the bundled-package auto-authorization inside the resolver, plus the Rust primitives exercised by tests. A user-installed package that requests a powerful capability (filesystem, network, shell, wasm, ai-tools, workspace-mutation, native-ui, client-runtime, raw-ops, package-control) cannot currently be granted through an end-user surface — enable fails closed with `MissingCapabilityGrant`. This is a documented implementation gap, not a hidden configuration shortcut; there is no `allowThirdPartyPackages`, `trusted`, `grant`, or capability-grant JSON/TOML key in `init.js`.

`@clay/*` means shipped by Clay, not more capable. The configuration surfaces above apply identically to bundled and user-installed packages; no config primitive branches on package source. Capability grants can grant powerful capabilities only through the explicit authorization flow above, with provenance (package identity/source/version/integrity), visibility (inspectable grants), and revocation (`disable`/`revoke` withdraws the grant and its contributions through `PackageRevocationRecord`).

Configuration evaluation for unified package authority is startup/install/enable/load/reload/explicit-user-command work only. Grant lookup at the enable/load/registration/request boundary is a cheap check against already-loaded authorization state. No source resolution, package-manager call, authorization prompt, grant recording, graph traversal, conflict resolution, or capability evaluation runs from keypress, paint, layout, scroll, text-event, edit-ack, pointer, or Masonry hot paths. See `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` for the authority model.

## Plan 030 security budgets are intentionally not Clay JS APIs

Plan 030 (code-review remediation) hardened several server-side limits. These are **security boundaries**, not user configuration, so they are intentionally **not** exposed as `clay:configuration` APIs and cannot be raised, lowered, or disabled from `init.js`. Raising any of them from user JavaScript would undermine the very boundary it enforces (e.g. a malicious `init.js` could lift the JS evaluation timeout to defeat the watchdog, or raise the openable-file ceiling to exhaust memory). They are compiled into the server binary in `src/perf/budgets.rs` and reviewed through code review and decisions rather than tuned at runtime.

- **JS runtime evaluation timeout** — `JS_RUNTIME_EVALUATION_TIMEOUT_MS` (5000 ms, default). A watchdog thread terminates the V8 isolate when the budget elapses; surfaced as `runtime.timeout`. Not configurable from `init.js`.
- **JS runtime heap limit** — `JS_RUNTIME_HEAP_LIMIT_BYTES` (128 MiB). The persistent runtime is created with `v8::CreateParams::heap_limits`; the near-heap callback terminates execution and surfaces `runtime.heap_limit`. Not configurable from `init.js`.
- **Openable file size** — `MAX_OPENABLE_FILE_BYTES` (768 KiB). Server-side file-open path rejects files above this before allocating full text, with headroom under the 1 MiB codec frame limit. Not configurable from `init.js`.
- **Runtime SDUI tree budgets** — `RUNTIME_SDUI_TREE_PAYLOAD_BUDGET_BYTES` (16 KiB), `RUNTIME_SDUI_TREE_MAX_NODES` (128), `RUNTIME_SDUI_TREE_MAX_DEPTH` (16), `RUNTIME_SDUI_TREE_MAX_NODE_TEXT_CHARS` (4096). Enforced before/during `op_clay_sdui_publish_tree`; rejected with `sdui.invalid_tree`. Not configurable from `init.js`.
- **Large-file resident memory budget** — `LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB` (256 MiB). Resident-memory ceiling for editor caches; not a per-open tunable.
- **Package install lifecycle-script suppression** — `pnpm add` runs with `--ignore-scripts` by default. The opt-in is a **CLI flag / env var**, not a Clay JS API: `clay package add --allow-scripts` or `CLAY_ALLOW_LIFECYCLE_SCRIPTS=1`. This is a process-level supply-chain control, not an `init.js` configuration option, and is documented in `docs/reference/primitives/package-loading.md`.
- **File-open capability gate** — `OpenSelectedFile` requires a server-minted single-use capability token issued after the `Hello` handshake; not a configuration option. See `docs/wiki/modules/server-ipc-skeleton.md`.
- **IPC endpoint ownership/permissions** — Unix socket `0o600` + parent-directory ownership and Windows named-pipe current-user-only DACL are OS-level hardening, not Clay JS configuration.

## Plan 034 persistent-runtime hardening is intentionally not configurable

Plan 034 added first-party runtime hardening and a minimal separate-process sandbox harness. These controls are server-owned security boundaries, not user customization. They do **not** promote a new `clay:configuration` API, hidden `init.js` key, JSON/TOML setting, command-line user preference, package option, or package-declared permission that can weaken the runtime boundary.

- **Heap guard** — `JS_RUNTIME_HEAP_LIMIT_BYTES` remains a compiled budget. `runtime.heap_limit` is a diagnostic code, not a callable configuration API.
- **Timeout guard** — `JS_RUNTIME_EVALUATION_TIMEOUT_MS` remains a compiled budget. `runtime.timeout` is a diagnostic code, not a mutable setting.
- **Sandbox supervision** — sandbox child spawn, handshake, payload budget, timeout kill, and restart policy are internal supervisor behavior. There is no `setSandboxDisabled`, `setSandboxTimeout`, or `enableSandboxBypass` configuration surface.
- **Denied authorities** — filesystem, network, shell, WASM, AI mutation, package-manager execution, native-widget handles, raw-op access, and client-side JavaScript remain powerful capabilities that require explicit user-authorized grants under the unified package authority model. They are not categorically denied for non-`@clay/*` packages, but they are never granted implicitly from `init.js`; they flow only through the documented [`packages.authorize`](#plan-035-unified-package-authority-configuration-review) surface with provenance and revocation.
- **Third-party execution gate** — non-`@clay/*` packages now load through the same source-aware `loadPackage` resolver as bundled packages after install and user authorization (see Plan 035). There is no `enableThirdPartyPackages` or `allowThirdPartyPackages` configuration shortcut; capability grants are explicit, visible, and revocable per-package.

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work. Ordinary keypress, paint, layout, scroll, edit acknowledgement, text-event handling, parse-result publication, and decoration rendering paths do not execute configuration JavaScript, wait on sandbox round trips, or re-check runtime hardening knobs.


## Phase 18.8 command execution and transient menu configuration review

Phase 18.8 added the server-owned `CommandExecution` boundary, the generic `TransientMenuSession` state model, and the first Control Center consumer. This review did **not** promote a new user-facing `clay:configuration` API for menu placement, control-center behavior, command filtering, default key bindings, or package action customization. The user-visible configuration surfaces reuse the existing Clay JS APIs; menu/session internals are kept `pub(crate)`/internal.

User-visible Phase 18.8 configuration surfaces:

| Surface | Status | API / mechanism | Notes |
|---|---|---|---|
| Control Center launch key binding | reused, runtime-backed | [`keybindings.bindKey`](keybindings/bind-key.md) | Bind a key to the built-in command `controlCenter.open`; a default `Ctrl+X Ctrl+P` chord ships in the default behavior manifest and is fully overrideable/removable via `bindKey`/`unbindKey` |
| Control Center command id | built-in server command | `controlCenter.open` (registered through `builtin_server_command`, `RoutingPolicy::ServerFirst`) | A fixed Clay command ID routed by inert behavior manifests after configuration evaluation; not an `init.js` key |
| Built-in server commands (`workspace.refresh`, `document.focus_active`, `document.open_recent`) | built-in server command | `builtin_server_command_ids` / `builtin_server_command` | Fixed Clay command IDs, not user configuration |
| Package command/action customization | reused, runtime-backed | [`commands.serverRegisterCommand`](commands/server-register-command.md), [`ui.serverRegisterPanelContribution`](ui/server-register-panel-contribution.md), [`ui.serverRegisterInputContribution`](ui/server-register-input-contribution.md), [`configuration.setPackageOption`](configuration/set-package-option.md) | Package commands, action targets, and `action.default`/`input.default` overrides flow through phase 18.3/18.4 package UI/configuration APIs |
| Transient menu session state | internal | `TransientMenuSession` (`src/shell/transient_menu.rs`) | Clay-owned session state: prompt, query, bounded items, selection, status, focus policy, inert activation actions; internal Rust type, not user configuration |
| Control Center menu building | internal | `ControlCenter` (`src/server/control_center.rs`, `pub(crate)`) | Builds the bounded `TransientMenuSession` from the generation-stamped command catalogue; excludes client-first edit commands only; not user configuration |
| Command execution validation | internal | `CommandExecutor` (`src/server/command_execution.rs`) | Validates command id, routing policy, provenance, permissions, argument budget, target context, and session/action freshness per request; internal Rust type, not user configuration |

The expected end-user Control Center configuration is a normal `~/.config/clay/init.js` binding:

```js
import { bindKey, unbindKey } from "clay:keybindings";

// Remove the shipped Ctrl+X Ctrl+P default, then bind a different chord
// (single-stroke or multi-stroke, e.g. "Ctrl+X Ctrl+P" or "Alt+X").
unbindKey("Ctrl+X Ctrl+P", { scope: "global" });
bindKey("Alt+X", "controlCenter.open", { scope: "global" });
```

`controlCenter.open` is a fixed Clay command ID routed by inert behavior manifests. Phase 24.5 ships the default `Ctrl+X Ctrl+P` chord (Global scope, `ServerFirst` routing; the pre-24.5 single-stroke default was `Ctrl+Shift+P`) in the default behavior manifest; `bindKey`/`unbindKey` can rebind or remove it — without an explicit unbind the default remains bound. `bindKey` is the documented configuration surface — the transient menu is not a callable `clay:configuration` API and cannot be styled, positioned, filtered, or dismissed through `init.js`. Menu geometry, item count limit (`MAX_ITEMS = 256`), query/label/detail/accessibility bounds, focus policy, fuzzy matcher constants, and built-in command membership are Clay-owned compiled/internal constants, not hidden `init.js` keys.

## Phase 24.3 path mode configuration review

Phase 24.3 added the Path Browser (`controlCenter.openPath`, “Browse Filesystem”): a second built-in consumer of the Phase 24.1/24.2 transient-menu round trip that browses user-authorized filesystem paths with dired-style navigation. This review added **no** new `clay:configuration` API. The user-visible configuration surface reuses [`keybindings.bindKey`](keybindings/bind-key.md)/`unbindKey` for the launch route; the browse session, listing, seed resolution, and grant conversion are `pub(crate)` Rust internals with no Clay JS facade and no raw `Deno.core.ops` path.

| Surface | Status | API / mechanism | Notes |
|---|---|---|---|
| Path Browser launch key binding | reused, runtime-backed | [`keybindings.bindKey`](keybindings/bind-key.md) | Bind a key to the built-in command id `controlCenter.openPath`; a default `Ctrl+X Ctrl+F` chord (Phase 24.5 sequence default, Global scope, `ServerFirst` routing) ships in the default behavior manifest and is fully overrideable/removable via `bindKey`/`unbindKey` without changing the id |
| Path Browser command id | built-in server command | `controlCenter.openPath` (`CommandDeclaration::server_intent`, `RoutingPolicy::ServerFirst`) | A fixed Clay command ID routed by inert behavior manifests; not an `init.js` key; the bare id is valid, `clay.controlCenter.openPath` is never valid |
| Browse listing and session | internal | `BuiltInUserBrowseListing` (`src/server/workspace.rs`), `PathBrowserSession` (`src/shell/path_browser.rs`), `ServerMenuSessions` (`src/server/menu_sessions.rs`) | Clay-owned bounded depth-1 listings and session state; packages cannot open, populate, intercept, or receive paths from the session |
| Browse authority conversion | internal | activation → `SingleFile` / `Directory` grant | Ephemeral user-authorized browse authority converts into exactly one explicit grant on file open / Alt+Enter workspace open; navigation alone creates no grant; native dialogs remain the fallback capability issuers |

```js
import { bindKey, unbindKey } from "clay:keybindings";

// Remove the shipped Ctrl+X Ctrl+F default, then bind a different chord
// (single-stroke or multi-stroke, e.g. "Ctrl+X Ctrl+F" or "Alt+P").
unbindKey("Ctrl+X Ctrl+F", { scope: "global" });
bindKey("Alt+P", "controlCenter.openPath", { scope: "global" });
```

`controlCenter.openPath` is a fixed Clay command ID routed by inert behavior manifests. The Path Browser path input, listing bounds (`TRANSIENT_MENU_MAX_ITEMS`, `TRANSIENT_MENU_MAX_QUERY_CHARS`), fuzzy matcher constants, seed fallback order, and grant conversion rules are Clay-owned compiled/internal constants, not hidden `init.js` keys. Hidden/ad hoc configuration keys that would claim to configure path mode are rejected by policy unless expressed through the documented APIs above.

Hidden/ad hoc configuration keys that are rejected by policy and are not valid unless expressed through a documented API above:

- `controlCenter.key`, `controlCenter.defaultKey`, `controlCenter.shortcut`
- `menu.position`, `menu.alignment`, `menu.maxItems`, `menu.height`, `menu.width`
- `transientMenu.focusPolicy`, `transientMenu.maxItems`, `transientMenu.queryCharLimit`
- `commandExecution.timeout`, `commandExecution.argumentBudget`, `commandExecution.allowBypass`
- `builtins.controlCenter`, ad hoc built-in command injection keys
- Unregistered command ids bound to keys, ad hoc package action routing keys, ad hoc menu filter keys

Package command/action registration through [`commands.serverRegisterCommand`](commands/server-register-command.md) declares routing policy, permissions, key bindings, custom properties, and lookup tags at package-load time; it does not grant execution authority. Command execution authority is re-validated per activation through `CommandExecutor` and never granted by registration, menu inclusion, or configuration. Packages may declare commands and expose them in transient menus; they cannot execute commands directly from UI callbacks, bypass command permission/provenance validation, run command handlers in the Rust client, or grant themselves filesystem, network, shell, AI mutation, WASM, workspace mutation, package-manager, package installation, package enable/disable, native widget, raw-op, or client-side JavaScript authority.

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work only. Command registration, action validation, transient menu filtering over installed bounded metadata, and command-id binding through `bindKey` are load/configuration/update-time work. Activating a selected command enqueues a server-first `CommandExecution` request; ordinary keypress routing, Masonry paint/layout, pointer, scroll, text-event handling, edit acknowledgement, and decoration rendering paths do not execute configuration JavaScript, wait on IPC, recompute package action defaults from user code, or run command handlers. This review adds no filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw-op, client-side JavaScript, executable callback, or command-authority grant.

## Phase 18.11 completion provider configuration review

Phase 18.11 added the `CompletionTriggerAndResult` primitive, the server-side completion provider framework, the built-in `core.bufferWords` provider, and `TransientMenuSession`-based completion display/acceptance. This review did **not** promote a new user-facing `clay:configuration` API for provider priority, provider enable/disable, trigger characters, buffer-word limits, completion menu placement, commit behavior, or result item budgets. The user-visible configuration surfaces reuse the existing Clay JS APIs; provider/coordinator/menu/acceptance internals are kept `pub(crate)`/internal.

User-visible Phase 18.11 configuration surfaces:

| Surface | Status | API / mechanism | Notes |
|---|---|---|---|
| Manual completion trigger key binding | reused, runtime-backed | [`keybindings.bindKey`](keybindings/bind-key.md) | Bind a key to the built-in `UiReactivePriority` command `completion.trigger`; no default chord exists in Rust, so manual completion is only reachable when `init.js` binds a key (e.g. `Ctrl+Space`) |
| Completion trigger command id | built-in server command | `completion.trigger` (registered through `CommandDeclaration::ui_reactive`, `RoutingPolicy::UiReactivePriority`) | A fixed Clay command ID routed by inert behavior manifests after configuration evaluation; not an `init.js` key |
| Autocomplete trigger characters | package manifest metadata | `clay.contributions.autocompleteTriggers` | Inert single-character manifest entries classified locally by `ClientBehaviorState`; not user configuration |
| Completion provider metadata registration | runtime-backed package load entry | [`completion.serverRegisterCompletionProvider`](completion/server-register-completion-provider.md) | Package-prefixed provider id, priority, inert trigger characters, inert word-boundary chars, bounded `timeoutMs`/`maxItems`; metadata-only in Phase 18.11 |
| Completion provider enable/disable | package load/disable | `packages.loadPackage` / `PackageService` disable | Provider enablement is tied to package load/disable, not a hidden config key; the built-in `core.bufferWords` provider is always available and is not removed by package disable/reload |
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

Package completion provider registration through [`completion.serverRegisterCompletionProvider`](completion/server-register-completion-provider.md) declares package-prefixed provider id, priority, inert trigger characters, inert word-boundary chars, and bounded `timeoutMs`/`maxItems` at package-load time; it does not grant execution authority. Phase 18.11 is metadata-only: Clay rejects `handler`/`callback`/`complete`/`function`/`module` executable values, raw ops, native handles, client JavaScript, snippets/commands, URLs, shell/network/AI/WASM/native/package-manager fields, duplicate ids, reserved `clay.*` ids, and oversize metadata. Providers may read only Clay-provided open-document content/windows; completion grants no filesystem, network, shell, AI mutation, extension loading, workspace mutation, package enable/disable, WASM, raw-op, native-widget, client-JS, or provider execution authority without later documented APIs and an approved decision log.

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work only. Provider metadata registration, trigger classification over installed inert manifest state, completion request enqueueing through a bounded non-blocking channel, and command-id binding through `bindKey` are load/configuration/update-time work. Provider execution runs server-side on a cancellable `UiReactivePriority` lane that aborts or stale-drops older in-flight requests and validates results against the current document/behavior version and provider generation before publication; ordinary keypress routing, local text mutation, Masonry paint/layout, pointer, scroll, text-event handling, edit acknowledgement, and decoration rendering paths do not execute configuration JavaScript, wait on IPC, run provider code, recompute provider metadata from user code, or mutate native layout from package code. This review adds no filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw-op, client-side JavaScript, executable callback, or provider-authority grant.

## Phase 18.12 workspace file-browser configuration review

Phase 18.12 added server-owned workspace-root discovery, bounded directory listing, a Clay-owned left file-browser panel, a bottom transient fuzzy-open route, and server-authoritative open/reveal command routing. This review did **not** promote a new user-facing `clay:configuration` API for file-browser visibility, file-browser slot placement, fuzzy-open key binding defaults, workspace root markers, ignore-list overrides, listing depth/count limits, tree refresh policy, reveal behavior, or raw file-open paths. User-visible configuration reuses existing Clay JS APIs: `keybindings.bindKey` for command chords and the Phase 18.12 `clay:workspace` / `clay:commands` APIs for explicit workspace actions. Phase 20 daily-editing chords (cut/paste/undo/redo/save/open-documents/recovery) are documented in the [Phase 20 configuration review](#phase-20-daily-editing-product-hardening-configuration-review).

User-visible Phase 18.12 configuration surfaces:

| Surface | Status | API / mechanism | Notes |
|---|---|---|---|
| Fuzzy-open key binding | reused, runtime-backed | [`keybindings.bindKey`](keybindings/bind-key.md) | Bind a key to the built-in server-first command `workspace.openFuzzyFile`; no default chord exists in Rust, so fuzzy open is only reachable when `init.js` binds a key or another Clay-owned action opens it |
| File-browser toggle key binding | reused, runtime-backed | [`keybindings.bindKey`](keybindings/bind-key.md) | The canonical `init.js` example binds `Ctrl+B` to `workspace.toggleFileBrowser`; the command is validated by `CommandExecutor` and flips visibility only for the calling tab |
| Native folder picker binding | reused, runtime-backed | [`keybindings.bindKey`](keybindings/bind-key.md), `workspace.clientOpenFolderDialog` | Bind a key to the fixed client UI command id; native selection still goes through selected-path capability and server root validation |
| Copy current selection binding | reused, runtime-backed | [`keybindings.bindKey`](keybindings/bind-key.md), `editor.clientCopySelection` | Bind an alternate key to copy the current native editor selection |
| File open/reveal commands | runtime-backed command APIs | [`commands.serverOpenFile`](commands/server-open-file.md), [`commands.serverRevealInTree`](commands/server-reveal-in-tree.md), [`commands.serverExecuteCommand`](commands/server-execute-command.md) | Open and reveal route through server workspace APIs, root-relative paths, selected-file grants, and open-document metadata validation |
| Workspace roots and discovery | runtime-backed workspace APIs | [`workspace.serverAddWorkspaceRoot`](workspace/server-add-workspace-root.md), [`workspace.serverDiscoverWorkspaceRootForPath`](workspace/server-discover-workspace-root-for-path.md), [`workspace.serverListWorkspaceRoots`](workspace/server-list-workspace-roots.md) | Roots and grants are explicit server-authoritative workspace APIs, not configuration keys |
| Directory listing | runtime-backed workspace APIs | [`workspace.serverListDirectory`](workspace/server-list-directory.md), [`workspace.serverCreateListingCancelToken`](workspace/server-create-listing-cancel-token.md), [`workspace.serverCancelListing`](workspace/server-cancel-listing.md) | Listing uses server validation, bounded depth/count, compiled ignore defaults, optional cancellation tokens, and diagnostics |
| Left file-browser panel visibility/slot | Clay-owned shell state | `src/server/mod.rs::TabServerState`; `src/shell/file_browser.rs::FileBrowserState`; `FixedSlotId::Left` via SDUI composition | Hidden by default per tab; `Ctrl+B` publishes an inert editor-only tree when hidden and the bounded file tree when shown. The first-party left panel is Clay-owned UI, not a configurable slot |
| Marker file set | compiled workspace boundary | `KNOWN_PROJECT_MARKERS` in `src/server/workspace.rs` | Closed Clay-owned marker table (`.git`, `Cargo.toml`, `package.json`); packages/users cannot extend it through `init.js` |
| Ignore defaults and list budgets | compiled listing boundary | `DEFAULT_IGNORED_NAMES`, `MAX_LIST_DIRECTORY_DEPTH`, `MAX_LIST_DIRECTORY_ENTRIES`, `MAX_LEFT_PANEL_ENTRIES`, `MAX_FUZZY_ITEMS` | Bounded security/performance constants, not hidden `init.js` keys |

The expected end-user fuzzy-open configuration is a normal `~/.config/clay/init.js` binding:

```js
import { bindKey } from "clay:keybindings";
import { clientCopySelection } from "clay:editor";
import { clientOpenFolderDialog } from "clay:workspace";

bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
bindKey("Ctrl+P", "workspace.openFuzzyFile", { scope: "editor" });
bindKey("Ctrl+B", "workspace.toggleFileBrowser", { scope: "editor" });
bindKey("Ctrl+Shift+C", clientCopySelection(), { scope: "editor" });
```

`workspace.openFuzzyFile` and `workspace.toggleFileBrowser` are fixed Clay command IDs validated by `CommandExecutor`. The canonical `examples/init.js` binds `Ctrl+B` to the toggle; the pane remains hidden when no binding is installed and visibility is retained per tab. `workspace.clientOpenFolderDialog` and `editor.clientCopySelection` are fixed client UI command IDs returned by synchronous Clay JS helpers. No default `Ctrl+P` shortcut in Rust exists for fuzzy open, and no default `Ctrl+Shift+O` or `Ctrl+Shift+C` shortcut exists for folder/copy workflow routes. Native copy (`Ctrl/Cmd+C`) is handled directly by the editor. `bindKey` is the documented configuration surface — the file-browser panel, fuzzy-open menu, workspace discovery scanner, directory listing service, ignore set, marker set, listing budgets, folder-picker backend, and clipboard backend are not callable `clay:configuration` APIs and cannot be styled, repositioned, resized, widened, filtered, granted extra workspace authority, or expose package/configuration/AI clipboard-contents APIs through `init.js`.

Hidden/ad hoc configuration keys that are rejected by policy and are not valid unless expressed through a documented API above:

- `fileBrowser.defaultVisibility`, `fileBrowser.visible`, `fileBrowser.leftPanelDefault`, `workspace.fileBrowser.leftPanelDefault` (visibility is fixed hidden-by-default; bind the command instead)
- `fileBrowser.slot`, `fileBrowser.position`, `fileBrowser.width`, `workspace.fileBrowser.width`
- `fuzzyOpen.key`, `fuzzyOpen.defaultKey`, `fileBrowser.fuzzyOpenKey`, `workspace.fuzzyOpenKey`
- `workspace.markers`, `workspace.markerFiles`, `workspace.rootMarkers`, `workspace.discoveryDepth`
- `workspace.ignore`, `workspace.ignoreRules`, `fileBrowser.ignore`, `fileBrowser.exclude`
- `fileBrowser.maxDepth`, `fileBrowser.maxEntries`, `fileBrowser.maxItems`, `fileBrowser.refreshInterval`
- `workspace.rawPath`, `workspace.allowArbitraryPath`, `workspace.allowOutsideRoot`, ad hoc selected-file/folder grant keys
- `clipboard.text`, `clipboard.writeText`, `clipboard.readText`, `copySelection.text`, arbitrary clipboard strings, package/config clipboard-contents keys

File-browser listing/open/reveal authority is server-owned. Root discovery scans only bounded ancestry with a closed marker set; directory listing stays inside known roots and uses bounded ignore/depth/count limits; open file commands route through `WorkspaceState::open_existing_file` or selected-file grants through `WorkspaceState::open_selected_file`; reveal validates open document metadata. Configuration cannot grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, raw-op, native widget, direct Masonry widget, arbitrary root marker, arbitrary ignore-rule, arbitrary path passthrough, or client-side JavaScript authority.

## Phase 22.8 per-tab workspace configuration verification

Phase 22.8 adds no new `clay:configuration` export, hidden config key, or workspace-root option. New-tab folder selection reuses [`shell.clientTabNew`](shell/client-tab-new.md) and the existing `bindKey` API; the picked folder is bound during the tab handshake, while the per-tab workspace and welcome document remain server-owned. The workspace pane's hidden-by-default state is also Clay-owned per-tab state: `Ctrl+B` is the canonical `bindKey` example for `workspace.toggleFileBrowser`, not a `fileBrowser.visible` or `workspaceRoot` setting.

The canonical `examples/init.js` contains one active `Ctrl+B` binding. Users may override or remove it, but cannot configure pane slot/width, workspace marker/ignore rules, listing budgets, or an arbitrary tab/root selector through `init.js`. Per-tab `workspaceRoot` persistence belongs to client-owned `layout.json`, not the configuration API. Configuration evaluation remains startup/reload work; keypress routing consumes the validated inert binding and does not evaluate JavaScript or perform filesystem work.

## Phase 20 daily editing product hardening configuration review

Phase 20 (plan `plans/055-Phase20-Daily-Editing-Product-Hardening.md`) ships clipboard cut/paste, inverse-edit undo/redo, IME preedit, multi-document retain/switch, save/conflict recovery menus, pending-edit/disconnect/resync recovery chrome, cross-platform file-open dialogs, and accessibility/theme polish. This review did **not** promote a new user-facing `clay:configuration` API. Every user-visible Phase 20 behavior reuses existing Clay JS command helpers plus [`keybindings.bindKey`](keybindings/bind-key.md). Command helpers keep empty `custom_properties` because there are no user-tunable setting fields — only fixed command IDs.

### User-visible Phase 20 configuration surfaces

| Surface | Status | API / mechanism | Notes |
|---|---|---|---|
| Open Markdown file dialog | reused, runtime-backed | [`bindKey`](keybindings/bind-key.md), [`documents.clientOpenFileDialog`](documents/client-open-file-dialog.md) | No default `Ctrl+O` in Rust; native dialogs on Windows, Linux (xdg-desktop-portal), and macOS (`NSOpenPanel`) use fixed Markdown/all-files filters |
| Save active document | reused, runtime-backed | [`bindKey`](keybindings/bind-key.md), [`documents.serverSaveDocument`](documents/server-save-document.md) | Recommended `Ctrl+S` binding; client intercepts the intent and enqueues `SaveDocument`; dirty chrome + stale-metadata recovery stay Clay-owned |
| Reload active document | reused, runtime-backed | [`bindKey`](keybindings/bind-key.md), [`documents.serverReloadDocument`](documents/server-reload-document.md) | Optional binding; dirty-reload conflicts open Clay-owned recovery menus |
| Cut current selection | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`editor.clientCutSelection`](editor/client-cut-selection.md) | Alternate chord; native `Ctrl/Cmd+X` remains editor-handled |
| Paste clipboard text | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`editor.clientPasteClipboard`](editor/client-paste-clipboard.md) | Alternate chord; native `Ctrl/Cmd+V` remains editor-handled |
| Undo latest local edit | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`editor.clientUndo`](editor/client-undo.md) | Alternate chord; native `Ctrl/Cmd+Z` remains editor-handled |
| Redo latest undone edit | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`editor.clientRedo`](editor/client-redo.md) | Alternate chord; native `Ctrl/Cmd+Shift+Z` / non-macOS `Ctrl+Y` remain editor-handled |
| Open-documents switcher | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`editor.clientShowOpenDocuments`](editor/client-show-open-documents.md) | Opens on the focused pane; lists every pane's open document plus retained sessions and activates one locally (cross-pane entries focus the owner); no tabstrip configuration API |
| Request resync | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`editor.clientRequestResync`](editor/client-request-resync.md) | Enqueues `RequestResync` for the active document |
| Dismiss recovery chrome | runtime-backed | [`bindKey`](keybindings/bind-key.md), [`editor.clientDismissRecovery`](editor/client-dismiss-recovery.md) | Clears disconnect/rejection recovery menus and sanitized diagnostics |
| Theme selection | reused (Phase 18.15) | [`theme.setTheme`](theme/set-theme.md) | Phase 20 does not rebuild themes; only verifies contrast/status-label polish |

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
bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
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
- `accessibility.labelTemplate`, `status.dirtyMarker`, theme rebuild keys that bypass `theme.setTheme`

### Security

Configuration evaluation remains startup, package-load, reload, or explicit setting-change work only. Ordinary keypress routing, Masonry paint/layout, pointer, scroll, text-event handling, IME preedit paint, edit acknowledgement, pending-edit observation, and recovery-menu presentation do not execute configuration JavaScript.

Phase 20 configuration does **not** invent clipboard-exfiltration, arbitrary filesystem, network, shell, package-manager, WASM, raw-op, or client-side JavaScript authority APIs. Broader package/configuration/AI authority over clipboard, filesystem, shell, network, and raw ops remains deferred (`decision-logs/2026-07-17-1841-phase20-daily-editing-semantics.md`). Binding a Phase 20 command through `bindKey` installs only an inert user-mediated route; clipboard cut/paste stay client-local after explicit user action, save/reload still consume server grants/leases, open dialogs still return selected-file capabilities only, and recovery menus only reuse existing `RequestResync` / save / reload / dismiss primitives.

## Plan 060/061 configuration closure

Plan 060 reviewed every user-visible behavior changed by the comprehensive remediation after Plan 061 established two package runtime trust domains. It promotes **no new configuration API**. Existing documented APIs already cover real user policy choices; implementation and security controls remain compiled and closed.

### Existing user choices

| Choice | Existing surface |
|---|---|
| Load an installed package from `init.js` | [`packages.loadPackage`](packages/load-package.md) |
| Adopt, inspect, revoke, or roll back a third-party package/replacement | `clay package adopt\|inspect\|revoke\|rollback` host CLI, never package JavaScript |
| Approve a fixed language-server contribution for known roots | [`language-server.authorizeLanguageServer`](language-server/authorize-language-server.md), before `loadPackage` seals authority |
| Bind built-in/package commands | [`keybindings.bindKey`](keybindings/bind-key.md) |
| Select theme, typography, or validated syntax tier | [`setTheme`](theme/set-theme.md), [`setTypography`](theme/set-typography.md), [`setSyntaxEnginePreference`](syntax/set-syntax-engine-preference.md) |
| Set approved package UI defaults | [`setPackageOption`](configuration/set-package-option.md) and [`serverSetLayoutOverride`](ui/server-set-layout-override.md) |
| Compose local configuration | [`loadConfigurationModule`](configuration/load-configuration-module.md), confined beneath `~/.config/clay/` |

Third-party adoption and replacement approval remain host-owned durable decisions. JavaScript cannot approve itself, mint `PackageContext`, choose/promote `RuntimeDomain`, expand relation/replacement scope, disable consent checks, or move third-party code into the trusted runtime. `loadPackage` consumes an already-valid approval and routes execution by host provenance; it is not an authorization setting.

### Fixed controls, not settings

The following are correctness, security, resource, or repository-policy invariants and intentionally have no `init.js`, JSON/TOML, environment-variable, package-option, or hidden facade key:

- connection identity stamping; document access holders, leases, version checks, close cleanup, and result subscriptions/routing;
- maximum active connections/documents/sessions, per-connection result lanes, runtime diagnostics, coordinator queues, actor mailboxes, payload/frame budgets, timeouts, worker counts, and heap ceilings;
- atomic-save temp naming, exclusive creation/retries, owner-only mode, sync/permission restoration, and target identity revalidation;
- directory-listing worker concurrency, exact component-ignore grammar and line/pattern/character/read ceilings, git-root concurrency, cancellation cleanup, and deterministic ordering;
- two runtime domains, package-context provenance, compiled bundled inventory/integrity, cross-domain envelopes/generations/deadlines/payloads, replay/restart policy, and shared-third-party-cohort semantics;
- language-server session actor capacity, process stderr/message ceilings, revocation cleanup, and fixed executable/argv/environment contribution descriptors;
- native-dialog generation/in-flight limits, platform backend selection, clipboard backend/lifetime, and file-dialog filters;
- sandbox frame limits/child reaping, IPC endpoint ownership, audit-exception expiry, Cargo test-suite grouping, `debug=line-tables-only`, opt-in `debugging` profile, target-directory layout, and CI commands.

Representative rejected keys include `runtime.domain`, `runtime.packageContext`, `crossDomain.payloadBytes`, `ipc.clientId`, `connections.maxActive`, `documents.maxPerClient`, `queue.capacity`, `completion.resultLaneCapacity`, `save.atomicMode`, `save.tempRetries`, `listing.maxConcurrency`, `listing.ignoreMaxPatterns`, `git.rootConcurrency`, `languageServer.sessionQueueCapacity`, `sandbox.frameBytes`, `dialog.maxInFlight`, `clipboard.backend`, `build.debugProfile`, and `build.targetDirectory`. The closed `setPackageOption` suffix allowlist rejects package-prefixed variants rather than storing inert, misleading state.

`clay:configuration` remains trusted-only. Its exact facade surface is three runtime-backed APIs (`loadConfigurationModule`, `getConfigurationState`, `setPackageOption`) and three explicit planned/unavailable stubs (`setModePreference`, `setDecorationTheme`, `setParsePolicy`). Internal controls are absent from all `custom_properties` and public facades. Configuration evaluation stays in startup/reload/explicit setting work and adds no keypress, paint, layout, scroll, filesystem traversal, process, IPC, or parser hot-path work.

Tests pin this closure in `src/server/configuration.rs::plan060_internal_security_and_performance_controls_are_not_configurable` and `tests/clay_js_api_inventory.rs::configuration_surface_is_closed_and_security_controls_are_not_properties`.

## Phase 20.6 theme/typography/appearance precedence and persistence

Phase 20.6 segregates the canonical default themes into packages (`@clay/theme-modus-operandi`, `@clay/theme-modus-vivendi`) and ships a settings UI panel (`@clay/settings`) for theme, appearance, font, and size-hierarchy selection. The settings panel emits inert `settings.*` command intents; the server validates them, persists the choice, and triggers a runtime reload so the change applies live through the canonical apply path (persist → reload → `init.js` re-evaluation + preference apply → `RuntimeStateSnapshot` fanout). No restart is required.

### Precedence

Configuration values for theme, appearance, and typography resolve in a single documented source order. Highest source wins:

| Rank | Source | Origin | Wins over |
|------|--------|--------|-----------|
| 1 (highest) | `ui-session` | `~/.config/clay/preferences.json`, written by `settings.setTheme` / `settings.setAppearance` (and `settings.setTypography` once free-form textInput value carriage lands) | everything below |
| 2 | `init-js` | `~/.config/clay/init.js` calls to `setTheme` / `setAppearance` / `setTypography` | package / canonical defaults |
| 3 (lowest) | canonical / package default | appearance-derived Modus default (`System` → dark → `@clay/theme-modus-vivendi`; `Light` → `@clay/theme-modus-operandi`), or the Clay core default | — |

On every startup and reload, `init.js` evaluates first; persisted `ui-session` preferences apply immediately after so a UI choice always overrides the equivalent `init.js` call. An explicit `setTheme` always wins over the appearance-derived canonical default. Absent preference fields are no-ops: `init.js` (or the canonical default) stays in effect.

### Persistence store

`~/.config/clay/preferences.json` is a closed JSON object with at most three keys: `theme` (a bundled first-party `@clay/theme-*` specifier), `appearance` (`light` | `dark` | `system`), and `typography` (the `setTypography` payload). The file is bounded (8 KiB), validated at load and persist time, and authority-rejecting (no raw ops, CSS, callbacks, client JavaScript, or state values). A corrupted, oversized, or manually-edited file is dropped field-by-field with a diagnostic so startup never breaks and no authority is granted. The `setPackageOption` source taxonomy is extended with `ui-session` to label these persisted values.

### Settings command flow

`settings.setTheme` / `settings.setAppearance` validate the value, merge it into `preferences.json` (atomic tmp + rename), and reload the runtime. `settings.reset` clears the store and reloads. `settings.open` / `settings.close` / `settings.setTypography` validate and acknowledge; `settings.setTypography` does not yet persist because free-form `textInput` value carriage is a follow-up protocol task — its bounds are still enforced by the `setTypography` op at apply time, so a persisted `typography` field (e.g. written by a future UI or by hand) round-trips safely through reload today.

### Example

```js
// ~/.config/clay/init.js — package defaults overridden by init.js
import { setTheme } from "clay:theme";
setTheme("@clay/theme-gruvbox-material-light"); // source: init-js
// A later UI choice of Modus Vivendi writes preferences.json (source: ui-session)
// and wins on the next reload.
```

## Phase 22.7 split-command alias configuration review

### What changed

Phase 22.7 added two direction-named split aliases — `shell.clientSplitPaneRight` and `shell.clientSplitPaneDown` — resolving to the existing `SplitPaneVertical` (side-by-side) and `SplitPaneHorizontal` (stacked) handlers. They are bindable command IDs, exactly like the canonical IDs, with no default chords; the canonical `Ctrl+\` and `Ctrl+-` bindings are unchanged.

### Configuration surfaces

The aliases are configuration through the documented Clay JS API convention only: string command IDs accepted by [`bindKey`](keybindings/bind-key.md). Bindability is enforced by the keybinding allowlist (`is_runtime_bindable_command` + the `ClientUiCommand` routing branch in `src/server/ops/keybindings.rs`), which the `bindKey` validation gate (`validate_command_id`) enforces for every `init.js` binding.

```js
// ~/.config/clay/init.js
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Shift+Right", "shell.clientSplitPaneRight", { scope: "global" });
bindKey("Ctrl+Shift+Down", "shell.clientSplitPaneDown", { scope: "global" });
```

The `clay:shell` facade also exports the alias IDs as helpers (`clientSplitPaneRight()` / `clientSplitPaneDown()`), documented in [client-split-pane-right](shell/client-split-pane-right.md) and [client-split-pane-down](shell/client-split-pane-down.md).

### Rejected hidden configuration keys

No new configuration keys were introduced. Tab-bar scroll speed, card minimum width, and split direction vocabulary stay fixed behavior or documented command IDs — not `init.js` options.

### Security

Binding an alias grants no authority beyond the canonical command it resolves to: a `client-ui-command-id` that mutates only the Clay-owned pane/split tree client-side after explicit user routing.
## Phase 23 configuration structure and auto-reload review

### What changed

Plan 080 restructured how configuration is organized and reloaded without
adding a single hidden key. Three shipped surfaces cover the whole phase:

1. **Modular structure via `loadConfigurationModule`** — the canonical
   `examples/` tree splits configuration into a base `init.js` plus
   `packages/first-party.js` and `packages/third-party.js` modules, loaded
   with `optional: true` so a broken or missing package module records a
   bounded `configuration.module_failed` warning and never blocks launch or
   reload. The three-file layout is a convention, not a requirement: any
   local module layout under the configuration root (any folder depth,
   static-import chains) drives the same one-line `loadPackage` calls with
   identical outcomes.
2. **Automatic configuration-root watch (server behavior, no JS API)** — a
   bounded polling watcher (~1 s interval, quiet-period debounce) detects
   created/modified/deleted `.js` files and `preferences.json` under the
   effective configuration root and schedules the same serialized
   `runtime.reloadConfiguration` reload path. Failed reloads keep the
   previous generation active and record bounded runtime diagnostics;
   successful reloads re-baseline the watch snapshot.
3. **Default reload chord** — `runtime.reloadConfiguration` ships with a
   global `Ctrl+Shift+R` binding (see the [Phase 19 review](#phase-19-persistent-runtime-hot-reload-configuration-review)
   command metadata above). Users may redeclare, override, or unbind it via
   `bindKey`/`unbindKey`; the Control Center displays the chord.

### Configuration surfaces

Everything above is expressed through documented Clay JS APIs or documented
server behavior: `loadConfigurationModule({ path, optional })`, `bindKey`/
`unbindKey`, the `runtime.reloadConfiguration` built-in command, and the
watcher as fixed automatic server behavior. There is deliberately **no**
`watch*` JS API and no hidden config file key for watching; watch interval,
debounce, and enable/disable are compiled constants this phase.

### Rejected hidden configuration keys

Attempting watcher-control keys through `setPackageOption` fails closed on
the closed option allowlist — `core.watch.intervalMs`, `core.watch.debounceMs`,
`core.watch.enabled`, and any other `watch.*`-suffixed option are rejected
with `unsupported package option` (pinned by
`configuration_rejects_watcher_control_keys` in `src/server/configuration.rs`).
A configurable watcher toggle was considered and rejected (YAGNI): the watcher
grants no authority, so there is nothing to disable for safety; if users later
need tuning, it arrives as a fully documented API through the schema, not a
hidden key.

### Security

The watcher, reload path, and module isolation grant no filesystem, network,
shell, package-install, AI, workspace, or client-side JavaScript authority.
The watcher only schedules the same user-command reload the keybinding
invokes; module loads stay inside the configuration root; optional isolation
bounds failures to a recorded diagnostic. Nothing here widens the
configuration trust domain.

### Performance

Configuration evaluation remains startup/reload-only. The watcher's bounded
polling scan (≤ 256 files, depth ≤ 8, skipping dotfiles/temp files) does zero
work on keypress, paint, layout, scroll, text-event, edit-acknowledgement,
parse-result, or decoration-rendering paths, and a completed reload
re-baselines the snapshot so the watcher never loops.
