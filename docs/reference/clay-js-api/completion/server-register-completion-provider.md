---
id: clay.completion.serverRegisterCompletionProvider
kind: clay-js-api
js_module: "clay:completion"
js_export: serverRegisterCompletionProvider
js_facade: runtime/js/completion.ts::serverRegisterCompletionProvider
backing_rust: src/server/completion.rs::CompletionProviderMeta
deno_op: op_clay_completion_register_completion_provider
deno_op_path: src/server/ops/completion.rs::op_clay_completion_register_completion_provider
name: serverRegisterCompletionProvider
user_facing_name: Register Completion Provider
summary: Register package-provided completion metadata and bounded static text items for Clay's server-side completion framework.
owner: server
phase: Phase 18.18
visibility: public
permissions: ['completion-provider']
key_bindings: []
custom_properties:
  - name: packageManifest
    type: object
    default: optional
    description: Full package.json-shaped manifest; when provided, Clay validates its completionProviders metadata directly.
  - name: packageName
    type: string
    default: required-without-packageManifest
    description: Package name used for provenance when a load entry passes one provider descriptor.
  - name: packagePrefix
    type: string
    default: required-without-packageManifest
    description: Package apiPrefix used for provider ID ownership and provenance.
  - name: permissions
    type: string[]
    default: required-without-packageManifest
    description: Must include completion-provider.
  - name: completionProvider
    type: object
    default: required-without-packageManifest
    description: Inert completion provider descriptor matching clay.contributions.completionProviders.
  - name: providerId
    type: string
    default: required-without-completionProvider
    description: Package-prefixed provider ID such as example.words.
  - name: triggerCharacters
    type: string[]
    default: []
    description: Inert trigger characters. They request completion but never execute callbacks.
  - name: wordBoundaryChars
    type: string[]
    default: core-buffer-word-boundaries
    description: Inert word-boundary characters used by providers to split tokens.
  - name: items
    type: string[]
    default: []
    description: Bounded inert static items; each string is both completion label and inserted text.
  - name: priority
    type: number
    default: 0
    description: Higher priority providers are considered first; ties are deterministic by ID.
  - name: timeoutMs
    type: number
    default: 500
    description: Provider timeout budget, bounded to 1..=5000.
  - name: maxItems
    type: number
    default: 64
    description: Per-provider item cap bounded by COMPLETION_RESULT_MAX_ITEMS.
security: Requires completion-provider permission and server-side package record validation of provider ID ownership, duplicate IDs/items, trigger metadata, static item/result bounds, timeout/item budgets, and inert load-time metadata. The public API registers inert metadata and static text items only; JS provider execution tokens are intentionally not exposed. It rejects handler/callback/complete/function/module, client JavaScript, native handles, raw ops, snippets, command side effects, URLs, CSS/raw colors, shell, network, AI, WASM/native/library, package-manager/download authority, and does not grant filesystem, workspace-index, extension loading authority, AI mutation authority, client-side JavaScript authority, raw-op, native-widget, or package-manager authority.
agent_guidance: Use `clay.completion.serverRegisterCompletionProvider` only from package load entries or tests that model package load entries. Prefer `loadPackage("@vendor/provider")` from user configuration; do not pass callbacks, raw Deno ops, modules, snippets, commands, or UI widget code.
lookup_tags: [js-api, completion, provider, package, phase18.18]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterCompletionProvider

## Summary

Registers inert completion provider metadata for a package. Phase 18.18 adds bounded static text items to the Phase 18.11 provider ID, priority, trigger, word-boundary, timeout, item-cap, and provenance contract. It does **not** expose JavaScript provider execution yet; built-in `core.bufferWords` remains the executable provider until the handler bridge is added.

## Description

`serverRegisterCompletionProvider` is the public `clay:completion` registration API for package load entries. It requires the `completion-provider` permission, validates the package-shaped contribution through Clay's package record assembler, and records only inert provider metadata in the server runtime state. It remains callback-free: package JavaScript functions are rejected instead of being stored as executable completion handlers. Bounded `items` strings are normalized to provenance-bearing `CompletionItem` text replacements; Clay retains the successful runtime evaluation's Rust snapshot and prefix-filters the active package's static items on completion requests without running package JavaScript.

## When to use

