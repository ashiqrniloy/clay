# Behavior Runtime Registration

## Source

- `src/server/js_runtime.rs`
- `src/server/ops/mod.rs`
- `src/server/ops/keybindings.rs`
- `src/server/ops/behavior.rs`
- `src/server/behavior.rs`
- `runtime/js/keybindings.ts`
- `runtime/js/behavior.ts`

## Overview

Phase 13 runtime-backs key binding and behavior query facades without putting JavaScript on the client keypress hot path. Configuration JavaScript may call `clay:keybindings` during server-side startup/evaluation. Those calls validate the requested chord, scope, and command ID, then compile the registration into the existing inert `BehaviorManifest` model.

## How It Works

The embedded runtime installs key binding ops in `clay_runtime_extension`. The `clay:keybindings` facade calls these ops with JSON options:

```js
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+S", "clay.documents.serverSaveDocument", { scope: "editor" });
```

`op_clay_keybindings_bind_key` parses a single key chord, maps `editor` or `global` scope into `KeyBindingContext`, rejects unsupported conditional `when` expressions, and checks the command against the runtime-bindable command allowlist. Server-first Clay API commands are declared as `CommandAuthority::ServerIntent`; built-in predictable text commands keep built-in client-edit authority. The op mutates `ClayOpState` by cloning the active manifest, replacing any existing rule for the same chord/context, adding a command declaration if needed, and publishing through `ActiveBehaviorManifest::publish_replacement` so validation and behavior-version advancement are reused.

`op_clay_keybindings_unbind_key` removes the matching chord/context and publishes another validated manifest replacement. `listKeyBindings`, `getActiveBehaviorManifest`, and `listBehaviorRoutes` are read-only facades over the same server-owned manifest state. `ClayRuntimeEvaluation` returns a behavior manifest only when configuration changed it; server startup applies that manifest to the process-wide `ActiveBehaviorManifest`, allowing normal connection bootstrap and replacement publication to keep using existing protocol paths.

## Invariants and Constraints

- JavaScript registration is configuration/startup work only; ordinary keypress routing remains native client manifest lookup.
- Behavior manifests stay inert data: no client-side JavaScript, executable action payloads, shell/network/package/WASM/AI authority, or direct filesystem access is embedded in a rule.
- Unknown command IDs and malformed chords/scopes are rejected before a manifest can be published.
- Manifest versioning is atomic and server-owned through `ActiveBehaviorManifest::publish_replacement`.
- Client routing continues through `src/client/behavior.rs::ClientBehaviorState::route_key`, so server-first bindings become intent routes instead of synchronous JavaScript calls.

## Tests

- `configuration_bind_key_updates_behavior_manifest`: verifies `bindKey` creates a versioned manifest route for a Clay API command.
- `configuration_unbind_key_updates_behavior_manifest`: verifies `unbindKey` removes the route through another atomic manifest update.
- `unknown_command_binding_is_rejected`: verifies unregistered/permission-bearing command IDs fail safely.
- `keypress_routing_uses_manifest_not_js`: installs the runtime-generated manifest in `ClientBehaviorState` and routes `Ctrl+S` locally as a server intent.
- Command: `cargo test js_runtime --quiet`

## Related

- [Behavior Manifests](behavior-manifests.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- `plans/014-Phase13-Embedded-JavaScript-Runtime.md`
