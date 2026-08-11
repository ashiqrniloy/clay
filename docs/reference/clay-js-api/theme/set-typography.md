---
id: theme.setTypography
kind: clay-js-api
js_module: "clay:theme"
js_export: setTypography
js_facade: runtime/js/theme.js::setTypography
backing_rust: src/server/ops/typography.rs::op_clay_theme_set_typography; src/server/mod.rs::RuntimeGenerationStore::replace_typography
deno_op: op_clay_theme_set_typography
deno_op_path: src/server/ops/typography.rs::op_clay_theme_set_typography
name: setTypography
user_facing_name: Set Typography
summary: Atomically configure Clay's monospace, proportional, and UI font-family fallback stacks, logical-pixel sizes, optional per-role ligature/feature policies, and optional UI text-variant hierarchy.
owner: server
phase: Phase 18.16.5
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: monospace.families
    type: string[]
    default: '["monospace"]'
    description: Ordered code-text family stack ending in a generic fallback; 1 to 8 entries, each at most 128 bytes.
  - name: monospace.size
    type: number
    default: 20
    description: Monospace base size in logical pixels, from 6 through 96 inclusive.
  - name: monospace.ligatures
    type: 'object | undefined'
    default: '{ enableStandard: true, enableContextual: true }'
    description: Optional OpenType ligature/feature policy for code text. Fields — enableStandard (liga+clig, default true), enableContextual (calt, default true), discretionaryFeatures (up to 32 feature tags to enable), rawFeatures (CSS-font-features-format string, at most 256 bytes), disableFeatures (up to 32 feature tags forced off). Omission keeps standard and contextual ligatures enabled.
  - name: proportional.families
    type: string[]
    default: '["sans-serif"]'
    description: Ordered prose family stack ending in a generic fallback; 1 to 8 entries, each at most 128 bytes.
  - name: proportional.size
    type: number
    default: 20
    description: Proportional base size in logical pixels, from 6 through 96 inclusive.
  - name: proportional.ligatures
    type: 'object | undefined'
    default: '{ enableStandard: true, enableContextual: true }'
    description: Optional OpenType ligature/feature policy for prose text; same schema as monospace.ligatures.
  - name: ui.families
    type: string[]
    default: '["system-ui"]'
    description: Ordered Clay UI family stack ending in a generic fallback; 1 to 8 entries, each at most 128 bytes.
  - name: ui.size
    type: number
    default: 12
    description: UI base size in logical pixels, from 6 through 96 inclusive.
  - name: ui.ligatures
    type: 'object | undefined'
    default: '{ enableStandard: true, enableContextual: true }'
    description: Optional OpenType ligature/feature policy for Clay UI text; same schema as monospace.ligatures.
  - name: hierarchy
    type: 'object | undefined'
    default: '{ display: 1.5, title: 14/12, section: 13/12, body: 1, status: 1, detail: 10/12, caption: 0.75 }'
    description: Optional complete bounded UI variant scale ratios. Omission uses Clay defaults; when present all seven fields are required and each must be a finite number greater than 0 and at most 4.
  - name: hierarchy.display
    type: number
    default: '1.5'
    description: Display heading scale ratio (multiplies the selected role base size).
  - name: hierarchy.title
    type: number
    default: '14/12'
    description: Title scale ratio.
  - name: hierarchy.section
    type: number
    default: '13/12'
    description: Section heading scale ratio.
  - name: hierarchy.body
    type: number
    default: '1'
    description: Body scale ratio.
  - name: hierarchy.status
    type: number
    default: '1'
    description: Status chrome scale ratio.
  - name: hierarchy.detail
    type: number
    default: '10/12'
    description: Detail/secondary scale ratio.
  - name: hierarchy.caption
    type: number
    default: '0.75'
    description: Caption scale ratio.
security: Accepts one bounded inert three-profile value plus an optional bounded UI hierarchy and does not grant filesystem, network, shell, font-download, package, extension loading, workspace mutation, AI mutation, native-widget, WASM, raw-op, raw CSS, renderer callback, or client-side JavaScript authority. Clay does not inspect installed fonts; packages may select semantic roles and variants but cannot override these concrete user values or hierarchy scales.
agent_guidance: Call once with all three profiles from init.js or a local configuration module. End every family stack with monospace, sans-serif, serif, system-ui, cursive, or fantasy. Never suggest font URLs, paths, bytes, downloads, package-controlled families, or partial profile updates.
lookup_tags: [js-api, typography, fonts, monospace, proportional, ui, configuration, init, phase18.16.5]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# setTypography

