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
