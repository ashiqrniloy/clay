---
id: configuration.setPackageOption
kind: clay-js-api
js_module: "clay:configuration"
js_export: setPackageOption
js_facade: runtime/js/configuration.js::setPackageOption
backing_rust: src/server/configuration.rs::ConfigurationRuntime::set_package_option
deno_op: op_clay_configuration_set_package_option
deno_op_path: src/server/ops/configuration.rs::op_clay_configuration_set_package_option
name: setPackageOption
user_facing_name: Set Package Option
summary: Register a validated package-owned configuration option from `~/.config/clay/init.js`.
owner: server
phase: Phase 18.4
visibility: public
permissions: ["package-configuration"]
key_bindings: []
custom_properties:
  - name: packagePrefix
    type: string
    default: required
    description: Package apiPrefix that owns the option.
  - name: option
    type: string
    default: package-prefixed
    description: Supported package-prefixed option name.
  - name: value
    type: json
    default: required
    description: Typed bounded option value.
  - name: source
    type: enum
    default: init-js
    description: One of `init-js`, `package-default`, `clay-default`, or `ui-session` (Phase 20.6 persisted user-preference source).
hot_path_policy: Evaluated during configuration/package update work only; not during typing, parsing, Masonry layout, or paint hot paths.
security: does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, hidden option keys, direct Masonry widgets, native widget handles, raw CSS, callbacks, or state-value authority.
agent_guidance: Use only documented typed override/configuration records; never expose hidden keys, raw ops, callbacks, native handles, raw CSS, or client-side JavaScript.
lookup_tags: [configuration, package-options, init-js, clay-js-api, phase18.4, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# setPackageOption

## Summary

`setPackageOption` records a validated package-owned option from `~/.config/clay/init.js`.

## Description

The runtime validates package prefixes, documented option names, typed values, source metadata, payload size, and prohibited authority before storing the option for configuration/package update work.

## When to use

Use this for supported package defaults such as panel visibility, default slot, split ratio, input/action defaults, theme-token remap requests, and fallback behavior.

## JavaScript usage

```ts
import { setPackageOption } from "clay:configuration";
```

## Example

```ts
setPackageOption({ packagePrefix: "markdown", option: "markdown.layout.defaultVisibility", value: "hidden", source: "init-js" });
```

## Options

- `packagePrefix`: package apiPrefix.
- `option`: package-prefixed documented option.
- `value`: typed bounded value.
- `source`: `init-js`, `package-default`, `clay-default`, or `ui-session` (Phase 20.6). The `ui-session` source labels values persisted from the settings UI into `~/.config/clay/preferences.json`.

## Key bindings

No key bindings are registered by this API.

## Custom properties

- `packagePrefix`
- `option`
- `value`
- `source`

## Return and async behavior

Returns a synchronous registration record; `async: false`.

## Errors

Throws `configuration.invalid_package_option` for hidden keys, ad hoc keys, invalid values, oversize payloads, raw ops, raw CSS, native handles, callbacks, state values, or client-side JavaScript.

## Permissions and security

Requires: `package-configuration`. server-side validation is required before any package option is accepted. This API does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, direct Masonry widgets, native widget handles, raw CSS, callbacks, or hidden option-key authority.

## Agent guidance

Use only documented package-prefixed option names. Do not invent hidden JSON/TOML keys or raw style/input/theme maps.

## Backing implementation

- Facade: `runtime/js/configuration.js::setPackageOption`
- Op: `src/server/ops/configuration.rs::op_clay_configuration_set_package_option`
- Rust: `src/server/configuration.rs::ConfigurationRuntime::set_package_option`

## Lookup metadata

Tags: configuration, package-options, init-js, phase18.4, runtime-backed.
