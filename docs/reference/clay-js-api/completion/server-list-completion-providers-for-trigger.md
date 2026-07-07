---
id: clay.completion.serverListCompletionProvidersForTrigger
kind: clay-js-api
js_module: "clay:completion"
js_export: serverListCompletionProvidersForTrigger
js_facade: runtime/js/completion.ts::serverListCompletionProvidersForTrigger
backing_rust: src/server/completion.rs::CompletionProviderRegistry::providers_for_trigger_character
deno_op: op_clay_completion_providers_for_trigger
deno_op_path: src/server/ops/completion.rs::op_clay_completion_providers_for_trigger
name: serverListCompletionProvidersForTrigger
user_facing_name: List Completion Providers for Trigger
summary: Query the registered completion providers that match a trigger character, sorted by priority descending then ID ascending.
owner: server
phase: Phase 18.14
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: trigger
    type: string
    default: required
    description: A single trigger character to match against provider triggerCharacters.
security: Returns only inert provider metadata (id, provenance, priority, trigger characters, budgets). Does not execute providers, read files, access the network, run shell commands, and does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use this API to inspect which providers are registered for a trigger. Prefer loadPackage over manual provider construction.
lookup_tags: [js-api, completion, provider, query, phase18.14]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverListCompletionProvidersForTrigger

## Summary

Query the registered completion providers that match a trigger character.

## Description

`serverListCompletionProvidersForTrigger` returns the subset of registered completion providers whose `triggerCharacters` include the requested trigger. Results are sorted by priority descending, then provider ID ascending, so higher-priority providers are listed first and ties are deterministic.

This is a metadata query; it does not execute any provider handler or perform I/O.

## When to use

Use this API from package load entries, configuration, or tests that need to verify which providers are active for a given trigger character.

## JavaScript usage

```ts
import { serverListCompletionProvidersForTrigger } from "clay:completion";

const result = serverListCompletionProvidersForTrigger({ trigger: "." });
console.log(result.providers.map((provider) => provider.id));
```

## Example

```ts
import { loadPackage } from "clay:packages";
import { serverListCompletionProvidersForTrigger } from "clay:completion";

await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");

const dot = serverListCompletionProvidersForTrigger({ trigger: "." });
const scope = serverListCompletionProvidersForTrigger({ trigger: "::" });

// dot.providers contains rust.keywords, typescript.keywords, javascript.keywords
// scope.providers contains only rust.keywords
```

## Options

- `trigger` (`string`): A non-empty trigger character to match.

## Key bindings

No default key binding.

## Custom properties

- `trigger`: the trigger character to query.

## Return and async behavior

Returns a synchronous object with:

- `trigger`: the requested trigger character.
- `providers`: array of provider metadata objects, each containing:
  - `id`: provider ID.
  - `packageName`, `packageVersion`, `packagePrefix`: provenance.
  - `priority`: deterministic priority.
  - `triggerCharacters`: the provider's registered trigger characters.
  - `wordBoundaryChars`: word-boundary metadata.
  - `timeoutMs`, `maxItems`: budgets.

## Errors

- `clay.completion.invalid_trigger`: `trigger` is missing, not a string, or empty.

## Permissions and security

No permission required. Returns only inert metadata. Does not execute provider handlers, read files, access the network, run shell commands, and does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `clay.completion.serverListCompletionProvidersForTrigger` only for trigger-provider inspection. Do not use it to bypass registration or pass executable callbacks.

## Backing implementation

- Facade: `runtime/js/completion.ts::serverListCompletionProvidersForTrigger`
- Embedded runtime facade: `src/server/js_runtime.rs::CLAY_FACADE_COMPLETION`
- Deno op: `src/server/ops/completion.rs::op_clay_completion_providers_for_trigger`
- Backing registry: `src/server/completion.rs::CompletionProviderRegistry`

## Lookup metadata

Lookup tags: `js-api`, `completion`, `provider`, `query`, `phase18.14`.
