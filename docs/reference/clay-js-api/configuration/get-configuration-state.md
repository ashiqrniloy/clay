---
id: clay.configuration.getConfigurationState
kind: clay-js-api
js_module: "clay:configuration"
js_export: getConfigurationState
js_facade: runtime/js/configuration.ts::getConfigurationState
backing_rust: src/server/configuration.rs::ConfigurationState
deno_op: op_clay_configuration_get_state
deno_op_path: src/server/ops/configuration.rs::op_clay_configuration_get_state
name: getConfigurationState
user_facing_name: Get Configuration State
summary: Inspect planned Clay configuration metadata for the `~/.config/clay/init.js` entry point and local modules.
owner: server
phase: Phase 8
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: entryPoint
    type: string
    default: ~/.config/clay/init.js
    description: User configuration entry point path reported by configuration state.
  - name: loadedModules
    type: string[]
    default: []
    description: Ordered local configuration module paths once runtime loading exists.
security: Returns configuration metadata only; Phase 8 does not execute JavaScript and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.configuration.getConfigurationState` only for read-only configuration metadata and discovery; do not invent configuration execution, filesystem, package, extension, network, shell, workspace, AI, WASM, or client-side JavaScript authority.
lookup_tags: [configuration, entrypoint, initjs, js-api]
app_visible: true
help_visible: true
stability: planned
async: false
---

# getConfigurationState

## Summary

Inspect planned Clay configuration metadata for the `~/.config/clay/init.js` entry point and local modules.

## Description

`getConfigurationState` is the planned public API for **Get Configuration State**. It provides a stable Clay JS lookup surface for app/help/agent discovery of configuration entry point metadata while configuration execution remains deferred.

Authority: `configuration-query-api`. Runtime path: `server-side-configuration-query-planned`. The entry point is `~/.config/clay/init.js`, and any modules reported by this API are local configuration modules declared by that entry point. This query is background/help/configuration metadata and must not be part of ordinary input/rendering hot paths.

## When to use

Use this API when a future Clay UI, help surface, or agent needs to explain which configuration entry point and local modules are known. Do not use lower-level protocol structures, Rust functions, raw `Deno.core.ops` bindings, package loaders, extension loaders, shell commands, workspace scans, or network fetches for this capability.

## JavaScript usage

```ts
import { getConfigurationState } from "clay:configuration";

const state = getConfigurationState();
console.log(state.entryPoint); // "~/.config/clay/init.js"
```

## Example

```ts
const { entryPoint, loadedModules } = getConfigurationState();
```

Phase 8 defines the shape only; the current facade stub does not read or execute configuration files.

## Options

No options are accepted.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.configuration.getConfigurationState` in `~/.config/clay/init.js` if a future command surface supports displaying configuration metadata.

## Custom properties

- `entryPoint` (`string`, default `~/.config/clay/init.js`): User configuration entry point path reported by configuration state.
- `loadedModules` (`string[]`, default `[]`): Ordered local configuration module paths once runtime loading exists.

## Return and async behavior

Returns a `ConfigurationState` object when runtime wiring exists:

```ts
interface ConfigurationState {
  entryPoint: "~/.config/clay/init.js";
  loadedModules: string[];
}
```

Current Phase 8 facade/runtime status is `planned`; the typed stub throws a planned-runtime error rather than reading configuration state.

## Errors

The planned runtime should fail only for unavailable configuration service state or future server-side validation errors. Current Phase 8 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Returns configuration metadata only; Phase 8 does not execute JavaScript and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.configuration.getConfigurationState` when the user asks for Clay configuration entry point or module-loading status. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/configuration.ts::getConfigurationState`
- Future Deno op: `src/server/ops/configuration.rs::op_clay_configuration_get_state` (`op_clay_configuration_get_state`)
- Backing Rust/current owner: `src/server/configuration.rs::ConfigurationState`
- Current implementation audit path: `runtime/js/configuration.ts::getConfigurationState`

## Lookup metadata

- Stable ID: `clay.configuration.getConfigurationState`
- User-facing name: Get Configuration State
- Kind: `clay-js-api`
- Module/export: `clay:configuration` / `getConfigurationState`
- Default key bindings: none
- Custom properties: `entryPoint`, `loadedModules`
- Tags: `configuration`, `entrypoint`, `initjs`, `js-api`
