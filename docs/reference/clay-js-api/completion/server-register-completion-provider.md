---
id: completion.serverRegisterCompletionProvider
kind: clay-js-api
js_module: "clay:completion"
js_export: serverRegisterCompletionProvider
js_facade: runtime/js/completion.js::serverRegisterCompletionProvider
backing_rust: src/server/completion.rs::CompletionProviderMeta
deno_op: op_clay_completion_register_completion_provider
deno_op_path: src/server/ops/completion.rs::op_clay_completion_register_completion_provider
name: serverRegisterCompletionProvider
user_facing_name: Register Completion Provider
summary: Registers the completion providers declared in a package's `clay.contributions.completionProviders` manifest entries and optionally binds a package-owned handler module (`module` or `moduleSpecifier` plus `exportName`) that Clay invokes itself under the per-provider timeout. A `moduleSpecifier` handler runs on the domain's latency lane (plan 127); provider metadata, items, and budgets stay manifest-declared data.
owner: server
phase: Phase 27
visibility: public
permissions: ['completion-provider']
key_bindings: []
custom_properties:
  - name: module
    type: object
    default: optional
    description: Package-owned module object whose `exportName` export receives completion requests. Clay invokes that export itself on the domain's general runtime lane; the function value never crosses the op boundary and no callback, cross-package module, command, or native handle is accepted.
  - name: exportName
    type: string
    default: provideCompletion
    description: Export of `module` (or `moduleSpecifier`) that Clay invokes, max 128 chars. Ignored when neither handler binding is present.
  - name: moduleSpecifier
    type: string
    default: optional
    description: Package-owned module specifier (max 512 chars), typically `import.meta.resolve("./provider.js")`. When present the provider runs on the domain's latency lane by importing that module in the lane's isolate, so a busy parse, analysis, or configuration lane cannot delay completions. Must resolve to a loaded module owned by the registering package.
security: Requires completion-provider permission and a package record whose `clay.contributions.completionProviders` entries passed package-record validation (package-owned IDs, unique IDs and item labels, payload budget, inert trigger/boundary metadata, item/budget bounds). Optionally accepts a package-owned `module` object or package-owned `moduleSpecifier` whose `exportName` export Clay invokes itself on the runtime worker under the per-provider timeout budget; the op validates specifier ownership against the package's loaded-module allowlist and never receives a function value, callback, or native handle. Snippet items carry inert LSP placeholder syntax expanded client-local on accept with no callback, command, or provider code. It rejects handler/callback/complete/function, client JavaScript, native handles, raw ops, non-package module references, command side effects, URLs, CSS/raw colors, shell, network, AI, WASM/native/library, package-manager/download authority, and does not grant filesystem, workspace-index, extension loading authority, AI mutation authority, client-side JavaScript authority, raw-op, native-widget, or package-manager authority.
agent_guidance: Declare provider metadata, trigger characters, items, and budgets in `package.json` under `clay.contributions.completionProviders` — this call never reads those fields from its options object. Use the options only to bind a package-owned handler module. Prefer `loadPackage("@vendor/provider")` from user configuration; do not pass callbacks, raw Deno ops, another package's modules, commands, or UI widget code. Structured snippet items are data; do not pass executable snippet transforms or callback-accept hooks.
lookup_tags: [js-api, completion, provider, package, phase18.19]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterCompletionProvider

## Summary

Registers the completion providers declared by the executing package's `clay.contributions.completionProviders` manifest entries, and optionally binds a package-owned handler module for those providers. Provider metadata — provider ID, trigger characters, word-boundary characters, static items, priority, exclusive claim, and budgets — is manifest data: the package record assembler validates it at enable/load time and this call registers exactly what the package declared. The call's options bind a handler: an inline package-owned `module` object with an `exportName` export, or a package-owned `moduleSpecifier` that the runtime imports itself. A `moduleSpecifier` handler additionally runs on the domain's latency lane (plan 127), so a busy parse, analysis, or configuration lane cannot delay completions.

## Description

`serverRegisterCompletionProvider` is the public `clay:completion` registration API. It requires the `completion-provider` permission and an enabled package record that declares at least one `completionProviders` contribution. Calling it registers every provider the package declares — `registeredProviderCount` reports how many — with provenance and package-owned provider IDs taken from the manifest. The Rust snapshot recorded from this registration is what completion requests are served against; the connection path prefix-filters the active package's static items without running package JavaScript.

