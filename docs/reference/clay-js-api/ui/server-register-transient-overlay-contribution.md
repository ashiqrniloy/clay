---
id: clay.ui.serverRegisterTransientOverlayContribution
kind: clay-js-api
js_module: "clay:ui"
js_export: serverRegisterTransientOverlayContribution
js_facade: runtime/js/ui.ts::serverRegisterTransientOverlayContribution
backing_rust: src/server/ui.rs::PackageUiRegistry::register_overlay
deno_op: op_clay_ui_register_transient_overlay_contribution
deno_op_path: src/server/ops/ui.rs::op_clay_ui_register_transient_overlay_contribution
name: serverRegisterTransientOverlayContribution
user_facing_name: Register Transient Overlay Contribution
summary: Register a package-prefixed transient overlay contribution through the runtime-backed `clay:ui` facade.
owner: server
phase: Phase 18.3
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: id
    type: string
    default: package-prefixed
    description: Package-prefixed overlay contribution ID.
  - name: anchor
    type: enum
    default: working-area
    description: Overlay anchor, one of `working-area`, `active-pane`, `main`, or `pointer`.
  - name: component
    type: ComponentContributionDefinition
    default: required
    description: Bounded inert component tree rendered by Clay in the overlay layer.
  - name: focusPolicy
    type: enum
    default: none
    description: Focus behavior, one of `none`, `restore`, or `trap`.
  - name: dismissalPolicy
    type: enum
    default: manual
    description: Dismissal behavior, one of `manual`, `escape`, `outside`, or `escape-or-outside`.
  - name: actionTargets
    type: string[]
    default: []
    description: Registered package-prefixed command IDs that overlay components may emit.
