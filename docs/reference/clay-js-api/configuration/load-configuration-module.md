---
id: configuration.loadConfigurationModule
kind: clay-js-api
js_module: "clay:configuration"
js_export: loadConfigurationModule
js_facade: runtime/js/configuration.js::loadConfigurationModule
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
    description: Local configuration module path relative to the configuration root (e.g. `~/.config/clay/init.js`).
  - name: optional
    type: boolean
    default: "false"
    description: When true, a failing or missing module is isolated: it records a bounded `configuration.module_failed` warning and returns `{ loaded: false, error }` instead of failing configuration evaluation.
security: Local modular configuration contract only; Phase 13 executes only server-side configuration JavaScript through the constrained runtime and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `configuration.loadConfigurationModule` only to describe modular local Clay configuration from `~/.config/clay/init.js`; do not invent filesystem, package, extension, network, shell, workspace, AI, WASM, or client-side JavaScript authority.
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

Authority: `configuration-api`. Runtime path: `server-side-configuration-loader-runtime`. Configuration starts at `~/.config/clay/init.js`; this API lets that entry point declare additional local configuration modules, such as `./keys.js` or `./editor.js`, through the Phase 13 server-side JavaScript runtime. Ordinary keypress handling, client paint/layout, IPC frame handling, and editor rendering must not wait on this API.

## When to use

Use this API from `~/.config/clay/init.js` when user configuration should be split into local files. Do not use lower-level protocol structures, Rust functions, raw `Deno.core.ops` bindings, package loaders, extension loaders, shell commands, workspace scans, or network fetches for this capability.

## JavaScript usage

```ts
import { loadConfigurationModule } from "clay:configuration";
import { bindKey } from "clay:keybindings";

// ~/.config/clay/init.js
await loadConfigurationModule({ path: "./keys.js" });
bindKey("Ctrl+I", "editor.serverInsertText", { scope: "editor" });
```

## Example

```ts
await loadConfigurationModule({ path: "./editor.js" });
```

`path` is a local configuration module path interpreted relative to the Clay configuration directory/entry point. The server validates the resolved path against the configuration root before importing and evaluating the local module.

Optional modules isolate faults:

```ts
await loadConfigurationModule({ path: "./packages/third-party.js", optional: true });
// -> { loaded: true }
```

If an optional module is missing or throws, configuration evaluation continues, a bounded `configuration.module_failed` warning is recorded (visible in diagnostics, reload outcomes, and the diagnostic store), and the call resolves to `{ loaded: false, error }`. The shipped `examples/` tree loads its first-party and third-party package modules this way, so a broken package module never blocks launch or reload.

## Options

- `path` (`string`, required): Local module path for another configuration file, normally relative to the configuration root. Package names, URLs, shell commands, workspace paths, and extension-loading forms are rejected.
- `optional` (`boolean`, default `false`): When `true`, a missing or failing module records a bounded `configuration.module_failed` warning and resolves to `{ loaded: false, error }`; evaluation continues. When `false` (default), a missing or failing module fails configuration evaluation with a `configuration.invalid_module` error. Path containment is validated before the optional catch: a path that escapes the configuration root still hard-fails with `optional: true`.

## Key bindings

No default key binding is assigned. Users call `loadConfigurationModule` from `~/.config/clay/init.js`; it is not a keypress command.

## Custom properties

- `path` (`string`, default `none`): Local configuration module path relative to the configuration root.
- `optional` (`boolean`, default `false`): Fault-isolated load; failures record a bounded `configuration.module_failed` warning and resolve to `{ loaded: false, error }` instead of failing evaluation.

## Return and async behavior

Returns `Promise<ConfigurationModuleLoadResult>` because module loading is ordered asynchronous configuration work:

- `{ loaded: true }` — the module imported and evaluated successfully.
- `{ loaded: false, error: string }` — only possible with `optional: true`; the error message is truncated to 1 KiB and also recorded as a bounded `configuration.module_failed` warning.

## Errors

The runtime fails if `path` is missing, malformed, outside the local configuration module contract, attempts package/URL/extension/workspace loading, or escapes the configuration root — `optional: true` does not mask containment violations because path validation runs before the optional catch. With `optional: true`, a module that is missing or throws during import does not fail evaluation; it records a bounded `configuration.module_failed` warning and resolves to `{ loaded: false, error }`. With `optional: false` (default), the same conditions fail configuration evaluation with typed JavaScript errors.

## Permissions and security

No additional permission is granted by this API. Module loads stay inside the configuration root; optional isolation bounds failures to a recorded diagnostic and grants no package, filesystem, network, shell, extension, AI, workspace, or client authority beyond the existing configuration trust domain.

Local modular configuration contract only; Phase 13 executes only server-side configuration JavaScript through the constrained runtime and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `configuration.loadConfigurationModule` when the user asks how to split Clay configuration from `~/.config/clay/init.js` into local modules. Avoid inventing direct Rust calls, raw op names, filesystem effects beyond the documented local configuration contract, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/configuration.js::loadConfigurationModule`
- Deno op: `src/server/ops/configuration.rs::op_clay_configuration_load_module` (`op_clay_configuration_load_module`)
- Backing Rust/current owner: `src/server/configuration.rs::ConfigurationRuntime::load_module`
- Current implementation audit path: `runtime/js/configuration.js::loadConfigurationModule`

## Lookup metadata

- Stable ID: `configuration.loadConfigurationModule`
- User-facing name: Load Configuration Module
- Kind: `clay-js-api`
- Module/export: `clay:configuration` / `loadConfigurationModule`
- Default key bindings: none
- Custom properties: `path`, `optional`
- Tags: `configuration`, `entrypoint`, `initjs`, `js-api`
