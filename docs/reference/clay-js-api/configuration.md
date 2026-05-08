# Clay Configuration System

Clay configuration is JavaScript loaded from `~/.config/clay/init.js`. The configuration system is part of the Clay JS API surface: every configurable option, command binding, and behavior-changing setting should be represented by a documented Clay JS API and included in the Markdown documentation registry.

## Configuration Entry Point

- Default file: `~/.config/clay/init.js`
- The file is loaded by Clay's server-side JavaScript runtime when configuration support is implemented.
- `init.js` may load other local configuration files so users can keep settings modular.
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
import { bindKey } from "clay:keybindings";
import { clientSetCursorStyle } from "clay:editor";

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

Configuration can customize documented Clay behavior through Clay JS APIs. It must not implicitly grant filesystem, network, shell, extension loading, AI mutation, or workspace authority. Permission-bearing APIs still require explicit documented permissions and server-side validation.
