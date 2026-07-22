---
id: clay.modes.serverClassifyDocument
kind: clay-js-api
js_module: "clay:modes"
js_export: serverClassifyDocument
js_facade: runtime/js/modes.js::serverClassifyDocument
backing_rust: src/packages/modes.rs::ModeRegistry::classify
deno_op: op_clay_modes_classify_document
deno_op_path: src/server/ops/modes.rs::op_clay_modes_classify_document
name: serverClassifyDocument
user_facing_name: Classify Document
summary: Classify Document through the runtime-backed `clay:modes` Clay JavaScript facade.
owner: server
phase: Phase 16.5
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: documentId
    type: number
    default: required
    description: Behavior-changing setting `documentId` for this primitive gate API.
  - name: path
    type: string
    default: optional
    description: Behavior-changing setting `path` for this primitive gate API.
  - name: mimeType
    type: string
    default: optional
    description: Behavior-changing setting `mimeType` for this primitive gate API.
security: Returns mode classification metadata from already registered static patterns through server validation; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, filesystem-scan, package installation, enable/disable, or arbitrary client behavior authority.
agent_guidance: Use `clay.modes.serverClassifyDocument` only for its documented primitive gate responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [js-api, modedocumentclassificationquery, modes]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverClassifyDocument

## Summary

Classify Document through the runtime-backed `clay:modes` Clay JavaScript facade.

## Description

`serverClassifyDocument` is the runtime-backed public primitive gate API for **Classify Document**. It is documented so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or `Deno.core.ops` bindings.

Authority: `server-owned-mode-classification-query`. Runtime path: `server-first-query`. Document classification runs at document open/reload or explicit reclassification time and never participates in ordinary typing, layout, paint, or edit acknowledgement hot paths.

## When to use

Use this API when server-side Clay JavaScript package/configuration code needs the documented `Classify Document` behavior. Do not use lower-level Rust functions, protocol structures, or raw `Deno.core.ops` names for this capability.

## JavaScript usage

```ts
import { serverClassifyDocument } from "clay:modes";

const classification = serverClassifyDocument({ documentId: 5, path: "README.md", mimeType: "text/markdown" });
```

## Example

```ts
const classification = serverClassifyDocument({ documentId: 5, path: "README.md", mimeType: "text/markdown" });
```

## Options

- `documentId` (`number`, default `required`): Behavior-changing setting `documentId` for this API.
- `path` (`string`, default `optional`): Behavior-changing setting `path` for this API.
- `mimeType` (`string`, default `optional`): Behavior-changing setting `mimeType` for this API.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.modes.serverClassifyDocument` in `~/.config/clay/init.js`.

## Custom properties

- `documentId` (`number`, default `required`): Behavior-changing setting `documentId` for this API.
- `path` (`string`, default `optional`): Behavior-changing setting `path` for this API.
- `mimeType` (`string`, default `optional`): Behavior-changing setting `mimeType` for this API.

## Return and async behavior

Returns JSON-serializable primitive gate metadata from the server-owned validator or registry. The facade is synchronous in the controlled server runtime and is intended for load-time, configuration-time, document-open, or activation-time work only.

The Phase 16.5 facade/runtime status is `runtime-backed`; the `deno_core` op wiring is executable during server-side configuration evaluation for runtime-backed entries.

## Errors

The runtime fails with actionable Clay error codes when arguments are malformed, package metadata fails server validation, required permissions are absent, duplicate prefixes/modes/commands are detected, ambiguous key bindings are found, or the requested primitive is intentionally unavailable.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Returns mode classification metadata from already registered static patterns through server validation; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, filesystem-scan, package installation, enable/disable, or arbitrary client behavior authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.modes.serverClassifyDocument` when the user asks for Classify Document through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/modes.js::serverClassifyDocument`
- Deno op: `src/server/ops/modes.rs::op_clay_modes_classify_document` (`op_clay_modes_classify_document`)
- Backing Rust/current owner: `src/packages/modes.rs::ModeRegistry::classify`
- Current implementation audit path: `src/packages/modes.rs::ModeRegistry; src/packages/modes.rs::DocumentClassificationInput`

## Lookup metadata

- Stable ID: `clay.modes.serverClassifyDocument`
- User-facing name: Classify Document
- Kind: `clay-js-api`
- Module/export: `clay:modes` / `serverClassifyDocument`
- Default key bindings: none
- Custom properties: `documentId`, `path`, `mimeType`
- Tags: `js-api`, `modedocumentclassificationquery`, `modes`
