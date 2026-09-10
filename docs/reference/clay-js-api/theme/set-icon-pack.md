---
id: theme.setIconPack
kind: clay-js-api
js_module: "clay:theme"
js_export: setIconPack
js_facade: runtime/js/theme.js::setIconPack
backing_rust: src/server/ops/theme.rs::op_clay_theme_set_icon_pack; src/server/ops/theme.rs::apply_icon_pack; src/shell/icons.rs::ActiveIconPack::from_record
deno_op: op_clay_theme_set_icon_pack
deno_op_path: src/server/ops/theme.rs::op_clay_theme_set_icon_pack
name: setIconPack
user_facing_name: Set Icon Pack
summary: Explicitly activate an icon pack (a first-party `@clay/icons-*` pack or one enabled package's `iconPack` contribution) for UI glyph rendering; icons ride the active theme's text color and never carry their own colors.
owner: server
phase: Phase 112
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: specifier
    type: string
    default: required
    description: "Name of one icon-pack contributor: a bundled first-party `@clay/icons-*` package or an already-enabled package record that declares a `clay.contributions.iconPack`."
security: Selects inert, bounded vector-geometry data only: paths are host-validated against a bounded schema (schema version, path/coordinate/command limits, absolute commands, no raw SVG, CSS, URLs, scripts, or events) and render through the host's own SVG pipeline with `fill="currentColor"`, so the active theme stays the sole color authority. Bundled `@clay/*` packs resolve from the compiled inventory without executing anything; third-party specifiers must already be enabled package records — selection never installs, adopts, or promotes trust. Package runtime callers cannot reach this op: it is registered only in the trusted runtime extension. On failure or later package revocation the previously valid pack is preserved and a sanitized diagnostic surfaces; missing or failed selections fall back to the bundled default subset. Does not grant filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, AI mutation, native widget, WASM, raw Deno ops, client-side JavaScript, raw CSS, or renderer callback authority.
agent_guidance: Use the bundled default (no `setIconPack` call at all) unless the user asks for a specific icon pack; zero configuration renders the bundled Regular subset. Never suggest raw SVG markup, package-owned colors, or icon packs from untrusted packages. If the selected pack is later removed or revoked, Clay keeps working controls with the bundled fallback and surfaces a sanitized diagnostic on the next reload.
lookup_tags: [theme, icons, icon-pack, ui, init, fallback]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# setIconPack

## Summary

Explicitly activate an icon pack for UI glyph rendering: a bundled first-party `@clay/icons-*` package, or one enabled package's `iconPack` contribution. Icons are colorized solely by the active theme — packs ship geometry, never colors.

## Description

`setIconPack` stores a validated `ActiveIconPack` snapshot on the server. A `@clay/` specifier resolves from the compiled bundled inventory without executing package code; any other specifier must resolve to an already-enabled package record with a `clay.contributions.iconPack` declaration (load ≠ select: selection never installs, adopts, or promotes trust). Each icon is a bounded vector-geometry set (schema version, viewBox bounds, at most 8 paths, absolute `M|L|C|Q|H|V|A|Z` commands, per-icon and per-pack byte budgets) validated by the host before storage. The resolved snapshot ships to the client inside the runtime state snapshot with a generation stamp; the React client stores it and `ClayIcon` renders semantic references (`action.close`, `document.save`, `git.branch`, …) from the active pack, falling back to the bundled default subset for missing or unknown keys so no control is ever blank.

Icon selection is independent of theme, appearance, and design-system selections: all four resolve concurrently and swapping the pack preserves the others.

## When to use

Use from `~/.clay/init.js` (or a local configuration module) to select the icon style at startup. Omit the call entirely for the zero-configuration default: the bundled Regular subset is active with no init.js icon lines and no package load entry. The recommended explicit path for third-party packs is `loadPackage` then `setIconPack` — the same load-versus-select split as packages and design systems, not a missing one-line primitive:

```ts
import { loadPackage } from "clay:packages";
import { setIconPack } from "clay:theme";

await loadPackage("@vendor/outline-icons");
setIconPack("@vendor/outline-icons");
```

For persistence across restarts, the validated selection is stored as the `iconPack` preference and reapplied on every reload. Explicit `loadPackage` registration alone never changes the active pack — only `setIconPack` (or the persisted preference) does. Loading both first-party packs in either order without an explicit selection changes nothing.

## JavaScript usage

```ts
import { setIconPack } from "clay:theme";

setIconPack("@clay/icons-phosphor-regular"); // clean monochrome outlines
// or
setIconPack("@clay/icons-phosphor-duotone"); // stronger two-layer silhouettes
```

`setIconPack` returns `{ pack, iconCount, schemaVersion }`.

## Example

```ts
import { setAppearance, setDesignSystem, setIconPack, setTheme } from "clay:theme";

setTheme("@clay/theme-gruvbox-material-dark");
setAppearance("dark");
setDesignSystem("@clay/core");
setIconPack("@clay/icons-phosphor-duotone");
```

## Options

Pass either a specifier string or `{ specifier }`. The specifier names one icon-pack contributor: a bundled first-party `@clay/icons-*` package, or an already-enabled package record that declares `clay.contributions.iconPack`.

## Return and async behavior

Synchronous. Returns `{ pack, iconCount, schemaVersion }` where `pack` is the resolved specifier, `iconCount` is the number of icons in the selected pack, and `schemaVersion` is the bounded geometry schema version.

## Errors

Throws `theme.invalid_request` for a missing, empty, or non-string specifier, `theme.load_failed` when a third-party pack is not enabled (or a first-party specifier cannot be resolved), and `theme.invalid_icon_pack` when the record's `iconPack` declaration fails bounded-geometry validation. A failed selection preserves the previously active pack untouched.

## Permissions and security

Packs are inert bounded data: no raw SVG/XML, CSS, URLs, scripts, event handlers, or unsupported attributes survive validation, and geometry renders through the host pipeline with `fill="currentColor"` so no package can introduce colors, brand palettes, or paint authority. Package callers cannot reach the op (trusted-runtime-only registration), so third-party runtime code cannot hijack the user-global icon selection. Adoption of a third-party pack requires the package service's existing enable/trust path; naming a package never promotes trust. Revoking or removing the selected package falls back to the bundled default subset with a sanitized diagnostic on the next reload — controls keep their accessible names and functionality either way.

## Agent guidance

Prefer the bundled default (no call) unless the user asks for a specific icon pack. Do not suggest raw SVG markup, package-owned colors, or packs from untrusted packages. After a selected package is removed, Clay falls back to the bundled subset and keeps every control functional.

## Backing implementation

`runtime/js/theme.js::setIconPack` calls `op_clay_theme_set_icon_pack`, which calls `apply_icon_pack` (`src/server/ops/theme.rs`) to resolve and validate the record and store the `ActiveIconPack`; `src/shell/icons.rs::ActiveIconPack::from_record` enforces core-key inventory gating (core semantic keys resolve only from compiled-inventory first-party records). The snapshot is validated again at the wire boundary (`src/protocol/runtime.rs`) and projected to the React client (`frontend/src/state/icon-store.ts`), where `ClayIcon` (`frontend/src/components/icon.tsx`) renders semantic references against the active pack with bundled fallback geometry.

Related public Rust surface (no further JS mapping needed): the `shell::icons` validation/parse helpers (`validate_icon_geometry`, `validate_core_icon_reference`, `parse_icon_path`) and `packages/record` icon-pack manifest parsing are consumed by the protocol, server, and Tauri bridge layers internally and are intentionally not package-facing; the only package-facing icon capability is the `clay.contributions.iconPack` declaration documented in [Icon packs](../../icon-packs.md).

## Lookup metadata

Tags: theme, icons, icon-pack, ui, init, fallback.

## Authority

Only bundled first-party `@clay/icons-*` packages and packages enabled through the package service are accepted. Packages contribute a static `iconPack` declaration (bounded vector geometry) validated server-side; icons reference semantic keys and never carry colors, so switching themes re-colors an activated pack automatically. Unknown or missing semantic keys resolve from the bundled fallback subset.

## Denied

Authority not granted: no filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, AI mutation, native widget, WASM, raw Deno ops, client-side JavaScript, raw CSS, raw SVG injection, or renderer callback authority; no automatic trust promotion by naming a package; no package-runtime access to the selection op.

## Key bindings

No default key bindings. This API is meant for startup configuration in `init.js`, not key routing.

## Custom properties

- name: specifier
- `specifier` (string, required): bundled `@clay/icons-*` package name, or the name of one enabled package contributing a `clay.contributions.iconPack`.

## Fallback and revocation

Explicit selection records intent; the bundled Regular subset covers every core semantic key so zero configuration renders complete UI. If the selected package fails to load on a later reload (removed, revoked, or invalid declaration), the last valid generation is preserved for the running session and a sanitized diagnostic is recorded; when no valid pack is active, the bundled fallback subset renders and every control keeps its accessible name and function. See [Icon packs](../../icon-packs.md) for the authoring contract.
