---
id: clay.completion.serverDisableCompletion
kind: clay-js-api
js_module: "clay:completion"
js_export: serverDisableCompletion
js_facade: runtime/js/completion.ts::serverDisableCompletion
backing_rust: src/server/completion.rs::CompletionProviderRegistry::disable_completion
deno_op: op_clay_completion_disable
deno_op_path: src/server/ops/completion.rs::op_clay_completion_disable
name: serverDisableCompletion
user_facing_name: Disable Completion Provider
summary: Suppress a registered completion provider by exact ID or package prefix. Does not grant registration, priority, or execution authority.
owner: server
phase: Phase 18.19
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: provider
    type: string
    default: optional
    description: Exact completion provider ID to disable, e.g. core.bufferWords or rust.snippets. Mutually exclusive with packagePrefix.
  - name: packagePrefix
    type: string
    default: optional
    description: Package apiPrefix whose every registered completion provider should be disabled. Mutually exclusive with provider.
security: Suppresses registered completion providers from selection, stale-drops in-flight results, and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw-op, native-widget, or package-manager authority. Does not remove registered metadata.
agent_guidance: Use from ~/.config/clay/init.js or test setup. Call once per target; re-enabling a disabled provider requires a package reload or runtime restart. Prefer packagePrefix when disabling all providers from one package; use exact provider when suppressing a single built-in or package-owned provider.
lookup_tags: [js-api, completion, provider, disable, phase18.19]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverDisableCompletion

## Summary

Suppresses a registered completion provider so it can no longer produce results for completion requests. Targets an exact provider ID (e.g. `core.bufferWords`, `rust.snippets`) or a package prefix that matches every provider registered under that package (`@clay/rust`). Does **not** grant registration, priority escalation, or execution authority.

## Description

`serverDisableCompletion` is the Phase 18.19 configuration-style API for suppressing completion providers without package unload or runtime restart. It records the target in a server-side disabled-provider set consulted by every completion trigger selection path; disabled providers are excluded from trigger classification, priority ordering, exclusive suppression, and result scheduling. In-flight results from the disabled provider are stale-dropped via a provider generation bump. Disabled providers persist across runtime reloads and generations; re-enabling requires a package reload or runtime restart.

The API follows the `server*` naming convention of sibling completion APIs `serverRegisterCompletionProvider` and `serverListCompletionProvidersForTrigger`. No matching enable API ships in Phase 18.19.

## When to use

Use from `~/.config/clay/init.js` to suppress a completion provider you do not want active. For package-level suppression, prefer `packagePrefix` so all providers from that package are disabled together. Use `provider` for fine-grained targeting of a single built-in or package-owned provider.

## JavaScript usage

```ts
import { serverDisableCompletion } from "clay:completion";

// Disable the built-in buffer-words provider.
serverDisableCompletion({ provider: "core.bufferWords" });

// Disable every completion provider from @clay/rust.
serverDisableCompletion({ packagePrefix: "rust" });
```

## Example

```js
// ~/.config/clay/init.js
import { loadPackage } from "clay:packages";
import { serverDisableCompletion } from "clay:completion";

await loadPackage("@clay/rust");
// Keep Rust language support but suppress the built-in buffer-words provider.
serverDisableCompletion({ provider: "core.bufferWords" });
```

## Options

Pass exactly one non-empty target: either `provider` for an exact completion provider ID (e.g. `rust.snippets`, `typescript.keywords`, `core.bufferWords`) or `packagePrefix` for a package's `apiPrefix` (e.g. `rust`, `typescript`, `javascript`). The target must be at most 128 characters. Both keys must not be set together; extra keys are rejected.

## Key bindings

This API has no default key bindings.

## Custom properties

- `provider`: exact completion provider ID to disable. Mutually exclusive with `packagePrefix`.
- `packagePrefix`: package prefix whose every completion provider should be disabled. Mutually exclusive with `provider`.

## Return and async behavior

Returns a synchronous JSON result with `{ target, disabled, providerGeneration }`. `disabled` is `true` when the target was newly recorded; it is `false` when already disabled (idempotent). `providerGeneration` is the server-side monotonic generation counter after the disable.

## Errors

- `clay.completion.invalid_disable`: options are missing, pass both provider and packagePrefix, pass a key other than provider/packagePrefix, pass a target exceeding 128 characters, or pass empty strings.

## Permissions and security

`serverDisableCompletion` requires no `clay.contributions.permissions` entry. It grants no filesystem, network, shell, extension loading, AI mutation, workspace, package authorization, WASM, client-side JavaScript, raw-op, native-widget, or package-manager authority. It only suppresses already-registered inert provider metadata from the selection path; it does not remove registration records, mutate package state, or grant any execution capability.

## Agent guidance

Prefer one call per target. Use `packagePrefix` to suppress every provider from a package. There is no enable API yet; reload the package or restart the runtime to re-enable disabled providers.

## Backing implementation

- Facade: `runtime/js/completion.ts::serverDisableCompletion`
- Embedded runtime facade: `src/server/js_runtime.rs::CLAY_FACADE_COMPLETION`
- Deno op: `src/server/ops/completion.rs::op_clay_completion_disable`
- Disabled set: `src/server/ops/mod.rs::ClayOpState::disabled_completion_providers`
- Selection filtering: `src/server/completion.rs::CompletionProviderRegistry` + `completion_provider_is_disabled`

## Lookup metadata

Lookup tags: `js-api`, `completion`, `provider`, `disable`, `phase18.19`.
