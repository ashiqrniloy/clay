---
id: clay.modes.serverRegisterModePattern
kind: clay-js-api
js_module: "clay:modes"
js_export: serverRegisterModePattern
js_facade: runtime/js/modes.js::serverRegisterModePattern
backing_rust: src/packages/modes.rs::ModeRegistry::register_mode
deno_op: op_clay_modes_register_pattern
deno_op_path: src/server/ops/modes.rs::op_clay_modes_register_pattern
name: serverRegisterModePattern
user_facing_name: Register Mode Pattern
summary: Register Mode Pattern through the runtime-backed `clay:modes` Clay JavaScript facade.
owner: server
phase: Phase 16.5
visibility: public
permissions: ['mode-registration']
key_bindings: []
custom_properties:
  - name: extensions
    type: string[]
    default: required
    description: Behavior-changing setting `extensions` for this primitive gate API.
  - name: mimeTypes
    type: string[]
    default: []
    description: Behavior-changing setting `mimeTypes` for this primitive gate API.
  - name: fileNames
    type: string[]
    default: []
    description: Behavior-changing setting `fileNames` for this primitive gate API.
  - name: fileNamePatterns
    type: string[]
    default: []
    description: Behavior-changing setting `fileNamePatterns` for this primitive gate API.
  - name: modeId
    type: string
    default: required
    description: Behavior-changing setting `modeId` for this primitive gate API.
  - name: apiPrefix
    type: string
    default: required
    description: Behavior-changing setting `apiPrefix` for this primitive gate API.
security: Requires mode-registration permission and server validation of package prefix, static pattern schema, duplicate modes, and bounded open-document metadata only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, filesystem-scan, package installation, enable/disable, or arbitrary client behavior authority.
agent_guidance: Use `clay.modes.serverRegisterModePattern` only for its documented primitive gate responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [js-api, modedocumentclassification, modes]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterModePattern

## Summary

Register Mode Pattern through the runtime-backed `clay:modes` Clay JavaScript facade.

## Description

`serverRegisterModePattern` is the runtime-backed public primitive gate API for **Register Mode Pattern**. It is documented so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or `Deno.core.ops` bindings.

Authority: `server-first-mode-registration`. Runtime path: `server-first-op-wrapper`. Mode pattern registration occurs at package/configuration load time and document open/reclassification; it is not on the typing, layout, or paint hot path.

## When to use

Use this API when server-side Clay JavaScript package/configuration code needs the documented `Register Mode Pattern` behavior. Do not use lower-level Rust functions, protocol structures, or raw `Deno.core.ops` names for this capability.

## JavaScript usage

```ts
import { serverRegisterModePattern } from "clay:modes";

serverRegisterModePattern(manifest, { modeId: "markdown", displayName: "Markdown", extensions: ["md"], mimeTypes: ["text/markdown"] });
```

## Example

```ts
serverRegisterModePattern(manifest, { modeId: "markdown", displayName: "Markdown", extensions: ["md"], mimeTypes: ["text/markdown"] });
```

## Options

- `extensions` (`string[]`, default `required`): Behavior-changing setting `extensions` for this API.
- `mimeTypes` (`string[]`, default `[]`): Behavior-changing setting `mimeTypes` for this API.
- `fileNames` (`string[]`, default `[]`): Behavior-changing setting `fileNames` for this API.
- `fileNamePatterns` (`string[]`, default `[]`): Behavior-changing setting `fileNamePatterns` for this API.
- `modeId` (`string`, default `required`): Behavior-changing setting `modeId` for this API.
- `apiPrefix` (`string`, default `required`): Behavior-changing setting `apiPrefix` for this API.

## Key bindings

No default key binding is assigned. Users may bind a key to `clay.modes.serverRegisterModePattern` in `~/.config/clay/init.js`.

## Custom properties

- `extensions` (`string[]`, default `required`): Behavior-changing setting `extensions` for this API.
- `mimeTypes` (`string[]`, default `[]`): Behavior-changing setting `mimeTypes` for this API.
- `fileNames` (`string[]`, default `[]`): Behavior-changing setting `fileNames` for this API.
- `fileNamePatterns` (`string[]`, default `[]`): Behavior-changing setting `fileNamePatterns` for this API.
- `modeId` (`string`, default `required`): Behavior-changing setting `modeId` for this API.
- `apiPrefix` (`string`, default `required`): Behavior-changing setting `apiPrefix` for this API.

## Return and async behavior

Returns JSON-serializable primitive gate metadata from the server-owned validator or registry. The facade is synchronous in the controlled server runtime and is intended for load-time, configuration-time, document-open, or activation-time work only.

The Phase 16.5 facade/runtime status is `runtime-backed`; the `deno_core` op wiring is executable during server-side configuration evaluation for runtime-backed entries.

## Errors

The runtime fails with actionable Clay error codes when arguments are malformed, package metadata fails server validation, required permissions are absent, duplicate prefixes/modes/commands are detected, ambiguous key bindings are found, or the requested primitive is intentionally unavailable.

## Permissions and security

Requires: mode-registration

Requires mode-registration permission and server validation of package prefix, static pattern schema, duplicate modes, and bounded open-document metadata only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, filesystem-scan, package installation, enable/disable, or arbitrary client behavior authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.modes.serverRegisterModePattern` when the user asks for Register Mode Pattern through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/modes.js::serverRegisterModePattern`
- Deno op: `src/server/ops/modes.rs::op_clay_modes_register_pattern` (`op_clay_modes_register_pattern`)
- Backing Rust/current owner: `src/packages/modes.rs::ModeRegistry::register_mode`
- Current implementation audit path: `src/packages/modes.rs::ModeRegistry; src/packages/modes.rs::ModeDeclaration`

## Lookup metadata

- Stable ID: `clay.modes.serverRegisterModePattern`
- User-facing name: Register Mode Pattern
- Kind: `clay-js-api`
- Module/export: `clay:modes` / `serverRegisterModePattern`
- Default key bindings: none
- Custom properties: `extensions`, `mimeTypes`, `fileNames`, `fileNamePatterns`, `modeId`, `apiPrefix`
- Tags: `js-api`, `modedocumentclassification`, `modes`
