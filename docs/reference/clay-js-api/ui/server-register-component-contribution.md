---
id: clay.ui.serverRegisterComponentContribution
kind: clay-js-api
js_module: "clay:ui"
js_export: serverRegisterComponentContribution
js_facade: runtime/js/ui.js::serverRegisterComponentContribution
backing_rust: src/server/ui.rs::PackageUiRegistry::register_component
deno_op: op_clay_ui_register_component_contribution
deno_op_path: src/server/ops/ui.rs::op_clay_ui_register_component_contribution
name: serverRegisterComponentContribution
user_facing_name: Register Component Contribution
summary: Register a bounded inert Clay component tree for package UI through the runtime-backed `clay:ui` facade.
owner: server
phase: Phase 18.3
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: id
    type: string
    default: package-prefixed
    description: Root component ID that must use the package API prefix.
  - name: kind
    type: enum
    default: required
    description: Supported Phase 18.3 component kind such as `panel`, `label`, `button`, `list`, `flex`, `stack`, `overlay`, `scroll`, `portal`, `statusItem`, or `editorView`.
  - name: deferredKinds
    type: enum
    default: table|dropdown|collapse|modal
    description: Explicitly deferred component kinds that are rejected until later phases.
  - name: children
    type: ComponentContributionDefinition[]
    default: []
    description: Bounded child component declarations.
  - name: styleTokens
    type: string[]
    default: []
    description: Typed style-variable token references resolved through Clay core or package theme tokens.
  - name: actionTargets
    type: string[]
    default: []
    description: Registered package-prefixed command IDs referenced by component action intents.
