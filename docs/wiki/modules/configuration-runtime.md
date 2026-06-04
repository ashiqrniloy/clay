# Configuration Runtime

## Source

- `src/server/configuration.rs`
- `src/server/js_runtime.rs`
- `src/server/mod.rs`
- `src/server/ops/configuration.rs`
- `runtime/js/configuration.ts`
- `src/server/js_runtime.rs` tests

## Overview

Clay can now evaluate a constrained local configuration entry point from a configuration root containing `init.js`. Server startup loads the default `~/.config/clay/init.js` when it exists, and tests can supply explicit fixture roots. The runtime supports the documented `clay:configuration` facade, local relative `.js` modules under the same configuration directory, and read-only configuration state for the entry point and loaded modules.

## Responsibilities

- Treat `~/.config/clay/init.js` as the default user configuration entry point while allowing tests to provide an explicit configuration root.
- Resolve relative local JavaScript modules without changing the process current directory or invoking shell/package loading behavior.
- Reject URLs, package specifiers, absolute paths, extensionless imports, and traversal outside the configuration root.
- Expose `loadConfigurationModule({ path })` and `getConfigurationState()` through Clay-owned ops, not raw user-facing op calls.
- Preserve Phase 16.5 package/mode configuration review surfaces (`setPackageOption`, `setModePreference`, `setDecorationTheme`, and `setParsePolicy`) as explicit planned `clay:configuration` facade exports rather than ad hoc settings.

## How It Works

`ClayJsRuntimeService::load_configuration_from_root` runs on the same blocking runtime worker used by controlled JavaScript evaluation. It constructs a `ConfigurationRuntime` from the supplied root, canonicalizes `init.js`, creates a file URL for that entry point, and installs both `ClayOpState` and the configuration state in `deno_core::OpState`.

`ClayModuleLoader` handles three allowed module families:

1. The main `init.js` file under the configuration root.
2. The built-in `clay:configuration` facade source.
3. Explicit relative `.js` modules that canonicalize under the configuration root.

The facade validates `loadConfigurationModule({ path })` through `op_clay_configuration_load_module` before using dynamic `import(path)`. The module loader performs the authoritative canonical path check again when resolving/loading the module, reads the file directly with Rust filesystem APIs, and records successfully loaded local modules in deterministic first-load order. `getConfigurationState()` returns JSON from `op_clay_configuration_get_state`, which the facade parses into `{ entryPoint, loadedModules }`.

The Phase 16.5 primitive gate reviewed package options, mode preferences, decoration theme preferences, and parse policy preferences but did not implement concrete behavior-changing settings. The controlled runtime and static facade source therefore export `setPackageOption`, `setModePreference`, `setDecorationTheme`, and `setParsePolicy` as planned-unavailable APIs routed through `op_clay_runtime_unavailable`. This keeps `~/.config/clay/init.js` as the future configuration entry point while preventing undocumented keys or package enable/disable authority from appearing before a later decision log and server validator define them.

## Code Examples

```js
// ~/.config/clay/init.js
import { getConfigurationState, loadConfigurationModule } from "clay:configuration";

await loadConfigurationModule({ path: "./ui.js" });
console.log(getConfigurationState().loadedModules);
```

```rust
let service = ClayJsRuntimeService::default();
let result = service.load_configuration_from_root(config_root).await?;
```

## Invariants and Constraints

- Configuration JavaScript runs server-side only; the native client never executes arbitrary configuration JavaScript.
- Module loading is startup/reload work and must stay off Masonry paint, text-event, and ordinary edit acknowledgement hot paths.
- Only explicit relative `.js` files under the configuration root are loadable. No network, npm/jsr/package, shell, workspace scan, extension loading, WASM, AI mutation, or direct client filesystem authority is introduced.
- `loadConfigurationModule` does not implement Deno/npm-style resolution: callers must provide the exact `.js` filename.
- Planned package/mode configuration exports are discoverable facade APIs only; they do not grant package installation, enable/disable mutation, mode activation authority, decoration rendering authority, parse-document authority, or external filesystem/network/shell/AI/workspace access.

## Tests

- `src/server/js_runtime.rs`: loads an `init.js` fixture, loads `./ui.js` via `loadConfigurationModule`, reports entry/module state, rejects traversal/URL/npm/package-style specifiers, and verifies planned package/mode configuration facade exports return clear unavailable errors.
- Command: `cargo test js_runtime --quiet && cargo test configuration_runtime --quiet`

## Related

- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Protocol and Performance Pattern](../../../.agents/skills/project-patterns/references/protocol-and-performance.md)
- `docs/reference/clay-js-api/configuration.md`
- `plans/014-Phase13-Embedded-JavaScript-Runtime.md`
