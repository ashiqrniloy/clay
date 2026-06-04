---
id: clay.configuration.loadConfigurationModule
kind: clay-js-api
js_module: "clay:configuration"
js_export: loadConfigurationModule
js_facade: runtime/js/configuration.ts::loadConfigurationModule
backing_rust: src/server/configuration.rs::ConfigurationRuntime::load_module
deno_op: op_clay_configuration_load_module
deno_op_path: src/server/ops/configuration.rs::op_clay_configuration_load_module
name: loadConfigurationModule
user_facing_name: Load Configuration Module
summary: Load a local modular configuration file from the runtime-backed `~/.config/clay/init.js` server-side configuration runtime.
owner: server
phase: Phase 13
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: path
    type: string
    default: none
    description: Local configuration module path relative to `~/.config/clay/init.js`.
security: Local modular configuration contract only; Phase 13 executes only server-side configuration JavaScript through the constrained runtime and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.configuration.loadConfigurationModule` only to describe modular local Clay configuration from `~/.config/clay/init.js`; do not invent filesystem, package, extension, network, shell, workspace, AI, WASM, or client-side JavaScript authority.
lookup_tags: [configuration, entrypoint, initjs, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: true
---

# loadConfigurationModule

## Summary

Load a local modular configuration file from the runtime-backed `~/.config/clay/init.js` server-side configuration runtime.

## Description

`loadConfigurationModule` is the runtime-backed public API for **Load Configuration Module**. It implements Clay's modular local configuration contract inside the constrained Phase 13 server-side JavaScript runtime.

Authority: `configuration-api`. Runtime path: `server-side-configuration-loader-runtime`. Configuration starts at `~/.config/clay/init.js`; this API lets that entry point declare additional local configuration modules, such as `./keys.js` or `./editor.js`, through the Phase 13 server-side JavaScript runtime. Ordinary keypress handling, Masonry paint/layout, IPC frame handling, and editor rendering must not wait on this API.

## When to use

Use this API from `~/.config/clay/init.js` when user configuration should be split into local files. Do not use lower-level protocol structures, Rust functions, raw `Deno.core.ops` bindings, package loaders, extension loaders, shell commands, workspace scans, or network fetches for this capability.

## JavaScript usage

```ts
import { loadConfigurationModule } from "clay:configuration";
import { bindKey } from "clay:keybindings";

// ~/.config/clay/init.js
await loadConfigurationModule({ path: "./keys.js" });
bindKey("Ctrl+I", "clay.editor.serverInsertText", { scope: "editor" });
```

## Example

```ts
await loadConfigurationModule({ path: "./editor.js" });
```

`path` is a local configuration module path interpreted relative to the Clay configuration directory/entry point. The server validates the resolved path against the configuration root before importing and evaluating the local module.

## Options

- `path` (`string`): Local module path for another configuration file, normally relative to `~/.config/clay/init.js`. Future runtime validation should reject package names, URLs, shell commands, workspace paths, and extension-loading forms unless a later permissioned API explicitly documents them.

## Key bindings

No default key binding is assigned. Users call `loadConfigurationModule` from `~/.config/clay/init.js`; it is not a keypress command.

## Custom properties

- `path` (`string`, default `none`): Local configuration module path relative to `~/.config/clay/init.js`.

## Return and async behavior

Returns `Promise<void>` because module loading is ordered asynchronous configuration work. The Phase 13 runtime-backed facade imports validated local configuration modules during startup/reload evaluation.

## Errors

The runtime fails if `path` is missing, malformed, outside the local configuration module contract, attempts package/URL/extension/workspace loading, or if future server-side validation rejects the module. The Phase 13 runtime returns typed JavaScript errors for unavailable state or validation failures.

## Permissions and security

No additional permission is granted by this API in Phase 13.

Local modular configuration contract only; Phase 13 executes only server-side configuration JavaScript through the constrained runtime and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.configuration.loadConfigurationModule` when the user asks how to split Clay configuration from `~/.config/clay/init.js` into local modules. Avoid inventing direct Rust calls, raw op names, filesystem effects beyond the documented local configuration contract, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/configuration.ts::loadConfigurationModule`
- Deno op: `src/server/ops/configuration.rs::op_clay_configuration_load_module` (`op_clay_configuration_load_module`)
- Backing Rust/current owner: `src/server/configuration.rs::ConfigurationRuntime::load_module`
- Current implementation audit path: `runtime/js/configuration.ts::loadConfigurationModule`

## Lookup metadata

- Stable ID: `clay.configuration.loadConfigurationModule`
- User-facing name: Load Configuration Module
- Kind: `clay-js-api`
- Module/export: `clay:configuration` / `loadConfigurationModule`
- Default key bindings: none
- Custom properties: `path`
- Tags: `configuration`, `entrypoint`, `initjs`, `js-api`
