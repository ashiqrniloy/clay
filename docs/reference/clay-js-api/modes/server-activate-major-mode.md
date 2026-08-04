---
id: clay.modes.serverActivateMajorMode
kind: clay-js-api
js_module: "clay:modes"
js_export: serverActivateMajorMode
js_facade: runtime/js/modes.js::serverActivateMajorMode
backing_rust: src/packages/modes.rs::ModeRegistry::activate_major_mode
deno_op: op_clay_modes_activate_major_mode
deno_op_path: src/server/ops/modes.rs::op_clay_modes_activate_major_mode
name: serverActivateMajorMode
user_facing_name: Activate Major Mode
summary: Activate Major Mode through the runtime-backed `clay:modes` Clay JavaScript facade.
owner: server
phase: Phase 16.5
visibility: public
permissions: ['mode-activation']
key_bindings: []
custom_properties:
  - name: documentId
    type: number
    default: required
    description: Behavior-changing setting `documentId` for this primitive gate API.
  - name: path
    type: string
    default: optional
    description: Behavior-changing setting `path` for this primitive gate API.
  - name: mimeType
    type: string
    default: optional
    description: Behavior-changing setting `mimeType` for this primitive gate API.
  - name: editorRules
    type: 'object | undefined'
    default: registered-mode defaults
    description: Optional declarative editor rules applied to the published behavior manifest for this activation (validated server-side, deny-by-default). Carries the generic rule fields plus per-mode movement and caretStyle settings (Plan 071 tasks 4/6/11); the registration-time editorRules from serverRegisterModePattern is the usual source.
  - name: behaviorVersion
    type: number
    default: generated
    description: Behavior-changing setting `behaviorVersion` for this primitive gate API.
security: Requires mode-activation permission and server validation that the mode was registered and matches the target document metadata; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, package installation, enable/disable, or arbitrary client behavior authority.
agent_guidance: Use `clay.modes.serverActivateMajorMode` only for its documented primitive gate responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [js-api, modeactivation, modes]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverActivateMajorMode

## Summary

Activate Major Mode through the runtime-backed `clay:modes` Clay JavaScript facade.

## Description

`serverActivateMajorMode` is the runtime-backed public primitive gate API for **Activate Major Mode**. It is documented so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or `Deno.core.ops` bindings.

Authority: `server-first-mode-activation`. Runtime path: `server-first-op-wrapper`. Major-mode activation is server-first and publishes validated inert behavior metadata; after activation, ordinary typing uses the installed manifest and does not call JavaScript.

## When to use

Use this API when server-side Clay JavaScript package/configuration code needs the documented `Activate Major Mode` behavior. Do not use lower-level Rust functions, protocol structures, or raw `Deno.core.ops` names for this capability.

## JavaScript usage

```ts
import { serverActivateMajorMode } from "clay:modes";

const activation = serverActivateMajorMode(manifest, { documentId: 5, path: "README.md" });
```

## Example

```ts
const activation = serverActivateMajorMode(manifest, { documentId: 5, path: "README.md" });
```

## Options

- `documentId` (`number`, default `required`): Behavior-changing setting `documentId` for this API.
- `path` (`string`, default `optional`): Behavior-changing setting `path` for this API.
- `mimeType` (`string`, default `optional`): Behavior-changing setting `mimeType` for this API.
- `behaviorVersion` (`number`, default `generated`): Behavior-changing setting `behaviorVersion` for this API.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.modes.serverActivateMajorMode` in `~/.config/clay/init.js`.

## Custom properties

- `documentId` (`number`, default `required`): Behavior-changing setting `documentId` for this API.
- `path` (`string`, default `optional`): Behavior-changing setting `path` for this API.
- `mimeType` (`string`, default `optional`): Behavior-changing setting `mimeType` for this API.
- `behaviorVersion` (`number`, default `generated`): Behavior-changing setting `behaviorVersion` for this API.

## Return and async behavior

Returns JSON-serializable primitive gate metadata from the server-owned validator or registry. The facade is synchronous in the controlled server runtime and is intended for load-time, configuration-time, document-open, or activation-time work only.

The Phase 16.5 facade/runtime status is `runtime-backed`; the `deno_core` op wiring is executable during server-side configuration evaluation for runtime-backed entries.

## Errors

The runtime fails with actionable Clay error codes when arguments are malformed, package metadata fails server validation, required permissions are absent, duplicate prefixes/modes/commands are detected, ambiguous key bindings are found, or the requested primitive is intentionally unavailable.

## Permissions and security

Requires: mode-activation

Requires mode-activation permission and server validation that the mode was registered and matches the target document metadata; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, package installation, enable/disable, or arbitrary client behavior authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.modes.serverActivateMajorMode` when the user asks for Activate Major Mode through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/modes.js::serverActivateMajorMode`
- Deno op: `src/server/ops/modes.rs::op_clay_modes_activate_major_mode` (`op_clay_modes_activate_major_mode`)
- Backing Rust/current owner: `src/packages/modes.rs::ModeRegistry::activate_major_mode`
- Current implementation audit path: `src/packages/modes.rs::ModeRegistry; src/packages/modes.rs::MajorModeActivation`

## Lookup metadata

- Stable ID: `clay.modes.serverActivateMajorMode`
- User-facing name: Activate Major Mode
- Kind: `clay-js-api`
- Module/export: `clay:modes` / `serverActivateMajorMode`
- Default key bindings: none
- Custom properties: `documentId`, `path`, `mimeType`, `behaviorVersion`
- Tags: `js-api`, `modeactivation`, `modes`