Executable functions are never passed across the boundary. A JS-backed provider supplies a package-owned `module` object (or a package-owned `moduleSpecifier`) plus an `exportName`; Clay invokes that export itself on the domain's runtime lane under the per-provider timeout. An inline `module` stays on the domain's general lane; a `moduleSpecifier` is materialized by importing the module inside the domain's latency lane, so it cannot be blocked behind package load entries, parse handlers, or document analyzers running on the general lane.

Provider metadata follows the manifest contract: bounded `items` accept plain strings (label == insertText, textFormat plainText) or structured objects with `{ label, insertText, detail?, textFormat?: "plainText"|"snippet" }`. Snippet items carry inert LSP placeholder syntax (`$1`, `${2:default}`, `$0`) expanded client-local on accept; no provider code runs on accept. Budgets (`budgets.timeoutMs`, `budgets.maxItems`), `priority`, `exclusive`, `triggerCharacters`, and `wordBoundaryChars` are declared in the manifest entry; see the package authoring contract.

## When to use

Use this API from a package load entry that needs a handler bound to its declared completion providers, or when a package that registers at runtime must (re)register its manifest-declared provider metadata. Metadata-only providers need no call at all when the host applies the manifest record itself; end-user configuration should use `loadPackage("@vendor/provider")`.

## JavaScript usage

Metadata-only registration (the package declares its providers in `package.json` and ships no executable handler):

```ts
import { serverRegisterCompletionProvider } from "clay:completion";

serverRegisterCompletionProvider({});
```

JS-backed provider bound to a package-owned module that runs on the domain's latency lane:

```ts
import { serverRegisterCompletionProvider } from "clay:completion";
import * as completionModule from "./completion.js";

export default function load() {
  serverRegisterCompletionProvider({
    module: completionModule,
    moduleSpecifier: import.meta.resolve("./completion.js"),
    exportName: "provideCompletion"
  });
}
```

The matching manifest contribution carries the inert metadata:

```json
{
  "clay": {
    "apiPrefix": "words",
    "permissions": ["completion-provider"],
    "contributions": {
      "completionProviders": [{
        "id": "words.buffer",
        "priority": 0,
        "exclusive": false,
        "triggerCharacters": ["."],
        "wordBoundaryChars": [".", ","],
        "items": [
          "const",
          { "label": "fn", "insertText": "fn ${1:name}(${2:args}) {\n\t$0\n}", "textFormat": "snippet", "detail": "function" }
        ],
        "budgets": { "timeoutMs": 500, "maxItems": 64 }
      }]
    }
  }
}
```

## Example

```ts
import { loadPackage } from "clay:packages";

await loadPackage("@vendor/words");
```

The resolver validates and loads the package; the host-enabled record supplies the provider metadata that this API registers.

## Options

`serverRegisterCompletionProvider({ module?, moduleSpecifier?, exportName? })`:

- `module`: package-owned module object whose `exportName` export receives completion requests. Bound as an inline handler on the domain's general lane.
- `moduleSpecifier`: package-owned module specifier (max 512 chars, e.g. `import.meta.resolve("./provider.js")`). The latency lane imports it and invokes `exportName` there; it must resolve to a loaded module owned by the registering package. Omitting both handler bindings registers metadata only.
- `exportName`: handler export name, default `"provideCompletion"`, max 128 chars, read only when `module` or `moduleSpecifier` is present.

