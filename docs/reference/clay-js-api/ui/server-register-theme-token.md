---
id: ui.serverRegisterThemeToken
kind: clay-js-api
js_module: "clay:ui"
js_export: serverRegisterThemeToken
js_facade: runtime/js/ui.js::serverRegisterThemeToken
backing_rust: src/server/ui.rs::PackageUiRegistry::register_theme_token
deno_op: op_clay_ui_register_theme_token
deno_op_path: src/server/ops/ui.rs::op_clay_ui_register_theme_token
name: serverRegisterThemeToken
user_facing_name: Register Theme Token
summary: Register a package-prefixed typed theme token declaration through the runtime-backed `clay:ui` facade.
owner: server
phase: Phase 18.3
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: token
    type: string
    default: package-prefixed
    description: Semantic package token name such as `markdown.preview.background`.
  - name: type
    type: enum
    default: required
    description: Token type, one of `color-role`, `spacing`, `radius`, `typography`, `opacity`, `dimension`, `elevation`, `motion-duration`, `z-level`, or `density`.
  - name: fallback
    type: string
    default: clay-core-token-same-type
    description: Same-type Clay core token used when no future user override is present.
  - name: description
    type: string
    default: required
    description: Human-readable semantic purpose for diagnostics, help, and future configuration UIs.
  - name: source
    type: enum
    default: package
    description: Provenance source for the declaration; Phase 18.3 accepts package declarations.
security: Validates package-prefixed semantic token names, token types, same-type Clay core fallbacks, descriptions, provenance, and token conflicts; does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, raw CSS, raw style strings, arbitrary raw colors, direct client widgets, native widget handles, renderer callbacks, arbitrary GPU draw authority, or external authority.
agent_guidance: Use `ui.serverRegisterThemeToken` for semantic package theme tokens only; do not expose raw colors, CSS, renderer callbacks, native handles, raw ops, hidden theme override keys, or client-side JavaScript style hooks.
lookup_tags: [ui, package-ui, theme-token, style-tokens, clay-js-api, phase18.3, runtime-backed]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterThemeToken

## Summary

Register a package-prefixed typed theme token declaration through the runtime-backed `clay:ui` facade.

## Description

`serverRegisterThemeToken` declares a semantic package theme token and maps it to a same-type Clay core fallback. Clay validates package prefix, token type, fallback type compatibility, description text, prohibited raw value fields, duplicate tokens, and provenance before making the token available to component style validation.

The token declaration is not a user override API. Phase 18.3 package tokens defined typed contracts for `color-role`, `spacing`, `radius`, `typography`, and `opacity`. Phase 20.1 extended the typed catalog additively with `dimension` (panel/border logical-pixel defaults), `elevation` (near-invisible levels), `motion-duration` (bounded transition durations), `z-level` (ordered overlay stacking), and `density` (compact/default/spacious intent). Clay resolves every declared package token to a same-typed Clay core fallback; future user overrides must use documented configuration APIs.

## When to use

Use this API when a package component style needs a semantic token name rather than a hardcoded Clay core token. For example, Markdown can declare `markdown.preview.background` and fall back to `surface.panel` while still allowing future user configuration to remap the package token safely.

## JavaScript usage

```ts
import { serverRegisterThemeToken } from "clay:ui";

const token = serverRegisterThemeToken(manifest, {
  token: "markdown.preview.background",
  type: "color-role",
  fallback: "surface.panel",
  description: "Markdown preview panel background",
});
```

## Example

```ts
const spacing = serverRegisterThemeToken(manifest, {
  token: "markdown.preview.padding",
  type: "spacing",
  fallback: "spacing.panel",
  description: "Padding around the Markdown preview body",
});

console.log(spacing.token, spacing.type, spacing.resolvedCoreToken);
```

## Options

- `token` (`string`, default `package-prefixed`): Semantic token name. It must use the package `apiPrefix`.
- `type` (`enum`, required): Token type: `color-role`, `spacing`, `radius`, `typography`, `opacity`, `dimension`, `elevation`, `motion-duration`, `z-level`, or `density`.
- `fallback` (`string`, default `clay-core-token-same-type`): Clay core fallback token of the same type, such as `surface.panel` for `color-role` or `spacing.panel` for `spacing`.
- `description` (`string`, required): Human-readable semantic purpose for diagnostics, help, and future configuration UIs.
- `source` (`enum`, default `package`): Declaration provenance. Phase 18.3 accepts package declarations; user overrides remain planned.

## Key bindings

No default key binding is assigned. Theme token declarations are package load/configuration metadata and should not be bound directly to user input.

## Custom properties

- `token` (`string`, default `package-prefixed`): Package semantic token name.
- `type` (`enum`, default `required`): Typed token kind.
- `fallback` (`string`, default `clay-core-token-same-type`): Same-type Clay core fallback token.
- `description` (`string`, default `required`): Token purpose.
- `source` (`enum`, default `package`): Declaration provenance.

## Return and async behavior

Returns a JSON-serializable registration result synchronously in the constrained server runtime. The result includes `registered`, `token`, `type`, `fallback`, `description`, `resolvedCoreToken`, `estimatedPayloadBytes`, and `provenance` fields.

Registration and token resolution happen during package load, configuration, or explicit UI update work. Paint and layout read cached resolved token values without package JavaScript, raw style parsing, or unbounded allocation.

## Errors

Fails when the manifest is invalid, the token is not package-prefixed, the type is unsupported, the fallback is unknown or type-incompatible, the description is missing, raw `value`, `rawColor`, `css`, raw style, renderer callback, native handle, or raw op fields are present, the payload is oversize, or another package already registered the same token.

## Permissions and security

No additional permission is required for inert package theme token declarations. Future user token overrides remain a separate planned configuration surface and are not granted by this API.

Validates package-prefixed semantic token names, token types, same-type Clay core fallbacks, descriptions, provenance, and token conflicts; does not grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package enable/disable, WASM, client-side JavaScript, raw Deno ops, raw CSS, raw style strings, arbitrary raw colors, direct client widgets, native widget handles, renderer callbacks, arbitrary GPU draw authority, or external authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `ui.serverRegisterThemeToken` when the user asks for a public Clay JS API for package theme token declarations. Avoid raw color values, raw CSS, renderer callbacks, native style mutation, direct Rust calls, raw `Deno.core.ops`, hidden override keys, or client-side JavaScript styling hooks.

## Backing implementation

- JS facade: `runtime/js/ui.js::serverRegisterThemeToken`
- Deno op: `src/server/ops/ui.rs::op_clay_ui_register_theme_token` (`op_clay_ui_register_theme_token`)
- Backing Rust/current owner: `src/server/ui.rs::PackageUiRegistry::register_theme_token`
- Token resolver: `src/shell/theme.rs::ThemeTokenResolver`

## Lookup metadata

- Stable ID: `ui.serverRegisterThemeToken`
- User-facing name: Register Theme Token
- Kind: `clay-js-api`
- Module/export: `clay:ui` / `serverRegisterThemeToken`
- Default key bindings: none
- Custom properties: `token`, `type`, `fallback`, `description`, `source`
- Tags: `ui`, `package-ui`, `theme-token`, `style-tokens`, `clay-js-api`, `phase18.3`, `runtime-backed`
