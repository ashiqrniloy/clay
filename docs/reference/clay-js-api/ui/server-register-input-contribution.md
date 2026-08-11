---
id: ui.serverRegisterInputContribution
kind: clay-js-api
js_module: "clay:ui"
js_export: serverRegisterInputContribution
js_facade: runtime/js/ui.js::serverRegisterInputContribution
backing_rust: src/server/ui.rs::PackageUiRegistry::register_input
deno_op: op_clay_ui_register_input_contribution
deno_op_path: src/server/ops/ui.rs::op_clay_ui_register_input_contribution
name: serverRegisterInputContribution
user_facing_name: Register Input Contribution
summary: Register bounded package-owned pointer, focus, and component action metadata through the runtime-backed `clay:ui` facade.
owner: server
phase: Phase 18.4
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: id
    type: string
    default: package-prefixed
    description: Package-prefixed input contribution ID such as `markdown.preview.input`.
  - name: scope
    type: enum
    default: required
    description: Input scope, one of `component`, `panel`, or `overlay`.
  - name: componentId
    type: string
    default: required
    description: Package-prefixed component, panel, or overlay component ID receiving the inert policy.
  - name: pointer.click
    type: enum
    default: none
    description: Pointer click policy, one of `none`, `focus`, `action`, or `select`.
  - name: pointer.action
    type: string
    default: none
    description: Registered package command emitted when `pointer.click` is `action`.
  - name: pointer.drag
    type: enum
    default: none
    description: Pointer drag policy, one of `none`, `select`, or `pan`.
  - name: focus.policy
    type: enum
    default: restore-editor
    description: Focus policy, one of `none`, `restore-editor`, `focus-component`, or `trap`.
  - name: selectionPolicy
    type: enum
    default: preserve-editor
    description: Selection policy, one of `preserve-editor`, `component-local`, or `disabled`.
  - name: context.modes
    type: string[]
    default: []
    description: Optional manifest-declared mode conditions for this input policy.
  - name: actionTargets
    type: string[]
    default: []
    description: Registered package command IDs allowed for component-scoped actions.