security: Validates package-prefixed overlay IDs, anchors, focus and dismissal policy, bounded component payloads, registered action targets, provenance, and conflicts while Clay owns z-order, focus, accessibility, and native overlay rendering; does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, direct Masonry widgets, native widget handles, raw CSS, renderer callbacks, unregistered action authority, or external authority.
agent_guidance: Use `clay.ui.serverRegisterTransientOverlayContribution` for declarative dismissible package overlays only; avoid fixed-slot panels, raw ops, native widget handles, direct Masonry APIs, raw CSS, renderer callbacks, client-side JavaScript hooks, and hidden focus/z-order settings.
lookup_tags: [ui, package-ui, overlay, focus-policy, clay-js-api, phase18.3, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterTransientOverlayContribution

## Summary

Register a package-prefixed transient overlay contribution through the runtime-backed `clay:ui` facade.

## Description

`serverRegisterTransientOverlayContribution` accepts an already validated package manifest and a declarative transient overlay. Clay validates the package prefix, overlay anchor, focus policy, dismissal policy, component tree, action targets, duplicate IDs, and provenance before storing the overlay in package UI runtime state.

Transient overlays render in an overlay layer and do not consume fixed pane slot geometry. Clay owns z-order, focus restoration/trapping policy, accessibility metadata, dismissal behavior, native layout, and paint. Package code contributes inert metadata only.

## When to use

Use this API when a package needs a command palette, tooltip-like pointer overlay, quick picker, preview popover, or temporary status/detail view. Use `serverRegisterPanelContribution` for persistent fixed slot panels.

## JavaScript usage

```ts
import { serverRegisterTransientOverlayContribution } from "clay:ui";

const overlay = serverRegisterTransientOverlayContribution(manifest, {
  id: "markdown.preview.peek",
  anchor: "main",
  focusPolicy: "restore",
  dismissalPolicy: "escape-or-outside",
  actionTargets: ["markdown.openPreview"],
  component: {
    kind: "panel",
    id: "markdown.preview.peek.root",
    title: "Preview",
    children: [],
  },
});
```

## Example

```ts
const picker = serverRegisterTransientOverlayContribution(manifest, {
  id: "git.branchPicker",
  anchor: "working-area",
  focusPolicy: "trap",
  dismissalPolicy: "escape",
  component: {
    kind: "panel",
    id: "git.branchPicker.root",
    title: "Branches",
    children: [
      { kind: "label", id: "git.branchPicker.loading", text: "Loading branches" },
    ],
  },
  actionTargets: ["git.checkoutBranch"],
});

console.log(picker.id, picker.anchor, picker.focusPolicy);
```

## Options

- `id` (`string`, default `package-prefixed`): Stable overlay contribution ID. It must use the package `apiPrefix`.
- `anchor` (`enum`, default `working-area`): Overlay anchor: `working-area`, `active-pane`, `main`, or `pointer`.
- `component` (`ComponentContributionDefinition`, required): Root component tree rendered inside the overlay layer.
- `focusPolicy` (`enum`, default `none`): Focus behavior: `none`, `restore`, or `trap`.
- `dismissalPolicy` (`enum`, default `manual`): Dismissal behavior: `manual`, `escape`, `outside`, or `escape-or-outside`.
- `actionTargets` (`string[]`, default `[]`): Registered command IDs that overlay actions may emit.

## Key bindings

No default key binding is assigned. Overlays may be opened or dismissed through separately registered commands and Clay-owned focus/dismissal routing.

## Custom properties

- `id` (`string`, default `package-prefixed`): Package-owned overlay ID.
- `anchor` (`enum`, default `working-area`): Overlay geometry anchor.
- `component` (`ComponentContributionDefinition`, default `required`): Bounded inert component tree.
- `focusPolicy` (`enum`, default `none`): Focus behavior metadata.
- `dismissalPolicy` (`enum`, default `manual`): Dismissal behavior metadata.
- `actionTargets` (`string[]`, default `[]`): Registered command action IDs.

## Return and async behavior

Returns a JSON-serializable registration result synchronously in the constrained server runtime. The result includes `registered`, `id`, `anchor`, `focusPolicy`, `dismissalPolicy`, `componentId`, `actionTargets`, `estimatedPayloadBytes`, and `provenance` fields.

Registration is package load, configuration, or explicit UI-command work. Active overlay paint and layout read installed inert state without JavaScript, package validation, IPC waits, or child mutation during layout.

## Errors

Fails when the manifest is invalid, the overlay ID is not package-prefixed, the anchor/focus/dismissal policy is unsupported, the component tree is invalid or over budget, component IDs are duplicated, action targets are unregistered, raw op/native/CSS/client-script/renderer fields are present, or another enabled package has already claimed the same overlay ID.

## Permissions and security

No additional permission is required for inert overlay metadata. Overlay action targets must resolve to registered commands, and those commands retain their own permission and routing constraints.

Validates package-prefixed overlay IDs, anchors, focus and dismissal policy, bounded component payloads, registered action targets, provenance, and conflicts while Clay owns z-order, focus, accessibility, and native overlay rendering; does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, direct Masonry widgets, native widget handles, raw CSS, renderer callbacks, unregistered action authority, or external authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.ui.serverRegisterTransientOverlayContribution` when the user asks for a public Clay JS API for package overlays. Avoid direct Rust calls, raw `Deno.core.ops`, protocol DTO construction, Masonry widgets, raw CSS, renderer callbacks, hidden focus/z-order keys, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/ui.ts::serverRegisterTransientOverlayContribution`
- Deno op: `src/server/ops/ui.rs::op_clay_ui_register_transient_overlay_contribution` (`op_clay_ui_register_transient_overlay_contribution`)
- Backing Rust/current owner: `src/server/ui.rs::PackageUiRegistry::register_overlay`
- Runtime composition path: `src/shell/package_ui.rs::PackageUiRuntimeState`; `src/masonry_sdui.rs::SduiNativeState`

## Lookup metadata

- Stable ID: `clay.ui.serverRegisterTransientOverlayContribution`
- User-facing name: Register Transient Overlay Contribution
- Kind: `clay-js-api`
- Module/export: `clay:ui` / `serverRegisterTransientOverlayContribution`
- Default key bindings: none
- Custom properties: `id`, `anchor`, `component`, `focusPolicy`, `dismissalPolicy`, `actionTargets`
- Tags: `ui`, `package-ui`, `overlay`, `focus-policy`, `clay-js-api`, `phase18.3`, `runtime-backed`
