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

## Phase 18.5 large-file Markdown configuration review

Phase 18.5 reviewed Markdown large-file behavior and did **not** promote any new user-facing configuration API for the first-party Markdown thresholds. The current Markdown package owns fixed defaults for full/windowed/degraded/plain-text-fallback behavior: full highlighting through `1 MiB`, windowed highlighting above `1 MiB`, large-file behavior above `5 MiB`, `64 KiB` parse windows, `4 KiB` guard ranges, `30 MiB` retained syntax/decor cache budget, and `50 ms` parser timeout.

Those values are documented package defaults, not hidden `init.js` keys. The package registers bounded parser metadata through the existing [`serverRegisterParseHandler`](parse/server-register-parse-handler.md) Clay JS API, whose behavior-changing parser policy fields are listed in `custom_properties` and validated by the server before scheduling parser work. File-size thresholds and degraded-mode labels remain package-owned constants until a later phase implements a concrete `clay.configuration.setPackageOption` or `clay.configuration.setParsePolicy` validator with registry docs, custom-property metadata, and explicit security tests.

Configuration evaluation remains load-time or explicit setting-change work only. Markdown large-file policy must not be recomputed from user JavaScript during keypress, paint, scroll, layout, text-event handling, or parse-result publication. The existing planned `setPackageOption`, `setModePreference`, `setDecorationTheme`, and `setParsePolicy` facades remain unavailable stubs and do not grant package enable/disable, filesystem, network, shell, extension loading, AI mutation, workspace mutation, WASM, raw-op, or client-side JavaScript authority.

## Phase 19 Windows open-dialog configuration review

Phase 19 reviewed the Windows Markdown open-dialog smoke path and did **not** promote a new dialog-settings configuration API. The configurable behavior is the key binding itself, expressed through the existing [`bindKey`](keybindings/bind-key.md) Clay JS API:

```js
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

`clay.documents.clientOpenFileDialog` is a fixed Clay command ID that can be routed by inert behavior manifests after configuration evaluation. No default `Ctrl+O` shortcut in Rust exists; without an `init.js` binding or fixture binding, `Ctrl+O` is not treated as the open-file command.

Dialog behavior in this phase uses fixed defaults, not hidden `init.js` keys: Windows-only native dialog support, Markdown filters for `.md`, `.markdown`, and `.mdown`, an all-files fallback, cancellation as a non-error no-op, selected-file-only server validation/granting, and edit-only selected-file behavior with save out of scope. The `windows-markdown-open` development fixture uses normal package, SDUI, parse/decorations, and `bindKey` APIs; it does not introduce ad hoc keys such as dialog filters, default directories, package enablement settings, or callable client-side hooks.

Configuration remains server startup/load-time work. Pressing the configured key uses client-local manifest routing and then an explicit native UI command; ordinary keypress, paint, scroll, layout, text-event, edit acknowledgement, and Markdown decoration rendering paths do not execute configuration JavaScript. This configuration route does not grant arbitrary filesystem authority, package installation or enable/disable authority, shell, network, AI, WASM, raw Deno ops, workspace expansion, or client-side JavaScript authority.

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
