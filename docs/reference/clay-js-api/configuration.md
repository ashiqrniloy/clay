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

## Phase 19 Windows open-dialog configuration review

Phase 19 reviewed the Windows Markdown open-dialog smoke path and did **not** promote a new dialog-settings configuration API. The configurable behavior is the key binding itself, expressed through the existing [`bindKey`](keybindings/bind-key.md) Clay JS API:

```js
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

`clay.documents.clientOpenFileDialog` is a fixed Clay command ID that can be routed by inert behavior manifests after configuration evaluation. No default `Ctrl+O` shortcut in Rust exists; without an `init.js` binding or fixture binding, `Ctrl+O` is not treated as the open-file command.

Dialog behavior in this phase uses fixed defaults, not hidden `init.js` keys: Windows-only native dialog support, Markdown filters for `.md`, `.markdown`, and `.mdown`, an all-files fallback, cancellation as a non-error no-op, selected-file-only server validation/granting, and edit-only selected-file behavior with save out of scope. The `windows-markdown-open` development fixture uses normal package, SDUI, parse/decorations, and `bindKey` APIs; it does not introduce ad hoc keys such as dialog filters, default directories, package enablement settings, or callable client-side hooks.

Configuration remains server startup/load-time work. Pressing the configured key uses client-local manifest routing and then an explicit native UI command; ordinary keypress, paint, scroll, layout, text-event, edit acknowledgement, and Markdown decoration rendering paths do not execute configuration JavaScript. This configuration route does not grant arbitrary filesystem authority, package installation or enable/disable authority, shell, network, AI, WASM, raw Deno ops, workspace expansion, or client-side JavaScript authority.

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
| One-line package loading | `clay.packages.loadPackage` | implemented (Plan 029, Phase 18.6); constrained to first-party `@clay/*` packages only; see `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md` for the authority rationale | n/a |

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