Use this API from a package load entry that declares completion provider metadata. End-user configuration should normally use `loadPackage("@vendor/provider")`; it should not pass executable callbacks or provider functions directly.

## JavaScript usage

```ts
import { serverRegisterCompletionProvider } from "clay:completion";

serverRegisterCompletionProvider({
  packageName: "@vendor/words",
  packageVersion: "0.1.0",
  packagePrefix: "words",
  permissions: ["completion-provider"],
  providerId: "words.buffer",
  triggerCharacters: ["."],
  wordBoundaryChars: [".", ",", ";"],
  items: ["const", "function", "return"],
  priority: 0,
  timeoutMs: 50,
  maxItems: 50
});
```

A package may also pass its full `packageManifest` when the manifest declares `clay.contributions.completionProviders`.

## Example

```ts
import { loadPackage } from "clay:packages";

await loadPackage("@vendor/words");
```

The resolver validates and loads the package load entry; the load entry then calls `serverRegisterCompletionProvider` with inert metadata.

## Options

Pass either `packageManifest` or package context fields plus one inert provider descriptor. `completionProvider` and `contribution` accept the same descriptor shape; top-level `providerId`, trigger, boundary, static-item, priority, timeout, and item-cap fields are normalized into that descriptor.

## Key bindings

This API has no default key bindings. Manual completion triggering is handled separately by the `completion.trigger` command and active behavior manifest.

## Custom properties

- `packageManifest`: full package manifest with `clay.contributions.completionProviders`.
- `packageName`: package name used for provenance without `packageManifest`.
- `packageVersion`: package version used for provenance without `packageManifest`.
- `packagePrefix`: package `apiPrefix` used for provider ownership and provenance.
- `permissions`: required permissions must include `completion-provider`.
- `completionProvider`: inert completion provider descriptor.
- `contribution`: alias for `completionProvider`.
- `providerId`: package-owned provider ID.
- `triggerCharacters`: inert trigger metadata.
- `triggers`: optional wrapper for trigger characters.
- `wordBoundaryChars`: inert word-boundary metadata.
- `items`: bounded unique static strings; each becomes both `CompletionItem.label` and `insert_text` with package provenance.
- `priority`: deterministic provider priority.
- `timeoutMs`: provider timeout budget.
- `maxItems`: provider result item cap.

## Return and async behavior

Returns a synchronous registration summary with `packageName`, `packageVersion`, `packagePrefix`, `registeredProviderCount`, provider IDs, and `runtimeBridge: false`. Registration is load/reload-time work; completion request scheduling and result publication remain server-side cancellable UI-reactive work.

## Errors

- `clay.completion.invalid_provider`: options are malformed, missing `completion-provider`, use a non-package-owned provider ID, duplicate or exceed static item bounds, exceed budgets, or include prohibited authority fields.
- `clay.completion.registration_failed`: duplicate provider metadata was already registered in the current runtime evaluation state.

## Permissions and security

The facade and op reject executable fields including `handler`, `callback`, `complete`, `function`, and `module`, plus `clientJavaScript`, `nativeHandle`, and `rawOps`. Requires: `completion-provider`. server-side validation checks package permission declarations, provider ID ownership, duplicate IDs/items, trigger metadata, static item field/count bounds, and timeout/item budgets. Completion provider metadata grants only that registration capability. It does not grant filesystem, network, shell, AI mutation authority, WASM, workspace index, extension loading authority, client-side JavaScript authority, raw-op, native widget, package manager, snippet, or command execution authority.

Local typing, paint, layout, scroll, pointer, and text-event handlers never run package provider JavaScript.

## Agent guidance

Prefer `loadPackage("@vendor/provider")` from user configuration. Package load entries may call this API with inert provider metadata. Do not pass callbacks, raw ops, module objects, snippets, commands, or UI widget code.

## Backing implementation

- Facade: `runtime/js/completion.ts::serverRegisterCompletionProvider`
- Embedded runtime facade: `src/server/js_runtime.rs::CLAY_FACADE_COMPLETION`
- Deno op: `src/server/ops/completion.rs::op_clay_completion_register_completion_provider`
- Metadata shape: `src/server/completion.rs::CompletionProviderMeta`

## Lookup metadata

Lookup tags: `js-api`, `completion`, `provider`, `package`, `phase18.18`.