security: Validates component kind, duplicate component IDs, package-prefixed IDs, bounded payloads, typed style-token references, registered action targets, provenance, and prohibited fields; does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, direct Masonry widgets, native widget handles, raw CSS, renderer callbacks, native component mutation authority, or external authority.
agent_guidance: Use `clay.ui.serverRegisterComponentContribution` for declarative component trees only; keep native rendering, layout, style resolution, and action execution Clay-owned and avoid raw Rust, raw ops, Masonry names, CSS strings, or executable client hooks.
lookup_tags: [ui, package-ui, component-catalog, style-tokens, clay-js-api, phase18.3, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterComponentContribution

## Summary

Register a bounded inert Clay component tree for package UI through the runtime-backed `clay:ui` facade.

## Description

`serverRegisterComponentContribution` validates and stores a package-owned component root that can be reused by package panels and overlays. Clay validates component IDs, supported component kinds, child traversal limits, typed style variables, theme-token compatibility, action intents, and prohibited authority fields before the component can affect native UI state.

The API documents Clay's package-facing component catalog. It does not expose Masonry widgets, Vello/Parley callbacks, raw CSS, raw op names, native handles, or executable client-side JavaScript.

## When to use

Use this API when a package wants to register a reusable component tree independent of a fixed panel or overlay declaration. For inline component trees inside `serverRegisterPanelContribution` or `serverRegisterTransientOverlayContribution`, Clay runs the same validator through those APIs.

## JavaScript usage

```ts
import { serverRegisterComponentContribution } from "clay:ui";

const component = serverRegisterComponentContribution(manifest, {
  id: "markdown.preview.body",
  kind: "scroll",
  style: {
    background: "markdown.preview.background",
    padding: "spacing.panel",
  },
  children: [
    { kind: "label", id: "markdown.preview.empty", text: "Preview unavailable" },
  ],
});
```

## Example

```ts
const toolbar = serverRegisterComponentContribution(manifest, {
  id: "markdown.preview.toolbar",
  kind: "flex",
  style: { gap: "spacing.row" },
  children: [
    {
      kind: "button",
      id: "markdown.preview.refresh",
      label: "Refresh",
      action: { commandId: "markdown.refreshPreview" },
    },
  ],
});

console.log(toolbar.id, toolbar.rootKind, toolbar.componentCount);
```

## Options

- `id` (`string`, default `package-prefixed`): Root component ID. It must use the package `apiPrefix`.
- `kind` (`enum`, required): Supported kind: `editorView`, `panel`, `label`, `button`, `list`, `flex`, `stack`, `overlay`, `scroll`, `portal`, or `statusItem`.
- `deferredKinds` (`enum`, default `table|dropdown|collapse|modal`): `table`, `dropdown`, `collapse`, and `modal` are explicitly deferred and rejected with diagnostics.
- `children` (`ComponentContributionDefinition[]`, default `[]`): Bounded child component declarations.
- `styleTokens` (`string[]`, default `[]`): Typed style-variable token references through known Clay core tokens or package theme tokens.
- `actionTargets` (`string[]`, default `[]`): Registered command IDs referenced by action intents in this tree.

## Key bindings

No default key binding is assigned. Components may emit inert command intents only for registered commands; key routing belongs to documented command/keybinding APIs.

## Custom properties

- `id` (`string`, default `package-prefixed`): Root component ID.
- `kind` (`enum`, default `required`): Component catalog kind.
- `deferredKinds` (`enum`, default `table|dropdown|collapse|modal`): Rejected future component kinds.
- `children` (`ComponentContributionDefinition[]`, default `[]`): Bounded child declarations.
- `styleTokens` (`string[]`, default `[]`): Typed style-token references.
- `actionTargets` (`string[]`, default `[]`): Registered command action IDs.

## Return and async behavior

Returns a JSON-serializable registration result synchronously in the constrained server runtime. The result includes `registered`, `id`, `rootKind`, `componentCount`, `styleVariableCount`, `actionTargets`, `estimatedPayloadBytes`, and `provenance` fields.

Validation occurs during package load, configuration, or explicit UI update work. Paint and layout read already-validated component/theme state without package JavaScript or unbounded parsing.

## Errors

Fails when the manifest is invalid, IDs are not package-prefixed, the kind is unknown or deferred, child traversal exceeds Clay budgets, component IDs are duplicated, style variables are unsupported, tokens are unknown or type-incompatible, action targets are unregistered, payloads are oversize, or raw op/native handle/CSS/renderer/client-script fields are present.

## Permissions and security

No additional permission is required for inert component metadata. Component action targets must refer to registered commands whose own permissions and routing policies are validated separately.

Validates component kind, duplicate component IDs, package-prefixed IDs, bounded payloads, typed style-token references, registered action targets, provenance, and prohibited fields; does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, direct Masonry widgets, native widget handles, raw CSS, renderer callbacks, native component mutation authority, or external authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.ui.serverRegisterComponentContribution` when the user asks for a public Clay JS API for package component trees or style-token-validated component catalog entries. Do not bypass with raw Rust constructors, raw `Deno.core.ops`, protocol DTOs, Masonry widgets, raw CSS, renderer callbacks, hidden config keys, or client-side JavaScript execution.

## Backing implementation

- JS facade: `runtime/js/ui.js::serverRegisterComponentContribution`
- Deno op: `src/server/ops/ui.rs::op_clay_ui_register_component_contribution` (`op_clay_ui_register_component_contribution`)
- Backing Rust/current owner: `src/server/ui.rs::PackageUiRegistry::register_component`
- Component catalog and token validation: `src/shell/components.rs`; `src/shell/theme.rs`

## Lookup metadata

- Stable ID: `clay.ui.serverRegisterComponentContribution`
- User-facing name: Register Component Contribution
- Kind: `clay-js-api`
- Module/export: `clay:ui` / `serverRegisterComponentContribution`
- Default key bindings: none
- Custom properties: `id`, `kind`, `deferredKinds`, `children`, `styleTokens`, `actionTargets`
- Tags: `ui`, `package-ui`, `component-catalog`, `style-tokens`, `clay-js-api`, `phase18.3`, `runtime-backed`
