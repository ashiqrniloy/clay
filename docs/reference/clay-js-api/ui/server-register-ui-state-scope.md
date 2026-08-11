---
id: ui.serverRegisterUiStateScope
kind: clay-js-api
js_module: "clay:ui"
js_export: serverRegisterUiStateScope
js_facade: runtime/js/ui.js::serverRegisterUiStateScope
backing_rust: src/server/ui.rs::PackageUiRegistry::register_ui_state_scope
deno_op: op_clay_ui_register_ui_state_scope
deno_op_path: src/server/ops/ui.rs::op_clay_ui_register_ui_state_scope
name: serverRegisterUiStateScope
user_facing_name: Register UI State Scope
summary: Register bounded package-owned UI state scope schemas and lifecycle metadata through the runtime-backed `clay:ui` facade.
owner: server
phase: Phase 18.4
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: id
    type: string
    default: package-prefixed
    description: Package-prefixed state scope ID such as `markdown.preview.visibility`.
  - name: scope
    type: enum
    default: required
    description: One of `package-global`, `user-config`, `workspace`, `document`, `pane`, `component`, or `transient-overlay`.
  - name: owner
    type: enum
    default: required
    description: State owner, one of `package`, `shell`, or `server`.
  - name: lifetime
    type: enum
    default: required
    description: State lifetime, one of `session`, `workspace`, `document`, or `transient`.
  - name: persistence
    type: enum
    default: required
    description: Persistence contract, one of `none`, `client-local`, `server-canonical`, or `deferred`.
  - name: implementationStatus
    type: enum
    default: deferred
    description: Whether the scope lifecycle is currently `implemented` or explicitly `deferred`.
  - name: targetId
    type: string
    default: required for pane/component/transient-overlay
    description: Package-prefixed target ID for targeted shell/UI state scopes.
  - name: valueSchema.kind
    type: enum
    default: required
    description: Bounded schema kind, one of `boolean`, `number`, `string`, `enum`, or `object`.
