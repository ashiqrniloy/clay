---
id: clay.language.serverRegisterLanguageIntelligenceProvider
kind: clay-js-api
js_module: "clay:language"
js_export: serverRegisterLanguageIntelligenceProvider
js_facade: runtime/js/language.ts::serverRegisterLanguageIntelligenceProvider
backing_rust: src/server/language_intelligence.rs::LanguageIntelligenceProviderMeta; src/server/ops/language_intelligence.rs
deno_op: op_clay_language_register_intelligence_provider
deno_op_path: src/server/ops/language_intelligence.rs::op_clay_language_register_intelligence_provider
name: serverRegisterLanguageIntelligenceProvider
user_facing_name: Register Language Intelligence Provider
summary: Register a feature-tagged language-intelligence provider (hover, definition, code action, signature help) under `parse-document` authority for cancellable UI-reactive requests.
owner: server
phase: Phase 18.20
visibility: public
permissions: ["parse-document"]
key_bindings: []
custom_properties:
  - name: packageName
    type: string
    default: required
    description: Package name used for provider provenance.
  - name: packageVersion
    type: string
    default: required
    description: Package version used for provider provenance.
  - name: packagePrefix
    type: string
    default: required
    description: Package apiPrefix used for provider ID ownership.
  - name: permissions
    type: string[]
    default: required
    description: Must include parse-document.
  - name: id
    type: string
    default: required
    description: Package-prefixed provider ID (e.g. example.intelligence).
  - name: modes
    type: string[]
    default: []
    description: Language modes the provider handles; empty matches all modes.
  - name: features
    type: string[]
    default: required
    description: One or more of hover, definition, codeAction, signatureHelp.
  - name: priority
    type: number
    default: 0
    description: Higher priority providers are selected first for scheduling.
  - name: exportName
    type: string
    default: "provideLanguageIntelligence"
    description: Export name on the optional module for JS-backed providers.
  - name: timeoutMs
    type: number
    default: 500
    description: Per-request timeout budget, bounded 1..=5000.
  - name: module
    type: object
    default: optional
    description: Token-keyed module object for JS-backed providers; never a cross-package or native handle.
