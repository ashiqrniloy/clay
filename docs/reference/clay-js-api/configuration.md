# Clay Configuration System

Clay configuration is JavaScript loaded from `~/.config/clay/init.js`. The configuration system is part of the Clay JS API surface: every configurable option, command binding, and behavior-changing setting should be represented by a documented Clay JS API and included in the Markdown documentation registry.

## Configuration Entry Point

- Default file: `~/.config/clay/init.js`
- The file is loaded by Clay's server-side JavaScript runtime when configuration support is implemented in Phase 11.
- Phase 8 defines the public API contract only; it does not evaluate `init.js`, read arbitrary user files, or execute JavaScript in the Rust client.
- `init.js` may load other local configuration files through [`loadConfigurationModule`](configuration/load-configuration-module.md) so users can keep settings modular.
- App/help/agent surfaces can inspect the documented entry point shape through [`getConfigurationState`](configuration/get-configuration-state.md) once runtime state exists.
- Configuration code must use documented Clay JS APIs instead of raw ops or Rust internals.

## Configuration as Clay JS APIs

Each configuration option is exposed as a Clay JS API. That means it must have:

- A stable `id` and JS facade export.
- A searchable `user_facing_name`.
- `key_bindings`, using an empty list when no default binding exists.
- `custom_properties`, listing every behavior-changing setting the API accepts.
- Markdown documentation linked from `docs/index.md`.
- Generated registry and lookup coverage.
- Security and authority notes.

## Example Shape

```js
// ~/.config/clay/init.js
import { loadConfigurationModule } from "clay:configuration";
import { bindKey } from "clay:keybindings";
import { clientSetCursorStyle } from "clay:editor";

await loadConfigurationModule({ path: "./keys.js" });

bindKey("Ctrl+I", "clay.editor.serverInsertText");

clientSetCursorStyle({
  color: "#ffcc00",
  blinking: true,
  type: "bar",
});
```

A future `clientSetCursorStyle` API would document custom properties such as:

- `color`: CSS-style or Clay-defined color code.
- `blinking`: boolean toggle for cursor blinking.
- `type`: cursor shape, such as `block`, `bar`, or `underline`.

## Security Boundary

Configuration can customize documented Clay behavior through Clay JS APIs. It must not implicitly grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority. Modular loading is constrained to local configuration semantics; it is not a package manager, extension loader, workspace scanner, network fetcher, shell runner, or client-side JavaScript execution hook. Permission-bearing APIs still require explicit documented permissions and server-side validation.