## Summary

Atomically configures user-owned monospace, proportional, and UI typography profiles.

## Description

`setTypography` validates one complete three-profile object before replacing active typography. Each profile contains an ordered family fallback stack and logical-pixel base size. The server publishes one bounded snapshot to clients at bootstrap or after a changed configuration evaluation; native layout then reads cached client state without JavaScript or IPC.

Each profile may also carry an optional `ligatures` policy (Plan 071 task 7): semantic toggles first — `enableStandard` (`liga` + `clig`) and `enableContextual` (`calt`) — with bounded `discretionaryFeatures`/`disableFeatures` tag lists and a `rawFeatures` CSS-format escape hatch for stylistic alternates. Absent fields default to ligatures enabled; disabling is explicit user configuration. A mode's font role selects which profile's policy applies to its document text, so ligature preferences follow the typography role rather than individual modes or packages.

Modes and packages may choose semantic roles such as monospace or proportional. They cannot choose concrete family names, sizes, or ligature policies, which remain user-owned here.

## When to use

Use from `~/.config/clay/init.js` when Clay's default typography does not match your preferred fonts or sizes. No call is needed to use defaults.

## JavaScript usage

```ts
import { setTypography } from "clay:theme";

setTypography({
  monospace: { families: ["JetBrains Mono", "monospace"], size: 16 },
  proportional: { families: ["Inter", "sans-serif"], size: 17 },
  ui: { families: ["system-ui"], size: 13 },
});
```

With an optional per-role ligature policy (here disabling contextual alternates for code):

```ts
setTypography({
  monospace: {
    families: ["JetBrains Mono", "monospace"],
    size: 16,
    ligatures: { enableStandard: true, enableContextual: false },
  },
  proportional: { families: ["Inter", "sans-serif"], size: 17 },
  ui: { families: ["system-ui"], size: 13 },
});
```

With an optional UI variant hierarchy:

```ts
setTypography({
  monospace: { families: ["monospace"], size: 16 },
  proportional: { families: ["sans-serif"], size: 16 },
  ui: { families: ["system-ui"], size: 13 },
  hierarchy: {
    display: 1.5,
    title: 14 / 12,
    section: 13 / 12,
    body: 1,
    status: 1,
    detail: 10 / 12,
    caption: 0.75,
  },
});
```

## Example

```js
import { setTypography } from "clay:theme";

setTypography({
  monospace: { families: ["monospace"], size: 18 },
  proportional: { families: ["sans-serif"], size: 18 },
  ui: { families: ["system-ui"], size: 14 },
});
```

## Modular configuration

`init.js` may load a local module under the configuration root:

```js
import { loadConfigurationModule } from "clay:configuration";

await loadConfigurationModule({ path: "./typography.js" });
```

```js
// ~/.config/clay/typography.js
import { setTypography } from "clay:theme";

setTypography({
  monospace: { families: ["monospace"], size: 20 },
  proportional: { families: ["sans-serif"], size: 20 },
  ui: { families: ["system-ui"], size: 12 },
});
```

## Defaults

- `monospace`: `{ families: ["monospace"], size: 20 }`
- `proportional`: `{ families: ["sans-serif"], size: 20 }`
- `ui`: `{ families: ["system-ui"], size: 12 }`
- `hierarchy` (omitted): `{ display: 1.5, title: 14/12, section: 13/12, body: 1, status: 1, detail: 10/12, caption: 0.75 }`

## Options and allowed values

Pass exactly `monospace`, `proportional`, and `ui`, plus an optional `hierarchy`. Each profile must contain exactly:

- `families`: 1–8 non-empty strings, each at most 128 UTF-8 bytes and without control characters.
- `size`: finite number from `6` through `96`, inclusive, interpreted as logical pixels.

Each family stack must end with one supported generic fallback: `system-ui`, `serif`, `sans-serif`, `monospace`, `cursive`, or `fantasy`. Earlier entries may be named installed families. Clay does not test whether names are installed; local font resolution proceeds through the ordered stack and generic fallback.