hot_path_policy: Registration is package-load time only. Requests are cancellable UiReactivePriority work that never blocks typing, local paint, or layout. Provider JavaScript is invoked on the persistent Deno worker thread with per-request timeout and bounded Clay-provided document window data.
security: Requires parse-document. does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript. Providers receive only bounded Clay-provided open-document text (64 KB window); executable/process fields are rejected. Hover/definition/code-action/signature results are inert validated data with UTF-8 byte offsets; no commands auto-execute. Code-action edits are inert versioned previews in Phase 18.20. Provider provenance is stamped by the coordinator, not trusted from provider output.
agent_guidance: Use only from package load entries. Prefer loadPackage from user configuration. Do not pass callbacks, modules from other packages, raw Deno ops, shell commands, process handles, or network/data authorities.
lookup_tags: [js-api, language-intelligence, provider, package, phase18.20, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterLanguageIntelligenceProvider

## Summary

Registers a feature-tagged language-intelligence provider under `parse-document` authority. Providers supply hover, definition, code-action, and/or signature-help results for Clay's cancellable `UiReactivePriority` request lane.

## Description

`serverRegisterLanguageIntelligenceProvider` is the public `clay:language` registration API for package load entries. It requires `parse-document` permission, validates the provider descriptor against package provenance (apiPrefix ownership, reserved `clay.*` namespace, duplicate IDs), records inert metadata in the `LanguageIntelligenceCoordinator` registry, and issues a runtime token for optional JS-backed handler registration.

JS-backed providers register a `module` object with an `exportName` export. The handler receives `(request, window)` where `request` is the typed `LanguageIntelligenceRequest` and `window` is a bounded 64 KB document text slice around the cursor byte offset. Results are returned as JSON and validated/stamped by the coordinator before publication. No raw `Deno.core.ops`, process handles, or inter-package module references cross the boundary.

Package-owned keyword/snippet completion uses the separate `clay.completion.serverRegisterCompletionProvider` API. Semantic decoration and diagnostic publication require `render-decorations` in addition to this provider registration.

## When to use

Use from a package load entry that declares language-intelligence features. End-user configuration should use `loadPackage("@vendor/analyzer")`. Do not pass callbacks, modules from other packages, or executable/process authority.

## JavaScript usage

```ts
import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
```

## Example

```ts
// Static metadata-only provider (no JS handler):
serverRegisterLanguageIntelligenceProvider({
  packageName: "@vendor/analyzer",
  packageVersion: "0.1.0",
  packagePrefix: "analyzer",
  permissions: ["parse-document"],
  id: "analyzer.intelligence",
  modes: ["rust"],
  features: ["hover", "definition"],
  priority: 10,
  timeoutMs: 500,
});

// JS-backed provider:
serverRegisterLanguageIntelligenceProvider({
  packageName: "@vendor/analyzer",
  packageVersion: "0.1.0",
  packagePrefix: "analyzer",
  permissions: ["parse-document"],
  id: "analyzer.intelligence",
  modes: ["rust"],
  features: ["hover", "definition", "codeAction", "signatureHelp"],
  priority: 10,
  timeoutMs: 500,
  exportName: "provideLanguageIntelligence",
  module: {
    provideLanguageIntelligence(request, window) {
      return {
        status: "ok",
        payload: {
          kind: "hover",
          range: { byteStart: 0, byteEnd: 4 },
          markdown: "`example`",
        },
      };
    },
  },
});
```

## Options

- `packageName`: package name for provider provenance.
- `packageVersion`: package version for provider provenance.
- `packagePrefix`: apiPrefix used for provider ID ownership.
- `permissions`: must include `parse-document`.
- `id`: package-prefixed provider ID, max 128 chars.
- `modes`: optional language modes; empty means match all.
- `features`: one or more of `hover`, `definition`, `codeAction`, `signatureHelp`.
- `priority`: scheduling priority; higher values are preferred.
- `exportName`: string name of the handler export on `module`. Default `"provideLanguageIntelligence"`.
- `timeoutMs`: per-request timeout, bounded to 1..=5000.
- `module`: optional package-root-confined module with the handler export.

## Key bindings

No key bindings are registered by this API. Language intelligence commands (`clay.language.hover`, `clay.language.goToDefinition`, `clay.language.codeActions`, `clay.language.signatureHelp`) have empty default bindings configurable via `clay.keybindings.bindKey`.

## Custom properties

- `packageName`
- `packageVersion`
- `packagePrefix`
- `permissions`
- `id`
- `modes`
- `features`
- `priority`
- `exportName`
- `timeoutMs`
- `module`

## Return and async behavior

Returns a synchronous registration record with a `token` string for module-backed providers. `async: false`.

## Errors

- `clay.language.invalid_provider` — missing/invalid options, duplicate/reserved ID, missing required fields, unsupported features.
- `clay.language.unauthorized` — missing `parse-document` permission.
- `clay.language.prohibited_authority` — handler/callback/function/clientJavaScript/nativeHandle/rawOps/executable/process/languageServer field detected.
- `clay.language.invalid_module` — `exportName` not a function on `module`.

## Permissions and security

Requires: `parse-document`. server-side validation checks package permission declarations, provider ID ownership, duplicate IDs, reserved `clay.*` namespace, supported feature flags, and bounded mode/timeout/metadata before recording registration. does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript. Providers receive only bounded Clay-provided open-document text (64 KB window). Executable/process fields are rejected. Results are inert validated data; code-action edits are preview-only in Phase 18.20. Provider provenance is stamped by the coordinator. JS handlers run on the persistent Deno worker thread with per-request timeout.

## Agent guidance

Use only from package load entries. Never pass executable callbacks, modules from other packages, raw Deno ops, shell commands, process handles, or network/data authorities.

## Backing implementation

- Facade: `runtime/js/language.ts::serverRegisterLanguageIntelligenceProvider`
- Op: `src/server/ops/language_intelligence.rs::op_clay_language_register_intelligence_provider`
- Rust: `src/server/language_intelligence.rs::LanguageIntelligenceProviderMeta; src/server/ops/language_intelligence.rs`

## Lookup metadata

Tags: js-api, language-intelligence, provider, package, phase18.20, runtime-backed.