security: Validates package-prefixed IDs, supported state scopes, lifecycle metadata, schema kind, target IDs, provenance, hidden-key rejection, prohibited authority fields, and payload ceilings; registers schemas only and does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, WASM, client-side JavaScript, raw Deno ops, direct Masonry widgets, native widget handles, raw CSS, renderer callbacks, state-value mutation, hidden globals, or raw JSON blob authority.
agent_guidance: Use `ui.serverRegisterUiStateScope` for inert state schema and lifecycle declarations only. Do not store package state values, raw documents, hidden configuration, native handles, callbacks, raw ops, CSS, or client-side JavaScript through this API.
lookup_tags: [ui, package-ui, state, lifecycle, clay-js-api, phase18.4, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterUiStateScope

## Summary

Register bounded package-owned UI state scope schemas and lifecycle metadata through the runtime-backed `clay:ui` facade.

## Description

`serverRegisterUiStateScope` accepts a validated package manifest and an inert UI state-scope declaration. Clay validates the package prefix, scope, owner, lifetime, persistence contract, implementation status, targeted component/pane/overlay ID, schema kind, provenance, prohibited authority fields, and payload size before storing the declaration in the package UI registry.

This API registers schemas and lifecycle metadata only. It does not accept state values, arbitrary JSON blobs, persisted document/workspace data, hidden globals, native widget handles, callbacks, raw CSS, raw Deno ops, or client-side JavaScript. Registration runs during package load/configuration/update work; Masonry paint/layout/pointer/key/text hot paths read already-installed inert metadata only and do not execute package JavaScript or serialize full documents.

## When to use

Use this API when a package-owned panel, overlay, or component needs a documented state lifecycle contract, for example panel visibility stored as session client-local pane state. Use `implementationStatus: "deferred"` when a package needs to declare future workspace, document, user-config, or server-canonical state without implying mutation or persistence authority.

## JavaScript usage

```ts
import { serverRegisterUiStateScope } from "clay:ui";

const result = serverRegisterUiStateScope(manifest, {
  id: "markdown.preview.visibility",
  scope: "pane",
  targetId: "markdown.preview",
  owner: "shell",
  lifetime: "session",
  persistence: "client-local",
  implementationStatus: "implemented",
  valueSchema: { kind: "enum", values: ["visible", "hidden"] },
});
```

## Example

```ts
const cursorState = serverRegisterUiStateScope(manifest, {
  id: "markdown.preview.cursorSync",
  scope: "component",
  targetId: "markdown.preview.root",
  owner: "package",
  lifetime: "session",
  persistence: "none",
  implementationStatus: "implemented",
  valueSchema: { kind: "boolean" },
});

console.log(cursorState.id, cursorState.persistence);
```

## Options

- `id` (`string`, package-prefixed): Stable state scope ID. Hidden path segments such as `markdown._secret` are rejected.
- `scope` (`package-global | user-config | workspace | document | pane | component | transient-overlay`): State authority/lifecycle scope.
- `owner` (`package | shell | server`): Component responsible for owning the state contract.
- `lifetime` (`session | workspace | document | transient`): Lifetime classification.
- `persistence` (`none | client-local | server-canonical | deferred`): Persistence classification. Server-canonical and durable workspace/document semantics remain explicit lifecycle contracts and do not grant mutation authority by themselves.
- `implementationStatus` (`implemented | deferred`, default `deferred`): Whether the declared lifecycle is currently implemented or intentionally deferred.
- `targetId` (`string`, required for `pane`, `component`, and `transient-overlay`): Package-prefixed target ID.
- `valueSchema.kind` (`boolean | number | string | enum | object`): Bounded schema kind. Enum schemas require 1 to 32 string `values`.

## Key bindings

No key binding is assigned. This API does not route input; use behavior manifests and `clay:keybindings` for keys, and `serverRegisterInputContribution` for bounded pointer/focus/action metadata.

## Custom properties

- `id`
- `scope`
- `owner`
- `lifetime`
- `persistence`
- `implementationStatus`
- `targetId`
- `valueSchema.kind`
- `valueSchema.values`

## Return and async behavior

The function is synchronous and returns a JSON-compatible registration result containing `registered`, `id`, `scope`, `owner`, `lifetime`, `persistence`, `implementationStatus`, `valueSchemaKind`, `targetId`, `estimatedPayloadBytes`, and `provenance`.

## Errors

Registration throws when the manifest is invalid, IDs are not package-prefixed, IDs contain hidden path segments, scopes/lifetimes/persistence/status values are unsupported, targeted scopes omit `targetId`, target IDs are not package-prefixed, schema metadata is missing or unbounded, raw values are supplied during registration, prohibited authority fields are present, or the payload exceeds the package UI update budget.

## Permissions and security

`serverRegisterUiStateScope` does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package installation/enable/disable, WASM, raw op, native widget, direct Masonry, raw CSS, renderer callback, state-value mutation, or client-side JavaScript authority. It registers inert schema/lifecycle metadata only and rejects hidden globals, raw state blobs, callbacks, native handles, raw ops, CSS, executable code, and initial/default state values.

## Agent guidance

Agents should keep package state lifecycle declarations generic and primitive-first. Do not add Markdown-specific Rust branches, hidden JSON/TOML state keys, raw Masonry handles, package-authored native widget mutation, or package JavaScript execution in UI hot paths. Mark unsupported persistence/mutation semantics as `implementationStatus: "deferred"` instead of implying authority.

## Backing implementation

- Facade: `runtime/js/ui.js::serverRegisterUiStateScope`
- Deno op: `src/server/ops/ui.rs::op_clay_ui_register_ui_state_scope`
- Registry validator: `src/server/ui.rs::PackageUiRegistry::register_ui_state_scope`
- API inventory: `docs/reference/clay-js-api/api-inventory.toml`

## Lookup metadata

- API ID: `ui.serverRegisterUiStateScope`
- Module: `clay:ui`
- Export: `serverRegisterUiStateScope`
- Phase: Phase 18.4
- Status: runtime-backed