security: Validates package-prefixed input IDs, supported pointer/focus/selection policies, manifest-declared modes, registered action targets, provenance, and payload ceilings; does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, direct Masonry widgets, native widget handles, raw CSS, renderer callbacks, raw native event callbacks, or key-routing authority.
agent_guidance: Use `ui.serverRegisterInputContribution` for inert pointer/focus/action metadata only; keep keys in behavior manifests/keybindings and never expose raw event callbacks, native handles, raw ops, CSS, or client-side JavaScript hooks.
lookup_tags: [ui, package-ui, input, focus, action-routing, clay-js-api, phase18.4, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterInputContribution

## Summary

Register bounded package-owned pointer, focus, and component action metadata through the runtime-backed `clay:ui` facade.

## Description

`serverRegisterInputContribution` accepts a validated package manifest and an inert input declaration. Clay validates the package prefix, target component ID, pointer click/drag policy, focus policy, selection policy, optional mode context, registered action targets, payload size, and provenance before storing the route in the package UI registry.

Accepted declarations become installed shell runtime state (`PackageInputRouting`). Masonry input handlers read that already-validated state only; they do not execute package JavaScript, run package validation, block on IPC, evaluate configuration, expose native event callbacks, or mutate package-owned widgets.

Key routing is intentionally excluded. Packages must continue to use behavior manifests and `clay:keybindings` for keyboard shortcuts and predictable text behavior.

## When to use

Use this API when a package-owned panel, overlay, or component needs bounded pointer, focus, selection, or component-action metadata. Do not use it for key routing or text-editing behavior; those remain behavior manifest and `clay:keybindings` responsibilities.

## JavaScript usage

```ts
import { serverRegisterInputContribution } from "clay:ui";

const result = serverRegisterInputContribution(manifest, {
  id: "markdown.preview.input",
  scope: "component",
  componentId: "markdown.preview.root",
  pointer: {
    click: "action",
    action: "markdown.focusPreview",
    drag: "select",
  },
  focus: { policy: "restore-editor" },
  selectionPolicy: "component-local",
  context: { modes: ["markdown"] },
  actionTargets: ["markdown.togglePreview"],
});
```

## Example

```ts
const previewInput = serverRegisterInputContribution(manifest, {
  id: "markdown.preview.clickToFocus",
  scope: "component",
  componentId: "markdown.preview.root",
  pointer: { click: "focus", drag: "none" },
  focus: { policy: "restore-editor" },
  selectionPolicy: "preserve-editor",
});

console.log(previewInput.id, previewInput.focusPolicy);
```

## Options

- `id` (`string`, package-prefixed): Stable input contribution ID.
- `scope` (`component | panel | overlay`): The shell/component scope that owns the metadata.
- `componentId` (`string`, package-prefixed): Component, panel, or overlay component ID.
- `pointer.click` (`none | focus | action | select`, default `none`): Click behavior.
- `pointer.action` (`string`, optional): Registered command required when `pointer.click` is `action`.
- `pointer.drag` (`none | select | pan`, default `none`): Drag behavior.
- `focus.policy` (`none | restore-editor | focus-component | trap`, default `restore-editor`): Focus behavior.
- `selectionPolicy` (`preserve-editor | component-local | disabled`, default `preserve-editor`): Selection behavior.
- `context.modes` (`string[]`, default `[]`): Optional mode conditions; every mode must be declared by the package manifest.
- `actionTargets` (`string[]`, default `[]`): Registered command IDs allowed by this input contribution.

## Key bindings

No key binding is assigned. This API rejects `keys`, `keybindings`, and `onKey` fields; key routing remains behavior-manifest and `clay:keybindings` work.

## Custom properties

- `id`
- `scope`
- `componentId`
- `pointer.click`
- `pointer.action`
- `pointer.drag`
- `focus.policy`
- `selectionPolicy`
- `context.modes`
- `actionTargets`

## Return and async behavior

The function is synchronous and returns a JSON-compatible registration result containing `registered`, `id`, `scope`, `componentId`, pointer/focus/selection policies, `actionTargets`, `estimatedPayloadBytes`, and `provenance`.

## Errors

Registration throws when the manifest is invalid, IDs are not package-prefixed, scopes or policies are unsupported, context modes are not declared by the package manifest, action targets are not registered commands, prohibited authority fields are present, or the payload exceeds the package UI update budget.

## Permissions and security

The validator rejects raw callbacks, raw `Deno.core.ops`, op names, native widget handles, Masonry widget handles, raw CSS/style strings, client-side JavaScript hooks, unregistered commands, unsupported scopes/policies, undeclared context modes, and oversize payloads. This API does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, direct Masonry widgets, native widget handles, raw CSS, renderer callback, or key-routing authority.

Validation runs at package load, configuration, or explicit UI update time. Runtime input handling reads installed inert `PackageInputRouting` state and preserves client-first predictable text behavior.

## Agent guidance

Use this API for inert package input declarations only. Keep keyboard behavior in behavior manifests/keybindings, route side effects through registered commands, and never add Markdown-specific, package-specific, raw Masonry, raw native callback, raw op, raw CSS, or client-side JavaScript branches.

## Backing implementation

The public facade is `runtime/js/ui.js::serverRegisterInputContribution`. The runtime op is `src/server/ops/ui.rs::op_clay_ui_register_input_contribution`, and the backing validator is `src/server/ui.rs::PackageUiRegistry::register_input`. Accepted declarations are copied into `src/shell/package_ui.rs::PackageInputRouting` for client/runtime reads.

## Lookup metadata

- Stable ID: `ui.serverRegisterInputContribution`
- Module: `clay:ui`
- Export: `serverRegisterInputContribution`
- Phase: Phase 18.4
- Stability: runtime-backed