When `hierarchy` is present it must contain exactly the seven named scale fields `display`, `title`, `section`, `body`, `status`, `detail`, and `caption`. Each must be a finite number greater than `0` and at most `4`; the ratio multiplies the selected role's base size to produce a variant's pixel size. Omitting `hierarchy` keeps Clay defaults. Partial hierarchies are rejected atomically.

Unknown fields, partial profiles, partial hierarchies, and separate per-profile updates are rejected. There is no parallel JSON, TOML, environment-variable, or package setting for concrete typography.

## Return and async behavior

Synchronous. Returns `{ revision }` after the complete candidate passes runtime validation. No `await` is needed. The server assigns persistent revisions when applying changed evaluation state and sends one bounded update to each client.

Configuration evaluation and publication occur outside keypress, text-event, paint, layout, pointer, and scroll hot paths. Those paths use validated cached typography and layout state.

## Reload behavior

A successful configuration reload atomically replaces all three profiles. If profiles are unchanged, clients are not invalidated again. Invalid JavaScript or typography leaves the previous complete server state active and reports a sanitized runtime diagnostic. Removing the call and successfully reloading restores Clay defaults.

## Errors

Throws `theme.invalid_typography` when the value is not one complete object, contains missing or unknown fields, exceeds the bounded payload, has an invalid family stack, lacks a final generic fallback, uses a non-finite/out-of-range size, omits or adds extra hierarchy fields, or supplies a non-finite/out-of-range hierarchy scale. The operation does not partially install valid profiles or scales from a rejected object.

## Permissions and security

No permissions are required. Values are inert names and sizes only.

:Clay does not enumerate installed fonts on the server, open font files, fetch font URLs, or download fonts. This API grants no filesystem, network, shell, package, extension loading, workspace mutation, AI mutation, native-widget, WASM, raw `Deno.core.ops`, raw CSS, renderer callback, or client-side JavaScript authority. Package modes, syntax spans, and UI components may select validated semantic font roles and variants only; they cannot override user family stacks, sizes, or hierarchy scales.

## Agent guidance

Prefer one complete call and preserve a generic fallback at the end of every stack. Do not invent separate font keys, profile setters, CSS, paths, URLs, bytes, download steps, or package-controlled concrete typography.

## Backing implementation

- Facade: `runtime/js/theme.js::setTypography`
- Deno op: `src/server/ops/typography.rs::op_clay_theme_set_typography`
- Server state/publication: `src/server/mod.rs::RuntimeGenerationStore::replace_typography`
- Client resolution: `src/editor/typography.rs::TypographyRegistry`

## Lookup metadata

- id: `theme.setTypography`
- module: `clay:theme`
- export: `setTypography`
- lookup tags: `js-api`, `typography`, `fonts`, `monospace`, `proportional`, `ui`, `configuration`, `init`, `phase18.16.5`

## Key bindings

No default key bindings. This API is startup/reload configuration, not key routing.

## Custom properties

- `monospace.families`
  - name: monospace.families
  - type: string[]
  - default: `["monospace"]`
- `monospace.size`
  - name: monospace.size
  - type: number
  - default: `20`
- `proportional.families`
  - name: proportional.families
  - type: string[]
  - default: `["sans-serif"]`
- `proportional.size`
  - name: proportional.size
  - type: number
  - default: `20`
- `ui.families`
  - name: ui.families
  - type: string[]
  - default: `["system-ui"]`
- `ui.size`
  - name: ui.size
  - type: number
  - default: `12`
- `hierarchy`
  - name: hierarchy
  - type: object | undefined
  - default: `{ display: 1.5, title: 14/12, section: 13/12, body: 1, status: 1, detail: 10/12, caption: 0.75 }`
- `hierarchy.display`
  - name: hierarchy.display
  - type: number
  - default: `1.5`
- `hierarchy.title`
  - name: hierarchy.title
  - type: number
  - default: `14/12`
- `hierarchy.section`
  - name: hierarchy.section
  - type: number
  - default: `13/12`
- `hierarchy.body`
  - name: hierarchy.body
  - type: number
  - default: `1`
- `hierarchy.status`
  - name: hierarchy.status
  - type: number
  - default: `1`
- `hierarchy.detail`
  - name: hierarchy.detail
  - type: number
  - default: `10/12`
- `hierarchy.caption`
  - name: hierarchy.caption
  - type: number
  - default: `0.75`
