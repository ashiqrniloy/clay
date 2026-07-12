---
id: clay.theme.setTypography
kind: clay-js-api
js_module: "clay:theme"
js_export: setTypography
js_facade: runtime/js/theme.ts::setTypography
backing_rust: src/server/ops/typography.rs::op_clay_theme_set_typography; src/server/mod.rs::RuntimeGenerationStore::replace_typography
deno_op: op_clay_theme_set_typography
deno_op_path: src/server/ops/typography.rs::op_clay_theme_set_typography
name: setTypography
user_facing_name: Set Typography
summary: Atomically configure Clay's monospace, proportional, and UI font-family fallback stacks and logical-pixel sizes.
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
  - name: proportional.families
    type: string[]
    default: '["sans-serif"]'
    description: Ordered prose family stack ending in a generic fallback; 1 to 8 entries, each at most 128 bytes.
  - name: proportional.size
    type: number
    default: 20
    description: Proportional base size in logical pixels, from 6 through 96 inclusive.
  - name: ui.families
    type: string[]
    default: '["system-ui"]'
    description: Ordered Clay UI family stack ending in a generic fallback; 1 to 8 entries, each at most 128 bytes.
  - name: ui.size
    type: number
    default: 12
    description: UI base size in logical pixels, from 6 through 96 inclusive.
security: Accepts one bounded inert three-profile value and does not grant filesystem, network, shell, font-download, package, extension loading, workspace mutation, AI mutation, native-widget, WASM, raw-op, raw CSS, renderer callback, or client-side JavaScript authority. Clay does not inspect installed fonts; packages may select semantic roles but cannot override these concrete user values.
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

Modes and packages may choose semantic roles such as monospace or proportional. They cannot choose concrete family names or sizes, which remain user-owned here.

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

## Options and allowed values

Pass exactly `monospace`, `proportional`, and `ui`. Each must contain exactly:

- `families`: 1–8 non-empty strings, each at most 128 UTF-8 bytes and without control characters.
- `size`: finite number from `6` through `96`, inclusive, interpreted as logical pixels.

Each family stack must end with one supported generic fallback: `system-ui`, `serif`, `sans-serif`, `monospace`, `cursive`, or `fantasy`. Earlier entries may be named installed families. Clay does not test whether names are installed; local font resolution proceeds through the ordered stack and generic fallback.

Unknown fields, partial profiles, and separate per-profile updates are rejected. There is no parallel JSON, TOML, environment-variable, or package setting for concrete typography.

## Return and async behavior

Synchronous. Returns `{ revision }` after the complete candidate passes runtime validation. No `await` is needed. The server assigns persistent revisions when applying changed evaluation state and sends one bounded update to each client.

Configuration evaluation and publication occur outside keypress, text-event, paint, layout, pointer, and scroll hot paths. Those paths use validated cached typography and layout state.

## Reload behavior

A successful configuration reload atomically replaces all three profiles. If profiles are unchanged, clients are not invalidated again. Invalid JavaScript or typography leaves the previous complete server state active and reports a sanitized runtime diagnostic. Removing the call and successfully reloading restores Clay defaults.

## Errors

Throws `clay.theme.invalid_typography` when the value is not one complete object, contains missing or unknown fields, exceeds the bounded payload, has an invalid family stack, lacks a final generic fallback, or uses a non-finite/out-of-range size. The operation does not partially install valid profiles from a rejected object.

## Permissions and security

No permissions are required. Values are inert names and sizes only.

Clay does not enumerate installed fonts on the server, open font files, fetch font URLs, or download fonts. This API grants no filesystem, network, shell, package, extension loading, workspace mutation, AI mutation, native-widget, WASM, raw `Deno.core.ops`, raw CSS, renderer callback, or client-side JavaScript authority. Package modes, syntax spans, and UI components may select validated semantic font roles only; they cannot override user family stacks or sizes.

## Agent guidance

Prefer one complete call and preserve a generic fallback at the end of every stack. Do not invent separate font keys, profile setters, CSS, paths, URLs, bytes, download steps, or package-controlled concrete typography.

## Backing implementation

- Facade: `runtime/js/theme.ts::setTypography`
- Deno op: `src/server/ops/typography.rs::op_clay_theme_set_typography`
- Server state/publication: `src/server/mod.rs::RuntimeGenerationStore::replace_typography`
- Client resolution: `src/editor/typography.rs::TypographyRegistry`

## Lookup metadata

- id: `clay.theme.setTypography`
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
