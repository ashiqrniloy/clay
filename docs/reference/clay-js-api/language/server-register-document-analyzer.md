---
id: language.serverRegisterDocumentAnalyzer
kind: clay-js-api
js_module: "clay:language"
js_export: serverRegisterDocumentAnalyzer
js_facade: runtime/js/language.js::serverRegisterDocumentAnalyzer
backing_rust: src/server/document_analysis.rs::DocumentAnalysisCoordinator; src/server/ops/document_analysis.rs
deno_op: op_clay_language_register_document_analyzer
deno_op_path: src/server/ops/document_analysis.rs::op_clay_language_register_document_analyzer
name: serverRegisterDocumentAnalyzer
user_facing_name: Register Document Analyzer
summary: Register a long-lived document-analysis worker for a package language-server contribution. Requires exact grant, parse-document, and language-server permissions.
owner: server
phase: Phase 18.21
visibility: public
permissions: ["parse-document", "language-server"]
key_bindings: []
custom_properties:
  - name: packageManifest
    type: object
    default: required
    description: Full package manifest used for provenance validation.
  - name: analyzer.id
    type: string
    default: required
    description: Package-prefixed analyzer ID under the package apiPrefix namespace.
  - name: analyzer.contribution
    type: string
    default: required
    description: Contribution id from the package's clay.contributions.languageServers array; must match an existing exact LanguageServer grant.
  - name: analyzer.modes
    type: string[]
    default: []
    description: Language modes the analyzer monitors; empty matches all modes.
  - name: analyzer.moduleSpecifier
    type: string
    default: required
    description: clay://packages/ module specifier owned by the registering package.
  - name: analyzer.exportName
    type: string
    default: "handleDocumentAnalysis"
    description: Named export on the module providing the analyzer function.
hot_path_policy: Registration is package-load time only. Worker spawn is lazy (first eligible document open), output publication is validated before delivery, and worker stops after last close. Edit acknowledgement and local paint never wait on worker, JS, or subprocess.
security: deny-by-default; requires both parse-document and language-server permissions with an exact current grant; rejects handler/callback/function/executable/args/cwd/environment/process/rawOps; moduleSpecifier must be a loaded package-owned module; workers share the same permission boundary as the main runtime with language_server_authority_sealed; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, or raw-op authority.
agent_guidance: Use only from package load.js with serverRegisterDocumentAnalyzer. Never expose hidden executable/argv/env/config keys; the bridge contract owns all LSP framing, sync, capabilities, and URI conversion.
lookup_tags: [configuration, language, document-analysis, lsp-bridge, phase18.21, runtime-backed, deny-by-default]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterDocumentAnalyzer

## Summary

`serverRegisterDocumentAnalyzer` registers a long-lived document-analysis worker for a package language-server contribution. The worker receives bounded document events (open, change, reset, close) and publishes decorations and diagnostics back through validated output channels.

## Description

This is a **package-load-time** registration API. Call it from a package `dist/load.js` after the package manifest is loaded and an exact language-server grant exists. The registration is inert — no worker starts until a document opens with a mode the analyzer monitors.

The worker runs on a dedicated JS runtime instance sharing the same permission boundary as the main runtime with `language_server_authority_sealed`. It receives canonical events (`open`, `change`, `reset`, `close`, `completion`, `languageIntelligence`, `shutdown`) with bounded document windows and publishes decorations/diagnostics validated against the active document version.

Worker lifecycle is lazy and bounded:
- Spawned on first eligible document open
- Stops after last monitored document closes (2s graceful, 5s kill)
- Capped at `DOCUMENT_ANALYSIS_MAX_WORKERS` globally
- Bounded input mailbox (64 deltas / 2 MiB) with coalesce_reset deduplication
- Bounded output channel (64 events / 512 KiB)

## When to use

Use from `dist/load.js` inside a language-server bridge package. Never call from `~/.config/clay/init.js` directly — bridge packages own this registration. The package must have an exact current `authorizeLanguageServer` grant before registration.

## JavaScript usage

```ts
import { serverRegisterDocumentAnalyzer } from "clay:language";
```

## Example

```js
// packages/lsp-rust/dist/load.js
import { serverRegisterDocumentAnalyzer } from "clay:language";
import * as server from "./server.js";
import { lspRustPackageManifest } from "./index.js";

serverRegisterDocumentAnalyzer({
  packageManifest: lspRustPackageManifest(),
  analyzer: {
    id: "lsp-rust.bridge",
    contribution: "lsp-rust.server",
    modes: ["rust"],
    moduleSpecifier: "clay://packages/@clay/lsp-rust/dist/server.js",
    exportName: "handleDocumentAnalysis",
  },
});
```

