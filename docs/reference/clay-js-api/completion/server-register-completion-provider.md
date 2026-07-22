---
id: clay.completion.serverRegisterCompletionProvider
kind: clay-js-api
js_module: "clay:completion"
js_export: serverRegisterCompletionProvider
js_facade: runtime/js/completion.js::serverRegisterCompletionProvider
backing_rust: src/server/completion.rs::CompletionProviderMeta
deno_op: op_clay_completion_register_completion_provider
deno_op_path: src/server/ops/completion.rs::op_clay_completion_register_completion_provider
name: serverRegisterCompletionProvider
user_facing_name: Register Completion Provider
summary: Register package-provided completion metadata and bounded static keyword/snippet items for Clay's server-side completion framework. Phase 18.19 adds inert snippet text-format, exclusive provider claim, and structured item descriptors.
owner: server
phase: Phase 18.19
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
    type: (string|object)[]
    default: []
    description: Bounded inert static keyword/snippet items. Strings become both label and insertText with textFormat plainText; structured objects carry label, insertText, optional detail, and optional textFormat.
  - name: exclusive
    type: boolean
    default: false
    description: When true and this provider matches at the highest priority tier, suppresses all strictly lower-priority matching providers while preserving equal-priority peers.
  - name: textFormat
    type: "plainText"|"snippet"
    default: "plainText"
    description: Per-item text format. Snippet items carry inert LSP placeholder syntax expanded client-local on accept.
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
security: Requires completion-provider permission and server-side package record validation of provider ID ownership, duplicate IDs/items, trigger metadata, static item/result bounds, timeout/item budgets, and inert load-time metadata. The public API registers inert keyword/snippet items only; JS provider execution tokens are intentionally not exposed. Snippet items carry inert LSP placeholder syntax expanded client-local on accept with no callback, command, or provider code. It rejects handler/callback/complete/function/module, client JavaScript, native handles, raw ops, command side effects, URLs, CSS/raw colors, shell, network, AI, WASM/native/library, package-manager/download authority, and does not grant filesystem, workspace-index, extension loading authority, AI mutation authority, client-side JavaScript authority, raw-op, native-widget, or package-manager authority.
agent_guidance: Use `clay.completion.serverRegisterCompletionProvider` only from package load entries or tests that model package load entries. Prefer `loadPackage("@vendor/provider")` from user configuration; do not pass callbacks, raw Deno ops, modules, commands, or UI widget code. Structured snippet items are data; do not pass executable snippet transforms or callback-accept hooks.
lookup_tags: [js-api, completion, provider, package, phase18.19]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterCompletionProvider

## Summary

Registers inert completion provider metadata for a package. Phase 18.19 adds inert snippet text-format items with client-local LSP placeholder expansion, an exclusive boolean that suppresses lower-priority matching providers, and structured item descriptors (plain strings or `{ label, insertText, detail?, textFormat? }` objects). It does **not** expose JavaScript provider execution; built-in `core.bufferWords` remains the executable provider until the handler bridge is added.

## Description

`serverRegisterCompletionProvider` is the public `clay:completion` registration API for package load entries. It requires the `completion-provider` permission, validates the package-shaped contribution through Clay's package record assembler, and records only inert provider metadata in the server runtime state. It remains callback-free: package JavaScript functions are rejected instead of being stored as executable completion handlers. Bounded `items` accept plain strings (label == insertText, textFormat plainText) or structured objects with `{ label, insertText, detail?, textFormat?: "plainText"|"snippet" }`. Snippet items carry inert LSP placeholder syntax (`$1`, `${2:default}`, `$0`) expanded client-local on accept; no provider code runs on accept. Clay retains the successful runtime evaluation's Rust snapshot and prefix-filters the active package's static items on completion requests without running package JavaScript.

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
  items: ["const", "function", { label: "fn", insertText: "fn ${1:name}(${2:args}) {\n\t$0\n}", textFormat: "snippet", detail: "function" }],
  exclusive: false,
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
- `items`: bounded unique static entries. Plain strings become both label and insertText as plainText; structured objects carry { label, insertText, detail? (optional, bounded), textFormat? ("plainText" or "snippet") }. Snippet items use inert LSP placeholder syntax expanded client-local on accept.
- `exclusive`: when `true` and this provider is the highest-priority match for a request, suppresses all strictly lower-priority matching providers. Equal-priority peers remain. Default `false`.
- `textFormat`: per-item format ("plainText" or "snippet"). Only structured items carry an explicit textFormat; plain-string items default to "plainText". Mixing plainText and snippet items in one provider is rejected.
- `priority`: deterministic provider priority.
- `timeoutMs`: provider timeout budget.
- `maxItems`: provider result item cap.

## Return and async behavior

Returns a synchronous registration summary with `packageName`, `packageVersion`, `packagePrefix`, `registeredProviderCount`, provider IDs, and `runtimeBridge: false`. Registration is load/reload-time work; completion request scheduling and result publication remain server-side cancellable UI-reactive work.

## Errors

- `clay.completion.invalid_provider`: options are malformed, missing `completion-provider`, use a non-package-owned provider ID, duplicate or exceed static item bounds, exceed budgets, or include prohibited authority fields.
- `clay.completion.registration_failed`: duplicate provider metadata was already registered in the current runtime evaluation state.

## Permissions and security

The facade and op reject executable fields including `handler`, `callback`, `complete`, `function`, and `module`, plus `clientJavaScript`, `nativeHandle`, `rawOps`, and `snippets`. Requires: `completion-provider`. Snippet items (`textFormat: "snippet"`) carry inert LSP placeholder syntax only; they are data, not executable code. server-side validation checks package permission declarations, provider ID ownership, duplicate IDs/items, trigger metadata, static item field/count bounds, and timeout/item budgets. Completion provider metadata grants only that registration capability. It does not grant filesystem, network, shell, AI mutation authority, WASM, workspace index, extension loading authority, client-side JavaScript authority, raw-op, native widget, package manager, or command execution authority.

Local typing, paint, layout, scroll, pointer, and text-event handlers never run package provider JavaScript.

## Agent guidance

Prefer `loadPackage("@vendor/provider")` from user configuration. Package load entries may call this API with inert provider metadata. Do not pass callbacks, raw ops, module objects, commands, or UI widget code. Structured snippet items are data; do not pass executable snippet transforms or callback-accept hooks.

## Backing implementation

- Facade: `runtime/js/completion.js::serverRegisterCompletionProvider`
- Runtime include table: `src/server/facades.rs`
- Deno op: `src/server/ops/completion.rs::op_clay_completion_register_completion_provider`
- Metadata shape: `src/server/completion.rs::CompletionProviderMeta`

## Lookup metadata

Lookup tags: `js-api`, `completion`, `provider`, `package`, `phase18.19`.
