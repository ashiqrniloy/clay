---
id: clay.ui.serverRegisterPanelContribution
kind: clay-js-api
js_module: "clay:ui"
js_export: serverRegisterPanelContribution
js_facade: runtime/js/ui.js::serverRegisterPanelContribution
backing_rust: src/server/ui.rs::PackageUiRegistry::register_panel
deno_op: op_clay_ui_register_panel_contribution
deno_op_path: src/server/ops/ui.rs::op_clay_ui_register_panel_contribution
name: serverRegisterPanelContribution
user_facing_name: Register Panel Contribution
summary: Register a package-prefixed fixed panel contribution for a Clay pane slot through the runtime-backed `clay:ui` facade.
owner: server
phase: Phase 18.3
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: id
    type: string
    default: package-prefixed
    description: Package-prefixed panel contribution ID such as `markdown.preview`.
  - name: slot
    type: enum
    default: required
    description: Fixed pane slot target, one of `left`, `right`, `top`, or `bottom`.
  - name: kind
    type: enum
    default: fixed
    description: Panel contribution kind; Phase 18.3 accepts `fixed` and uses transient overlays for overlay UI.
  - name: defaultVisibility
    type: enum
    default: visible
    description: Initial panel visibility, one of `visible`, `hidden`, or `collapsed`.
  - name: component
    type: ComponentContributionDefinition
    default: required
    description: Bounded inert component tree rendered by Clay-owned native UI code.
  - name: actionTargets
    type: string[]
    default: []
    description: Package-prefixed command IDs that panel components may emit as inert action intents.
security: Validates package-prefixed panel IDs, supported slots, bounded component payloads, registered action targets, provenance, and conflicts; does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, direct Masonry widgets, native widget handles, raw CSS, renderer callbacks, unregistered action authority, or external authority.
agent_guidance: Use `clay.ui.serverRegisterPanelContribution` for inert fixed package panels only; do not invent raw ops, native widget handles, Masonry APIs, raw CSS, client-side JavaScript hooks, or hidden layout configuration keys.
lookup_tags: [ui, package-ui, panel, slot-layout, clay-js-api, phase18.3, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterPanelContribution

## Summary

Register a package-prefixed fixed panel contribution for a Clay pane slot through the runtime-backed `clay:ui` facade.

## Description

`serverRegisterPanelContribution` accepts an already validated package manifest and a declarative fixed panel contribution. Clay validates the package prefix, fixed slot, default visibility, bounded component tree, action targets, duplicate IDs, duplicate exclusive slot claims, and package provenance before storing the panel in the server-owned package UI registry.

Accepted declarations become inert shell/runtime state. The Rust client composes fixed panels through Clay-owned `PaneSlotLayout` geometry and native Masonry rendering; package JavaScript, raw CSS, raw ops, renderer callbacks, and package-owned widgets never enter Masonry paint, layout, pointer, scroll, keypress, or text-event handlers.

## When to use

Use this API from package load or configuration-time server JavaScript when a package needs a fixed side/top/bottom panel, such as a Markdown preview, outline, diagnostics list, or package status panel. Use `serverRegisterTransientOverlayContribution` for dismissible overlays.

## JavaScript usage

```ts
import { serverRegisterPanelContribution } from "clay:ui";

const result = serverRegisterPanelContribution(manifest, {
  id: "markdown.preview",
  slot: "right",
  kind: "fixed",
  defaultVisibility: "hidden",
  actionTargets: ["markdown.togglePreview"],
  component: {
    kind: "panel",
    id: "markdown.preview.root",
    title: "Preview",
    children: [],
  },
});
```

## Example

```ts
const panel = serverRegisterPanelContribution(manifest, {
  id: "git.status",
  slot: "left",
  defaultVisibility: "visible",
  component: {
    kind: "panel",
    id: "git.status.root",
    title: "Git Status",
    children: [
      { kind: "label", id: "git.status.empty", text: "No changes" },
    ],
  },
  actionTargets: [],
});

console.log(panel.id, panel.slot, panel.componentId);
```

## Options

- `id` (`string`, default `package-prefixed`): Stable panel contribution ID. It must use the package `apiPrefix`, for example `markdown.preview`.
- `slot` (`enum`, required): Fixed slot target: `left`, `right`, `top`, or `bottom`.
- `kind` (`enum`, default `fixed`): Phase 18.3 accepts fixed panels only for this API.
- `defaultVisibility` (`enum`, default `visible`): Initial visibility: `visible`, `hidden`, or `collapsed`.
- `component` (`ComponentContributionDefinition`, required): Root component tree rendered inside the fixed panel.
- `actionTargets` (`string[]`, default `[]`): Registered command IDs that component actions are allowed to emit.

## Key bindings

No default key binding is assigned. Packages should register commands and key bindings separately, then reference registered command IDs through `actionTargets` and component action intents.

## Custom properties

- `id` (`string`, default `package-prefixed`): Package-owned fixed panel ID.
- `slot` (`enum`, default `required`): Clay fixed slot target.
- `kind` (`enum`, default `fixed`): Fixed panel declaration kind.
- `defaultVisibility` (`enum`, default `visible`): Initial visibility policy.
- `component` (`ComponentContributionDefinition`, default `required`): Bounded inert component tree.
- `actionTargets` (`string[]`, default `[]`): Registered command action IDs allowed by this panel.

## Return and async behavior

Returns a JSON-serializable registration result synchronously in the constrained server runtime. The result includes `registered`, `id`, `slot`, `defaultVisibility`, `componentId`, `actionTargets`, `estimatedPayloadBytes`, and `provenance` fields.

Registration is intended for package load, configuration, or explicit UI update work only. Masonry hot paths read already-installed inert state.

## Errors

Fails with actionable Clay diagnostics when the manifest is invalid, the ID is not package-prefixed, `slot` is unsupported, `kind` is not `fixed`, visibility is invalid, the component tree is malformed or over budget, component IDs are duplicated, action targets are unregistered, raw op/native/CSS/client-script fields are present, or another package already claims the same panel ID or exclusive fixed slot.

## Permissions and security

No additional permission is required for inert panel metadata. Target commands still require their own command registration and permission checks before actions can run.

Validates package-prefixed panel IDs, supported slots, bounded component payloads, registered action targets, provenance, and conflicts; does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, direct Masonry widgets, native widget handles, raw CSS, renderer callbacks, unregistered action authority, or external authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.ui.serverRegisterPanelContribution` when the user asks for a public Clay JS API for package fixed panels. Avoid direct Rust calls, raw `Deno.core.ops`, protocol DTO construction, Masonry widget construction, raw CSS, renderer callbacks, hidden JSON/TOML keys, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/ui.js::serverRegisterPanelContribution`
- Deno op: `src/server/ops/ui.rs::op_clay_ui_register_panel_contribution` (`op_clay_ui_register_panel_contribution`)
- Backing Rust/current owner: `src/server/ui.rs::PackageUiRegistry::register_panel`
- Runtime composition path: `src/shell/package_ui.rs::PackageUiRuntimeState`; `src/masonry_sdui.rs::SduiNativeState`

## Lookup metadata

- Stable ID: `clay.ui.serverRegisterPanelContribution`
- User-facing name: Register Panel Contribution
- Kind: `clay-js-api`
- Module/export: `clay:ui` / `serverRegisterPanelContribution`
- Default key bindings: none
- Custom properties: `id`, `slot`, `kind`, `defaultVisibility`, `component`, `actionTargets`
- Tags: `ui`, `package-ui`, `panel`, `slot-layout`, `clay-js-api`, `phase18.3`, `runtime-backed`