## Options

### Required top-level fields
- `packageManifest`: full package manifest object (from `package.json`).
- `analyzer`: object with id, contribution, moduleSpecifier, and optional modes/exportName.

### Analyzer object fields
- `id`: package-prefixed analyzer ID within the package apiPrefix namespace (e.g. `lsp-rust.bridge`). Must not start with `clay.`.
- `contribution`: contribution id from the package's `clay.contributions.languageServers` array (e.g. `lsp-rust.server`).
- `modes`: optional array of language mode identifiers (max 32, each max 128 chars). Empty matches all modes.
- `moduleSpecifier`: `clay://packages/` module specifier for the analyzer handler. Must resolve to a loaded module owned by the registering package.
- `exportName`: name of the exported function on the module. Defaults to `"handleDocumentAnalysis"`. Max 128 chars.

### Rejected fields
Analyzer descriptors must not contain: `handler`, `callback`, `function`, `executable`, `args`, `cwd`, `environment`, `process`, `rawOps`. These are rejected before the registration is accepted. The LSP child process is spawned by the package JS through `startLanguageServerSession`, not by the analyzer descriptor.

## Key bindings

No key bindings are registered by this API.

## Custom properties

- `analyzer.id`
- `analyzer.contribution`
- `analyzer.modes`
- `analyzer.moduleSpecifier`
- `analyzer.exportName`
- `packageManifest`

## Return and async behavior

Returns a synchronous JSON result with `{ packageName, packageVersion, packagePrefix, analyzerId, contribution, exportName, runtimeBridge: true }`. Not awaited (`async: false`).

## Errors

- `language.invalid_analyzer` — malformed options or excess authority fields.
- `language.invalid_analyzer: authority field ... is not accepted` — rejected field in top-level or analyzer object.
- `language.invalid_analyzer: parse-document permission is required` — package manifest lacks parse-document.
- `language.invalid_analyzer: language-server permission is required` — package manifest lacks language-server.
- `language.invalid_analyzer: id ... must use package apiPrefix ...` — analyzer ID outside package namespace.
- `language.invalid_analyzer: contribution must name a fixed package language server` — contribution not found in manifest.
- `language.invalid_analyzer: moduleSpecifier must resolve to a loaded module owned by the package` — specifier outside package allowlist.
- `language.invalid_analyzer: package must be enabled with a current exact language-server grant before analyzer registration` — missing/expired grant.

## Required permissions

Requires: `parse-document` and `language-server`.

Both permissions must be declared in the package manifest's `clay.permissions` array and `language-server` must be in the `clay.capabilities` array. An exact current grant from [`authorizeLanguageServer`](language-server/authorize-language-server.md) with matching package, contribution, descriptor fingerprint, and non-empty workspace root ids must exist before registration.

## Permissions and security

Requires both `parse-document` and `language-server` permissions plus an exact current grant from `authorizeLanguageServer`. server-side validation checks: package enabled status, current exact grant with matching contribution + descriptor fingerprint + workspace roots, moduleSpecifier ownership within the package's loaded module allowlist, and rejected authority fields (handler, callback, function, executable, args, cwd, environment, process, rawOps). The registration is rejected if permissions are absent, the grant is missing or expired, or the moduleSpecifier is not owned by the registering package.

Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, or raw-op authority. Workers share the same sealed permission boundary as the main runtime with `language_server_authority_sealed`. Package-owned JS owns all LSP framing, sync, capabilities, positions, URIs, and cancellation; Rust core remains LSP-wire neutral. See `decision-logs/2026-07-15-1750-language-server-package-worker-authority.md`.

## Agent guidance

Use only from package `dist/load.js`. The bridge contract owns all LSP framing — never push Content-Length, jsonrpc, textDocument/*, or $/cancelRequest through this registration.

## Backing implementation

- Facade: `runtime/js/language.js::serverRegisterDocumentAnalyzer`
- Op: `src/server/ops/document_analysis.rs::op_clay_language_register_document_analyzer`
- Rust: `src/server/document_analysis.rs::DocumentAnalysisCoordinator`

## Lookup metadata

Tags: configuration, language, document-analysis, lsp-bridge, phase18.21, runtime-backed, deny-by-default.
