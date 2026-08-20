# Behavior Runtime Registration

## Source

- `src/server/js_runtime/mod.rs`
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

`op_clay_keybindings_bind_key` parses a key chord — since Phase 24.5 a
space-separated multi-stroke **sequence** (`parse_key_sequence`, each stroke
through the same `parse_key_chord` `+`-modifier grammar; empty sequences and
malformed strokes reject the whole bind) — maps `editor` or `global` scope
into `KeyBindingContext`, rejects unsupported conditional `when` expressions,
and checks the command against the runtime-bindable command allowlist. The
parsed strokes become `KeyBindingRule.sequence: Vec<KeyStroke>`, the
pre-existing archived protocol shape, so no protocol or op change was
needed. Server-first Clay API commands are declared as
`CommandAuthority::ServerIntent`; built-in predictable text commands keep
built-in client-edit authority. The op mutates `ClayOpState` by cloning the
active manifest, replacing any existing rule for the same sequence/context,
adding a command declaration if needed, and publishing through
`ActiveBehaviorManifest::publish_replacement` so validation (including the
Phase 24.5 prefix-collision check) and behavior-version advancement are
reused. `unbindKey` removes only rules whose FULL sequence matches, so a
default rule for the same command bound to a different sequence survives.

Batch table form (bindKey ergonomics round): `bindKey({ scope, bindings: { chord: command, ... } })` and `unbindKey({ scope, keys: [...] })` are overloads of the same facade functions, dispatched to `op_clay_keybindings_bind_keys` / `op_clay_keybindings_unbind_keys` when the first argument is an object. The batch ops are **all-or-nothing**: pass 1 validates every entry with the same pure helpers as the single ops (`parse_key_chord`, `validate_command_id`, `command_routing_policy` — none touch state), pass 2 applies via the existing `ClayOpState::bind_key`/`unbind_key` loop. A bad entry rejects the whole table with its 1-based index in the diagnostic (`keybindings.invalid_bind: entry 2: ...`). Duplicate chords inside one table collapse to the last value at JSON parse time, preserving the per-chord "last binding wins" rule. The single-argument form is unchanged; per-entry scope overrides were deliberately not added (YAGNI).

`documents.clientOpenFileDialog` is the first runtime-bindable client UI command. `bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" })` records a `RoutingPolicy::ClientUiCommand` route with `CommandAuthority::ClientUi`; keypress handling later remains a native manifest lookup and submits an app-driver action, not JavaScript execution or a server-first request.

`op_clay_keybindings_unbind_key` removes the matching chord/context and publishes another validated manifest replacement. `listKeyBindings`, `getActiveBehaviorManifest`, and `listBehaviorRoutes` are read-only facades over the same server-owned manifest state. `ClayRuntimeEvaluation` returns a behavior manifest only when configuration changed it; server startup applies that manifest to the process-wide `ActiveBehaviorManifest`, allowing normal connection bootstrap and replacement publication to keep using existing protocol paths.

## Phase 28 package key routing and line transforms

Package `clay.contributions.keyRouting` is converted through the same
`parse_key_sequence`/`parse_key_chord` grammar used by `bindKey`; one-stroke and
space-separated multi-stroke bindings become real `KeyStroke` sequences during
load/activation. The trusted package loader (`src/server/ops/packages.rs`)
attaches those parsed rules to the registered command snapshot, so Control
Center and active behavior manifests retain package-declared chords. Execute-only
load entries must not duplicate commands already applied from `package.json`.
`parse_keymap` now returns errors instead of creating a raw character chord, and
activation installs only validated rules. `RoutingPolicy::parse` in
`src/protocol/mod.rs` is the single string-to-policy parser, so package keymaps,
runtime `bindKey`, and built-in command declarations cannot drift into different
accepted policy vocabularies.

Mode `editorRules` carries generic transform data: `comments[].linePrefix` and
`continuePrefix` feed the indent-aware comment continuation/toggle engine,
`enter.kind = continueLineMarkers` supplies list markers, and ordered
`headingPrefixes` supplies heading rotation. The Rust client executes these
manifest parameters as leased client-first edits; package JavaScript never runs
before local paint. Package command IDs must have a registered server handler,
a documented built-in client route, or a closed Clay alias.
`EditorClientCommand::from_command_id` owns the Phase 28 alias table for
Rust/TypeScript/JavaScript line comments and Markdown list/heading commands,
while `editor.clientToggleFold` and `editor.toggleInlayHints` remain closed
client-UI routes. Metadata-only commands fail closed rather than returning an
accepted no-op.

## Invariants and Constraints

- JavaScript registration is configuration/startup work only; ordinary keypress routing remains native client manifest lookup.
- Behavior manifests stay inert data: no client-side JavaScript, executable action payloads, shell/network/package/WASM/AI authority, or direct filesystem access is embedded in a rule.
- Client UI command routes grant only native app UI intent routing. `documents.clientOpenFileDialog` may later open a user-mediated file picker, but the binding itself does not scan files, read file contents, install packages, enable shell/network/AI/WASM/raw-op access, or broaden workspace authority.
- Unknown command IDs and malformed chords/scopes are rejected before a manifest can be published.
- Manifest versioning is atomic and server-owned through `ActiveBehaviorManifest::publish_replacement`.
- Client routing continues through `src/client/behavior.rs::ClientBehaviorState::route_key`
(and, for multi-stroke chords, `route_key_sequence` with the
`EditorSurface` pending-chord buffer), so server-first bindings become intent
routes instead of synchronous JavaScript calls. Multi-stroke matching,
pending/timeout/cancel semantics, and prefix-collision validation are
documented in [Sequence Keybindings](sequence-keybindings.md).

## Tests

- `configuration_bind_key_updates_behavior_manifest`: verifies `bindKey` creates a versioned manifest route for a Clay API command.
- `configuration_bind_key_sequence_publishes_multi_stroke_rule`: verifies a space-separated sequence publishes one multi-stroke `KeyBindingRule`.
- `configuration_unbind_key_sequence_removes_only_the_matching_rule`: verifies `unbindKey` removes only full-sequence matches.
- `configuration_bind_key_prefix_collision_is_rejected`: verifies a same-scope strict-prefix rebind fails with `keybindings.bind_failed`.
- `unknown_command_binding_is_rejected`: verifies unregistered/permission-bearing command IDs fail safely.
- `configuration_bind_ctrl_o_to_client_open_file_dialog`: verifies `bindKey` can publish `Ctrl+O` as a client UI route with `client-ui` authority.
- `keypress_routing_uses_manifest_not_js`: installs the runtime-generated manifest in `ClientBehaviorState` and routes `Ctrl+S` locally as a server intent.
- `keypress_routing_can_reach_client_ui_command_without_js`: installs the runtime-generated manifest and routes `Ctrl+O` locally as `ClientUiCommandRoute`.
- Command: `cargo test js_runtime --quiet`

## Related

- [Behavior Manifests](behavior-manifests.md)
- [Sequence Keybindings](sequence-keybindings.md) — Phase 24.5 multi-stroke parser, matcher, pending-chord state, prefix validation
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- `plans/014-Phase13-Embedded-JavaScript-Runtime.md`
