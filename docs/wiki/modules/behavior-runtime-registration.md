# Behavior Runtime Registration

## Source

- `src/server/js_runtime.rs`
- `src/server/ops/mod.rs`
- `src/server/ops/keybindings.rs`
- `src/server/ops/behavior.rs`
- `src/server/behavior.rs`
- `runtime/js/keybindings.js`
- `runtime/js/behavior.js`

## Overview

Phase 13 runtime-backs key binding and behavior query facades without putting JavaScript on the client keypress hot path. Configuration JavaScript may call `clay:keybindings` during server-side startup/evaluation. Those calls validate the requested chord, scope, and command ID, then compile the registration into the existing inert `BehaviorManifest` model.

## How It Works

The embedded runtime installs key binding ops in `clay_runtime_extension`. The `clay:keybindings` facade calls these ops with JSON options:

```js
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
```

`op_clay_keybindings_bind_key` parses a single key chord, maps `editor` or `global` scope into `KeyBindingContext`, rejects unsupported conditional `when` expressions, and checks the command against the runtime-bindable command allowlist. Server-first Clay API commands are declared as `CommandAuthority::ServerIntent`; built-in predictable text commands keep built-in client-edit authority. The op mutates `ClayOpState` by cloning the active manifest, replacing any existing rule for the same chord/context, adding a command declaration if needed, and publishing through `ActiveBehaviorManifest::publish_replacement` so validation and behavior-version advancement are reused.

Batch table form (bindKey ergonomics round): `bindKey({ scope, bindings: { chord: command, ... } })` and `unbindKey({ scope, keys: [...] })` are overloads of the same facade functions, dispatched to `op_clay_keybindings_bind_keys` / `op_clay_keybindings_unbind_keys` when the first argument is an object. The batch ops are **all-or-nothing**: pass 1 validates every entry with the same pure helpers as the single ops (`parse_key_chord`, `validate_command_id`, `command_routing_policy` — none touch state), pass 2 applies via the existing `ClayOpState::bind_key`/`unbind_key` loop. A bad entry rejects the whole table with its 1-based index in the diagnostic (`keybindings.invalid_bind: entry 2: ...`). Duplicate chords inside one table collapse to the last value at JSON parse time, preserving the per-chord "last binding wins" rule. The single-argument form is unchanged; per-entry scope overrides were deliberately not added (YAGNI).

`documents.clientOpenFileDialog` is the first runtime-bindable client UI command. `bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" })` records a `RoutingPolicy::ClientUiCommand` route with `CommandAuthority::ClientUi`; keypress handling later remains a native manifest lookup and submits an app-driver action, not JavaScript execution or a server-first request.

`op_clay_keybindings_unbind_key` removes the matching chord/context and publishes another validated manifest replacement. `listKeyBindings`, `getActiveBehaviorManifest`, and `listBehaviorRoutes` are read-only facades over the same server-owned manifest state. `ClayRuntimeEvaluation` returns a behavior manifest only when configuration changed it; server startup applies that manifest to the process-wide `ActiveBehaviorManifest`, allowing normal connection bootstrap and replacement publication to keep using existing protocol paths.

## Invariants and Constraints

- JavaScript registration is configuration/startup work only; ordinary keypress routing remains native client manifest lookup.
- Behavior manifests stay inert data: no client-side JavaScript, executable action payloads, shell/network/package/WASM/AI authority, or direct filesystem access is embedded in a rule.
- Client UI command routes grant only native app UI intent routing. `documents.clientOpenFileDialog` may later open a user-mediated file picker, but the binding itself does not scan files, read file contents, install packages, enable shell/network/AI/WASM/raw-op access, or broaden workspace authority.
- Unknown command IDs and malformed chords/scopes are rejected before a manifest can be published.
- Manifest versioning is atomic and server-owned through `ActiveBehaviorManifest::publish_replacement`.
- Client routing continues through `src/client/behavior.rs::ClientBehaviorState::route_key`, so server-first bindings become intent routes instead of synchronous JavaScript calls.

## Tests

- `configuration_bind_key_updates_behavior_manifest`: verifies `bindKey` creates a versioned manifest route for a Clay API command.
- `configuration_unbind_key_updates_behavior_manifest`: verifies `unbindKey` removes the route through another atomic manifest update.
- `unknown_command_binding_is_rejected`: verifies unregistered/permission-bearing command IDs fail safely.
- `configuration_bind_ctrl_o_to_client_open_file_dialog`: verifies `bindKey` can publish `Ctrl+O` as a client UI route with `client-ui` authority.
- `keypress_routing_uses_manifest_not_js`: installs the runtime-generated manifest in `ClientBehaviorState` and routes `Ctrl+S` locally as a server intent.
- `keypress_routing_can_reach_client_ui_command_without_js`: installs the runtime-generated manifest and routes `Ctrl+O` locally as `ClientUiCommandRoute`.
- Command: `cargo test js_runtime --quiet`

## Related

- [Behavior Manifests](behavior-manifests.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- `plans/014-Phase13-Embedded-JavaScript-Runtime.md`