Provider metadata is not an option of this call. Fields such as `providerId`, `triggerCharacters`, `triggers`, `wordBoundaryChars`, `items`, `exclusive`, `textFormat`, `priority`, `timeoutMs`, `maxItems`, `packageManifest`, `packageName`, `packagePrefix`, `permissions`, `completionProvider`, and `contribution` are ignored here — declare them in `clay.contributions.completionProviders` (see [Creating Packages: completion providers](../../packages/creating-packages.md#phase-1811-authoring-contract-completion-providers)). Passing an unknown field does not widen authority: nothing in this call grants a provider any capability beyond the manifest record it was enabled with. The op additionally rejects the executable/raw authority fields `handler`, `callback`, `complete`, `function`, `clientJavaScript`, `nativeHandle`, and `rawOps`, and range-checks a `timeoutMs` option when present (1..=5000); the effective timeout is the manifest budget.

## Key bindings

This API has no default key bindings. Manual completion triggering is handled separately by the `completion.trigger` command and active behavior manifest.

## Custom properties

- `module`: optional package-owned module object whose `exportName` export receives the completion request (never a cross-package module, callback, or native handle). Bound on the general lane.
- `exportName`: handler export on `module`/`moduleSpecifier`, default `"provideCompletion"`, max 128 chars.
- `moduleSpecifier`: optional package-owned module specifier (max 512 chars, e.g. `import.meta.resolve("./provider.js")`) that routes the provider to the latency lane by module import. Must resolve to a loaded module owned by the registering package.

Provider ID, triggers, items, `exclusive`, `textFormat`, `priority`, and budgets (`timeoutMs`, `maxItems`) are package.json contribution fields, documented in [Creating Packages](../../packages/creating-packages.md#phase-1811-authoring-contract-completion-providers), not call options.

## Return and async behavior

Returns a synchronous registration summary with `packageName`, `packageVersion`, `packagePrefix`, `registeredProviderCount`, the registered `providers` IDs, `tokens` (empty unless a handler was bound), `exportName`, and `runtimeBridge` (`true` when a `module` was passed, `false` otherwise). Registration is load/reload-time work; completion request scheduling and result publication remain server-side cancellable UI-reactive work.

## Errors

- `completion.invalid_provider`: options are malformed, the executing package lacks `completion-provider`, the package declares no `completionProviders` contribution, a prohibited executable/raw authority field is present, `timeoutMs` is outside 1..=5000, or `moduleSpecifier` does not resolve to a loaded module owned by the registering package.
- `completion.invalid_provider: module export <name> must be a function`: an inline `module` was passed without a callable `exportName` export.
- `completion.registration_failed`: provider metadata was already registered in the current runtime evaluation state (duplicate IDs inside one evaluation or generation).

## Permissions and security

The facade and op reject executable fields including `handler`, `callback`, `complete`, and `function`, plus `clientJavaScript`, `nativeHandle`, and `rawOps`. Requires: `completion-provider`, plus a package record whose `completionProviders` contributions passed package-record validation (package-owned IDs, unique IDs, unique item labels, payload budget, inert trigger/boundary metadata, `budgets.timeoutMs` within 1..=5000 and `budgets.maxItems` within `1..=COMPLETION_RESULT_MAX_ITEMS`). A JS-backed provider passes a package-owned `module` object or a package-owned `moduleSpecifier`; the op validates `moduleSpecifier` ownership against the package's loaded module allowlist before storing it, and the runtime imports and invokes the handler itself, so no function value crosses the boundary. Snippet items (`textFormat: "snippet"`) carry inert LSP placeholder syntax only; they are data, not executable code. Completion provider metadata grants only that registration capability. It does not grant filesystem, network, shell, AI mutation authority, WASM, workspace index, extension loading authority, client-side JavaScript authority, raw-op, native widget, package manager, or command execution authority.

Local typing, paint, layout, scroll, pointer, and text-event handlers never run package provider JavaScript: provider handlers execute on the domain runtime worker (general lane for an inline `module`, latency lane for a `moduleSpecifier`) as cancellable UI-reactive work with a per-request timeout.

## Agent guidance

Declare provider metadata in `package.json`; do not pass it as call options. Use `module` for an inline package-owned handler on the general lane and `moduleSpecifier` for a handler that must stay responsive while the general lane is busy. Prefer `loadPackage("@vendor/provider")` from user configuration. Do not pass callbacks, raw ops, another package's module objects, commands, or UI widget code. Structured snippet items are data; do not pass executable snippet transforms or callback-accept hooks.

## Backing implementation

- Facade: `runtime/js/completion.js::serverRegisterCompletionProvider`
- Runtime include table: `src/server/facades.rs`
- Deno op: `src/server/ops/completion.rs::op_clay_completion_register_completion_provider`
- Metadata shape: `src/server/completion.rs::CompletionProviderMeta`
- Manifest contribution contract: `src/packages/record/language.rs::parse_completion_provider_contributions`

## Lookup metadata

Lookup tags: `js-api`, `completion`, `provider`, `package`, `phase18.19`.