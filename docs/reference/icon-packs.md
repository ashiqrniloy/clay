# Icon Packs

Public specification for Clay icon-pack packages: bounded, host-validated vector geometry that supplies UI glyphs for semantic references such as `action.close`, `document.save`, or `git.branch`. Icon packs are the icon counterpart of [UI design systems](ui-design-systems.md): packages ship inert geometry data, Clay owns rendering, and the active theme stays the sole color authority. (Plan 112.)

## Goals

- Let users switch UI icon styles through the same package mechanism as themes and design systems.
- Ship two first-party styles — `@clay/icons-phosphor-regular` (default, clean monochrome outlines) and `@clay/icons-phosphor-duotone` (stronger two-layer silhouettes) — covering an identical required semantic key set.
- Guarantee that a missing, invalid, or revoked pack never leaves invisible or broken controls: the bundled default subset is always available with zero configuration.

## Non-goals

- No arbitrary SVG injection: packs cannot ship raw SVG/XML, CSS, URLs, scripts, event handlers, filters, or unsupported attributes.
- No color authority: icons render with `fill="currentColor"` from the active theme's `text.icon` role; packs carry no palettes.
- No design-system coupling: icon selection is independent of theme, appearance, and design-system selections.

## Contribution contract

Icon packs are declared solely in the package manifest under `clay.contributions.iconPack` (manifest-single-source; no separate icon asset file, no init.js registration, no imperative registration):

```json
{
  "name": "@acme/outline-icons",
  "clay": {
    "contributions": {
      "iconPack": {
        "icons": {
          "vendor.custom-close": {
            "viewBox": [0, 0, 256, 256],
            "paths": [{ "d": "M 40 40 L 216 216 M 216 40 L 40 216" }]
          }
        }
      }
    }
  }
}
```

- **Keys**: either core semantic keys (first-party compiled-inventory packs only — core keys from any other source are rejected) or keys prefixed with the package's own namespace (e.g. `vendor.custom-close`). Duplicate and prototype-sensitive keys are rejected.
- **Geometry per icon**: `schemaVersion` (currently `1`), `viewBox` `[x, y, w, h]` with side lengths in `[1, 512]`, and 1–8 paths. Each path is 1–512 absolute `M|L|C|Q|H|V|A|Z` commands with finite coordinates in `[-4096, 4096]`, plus an optional `opacity` in `[0, 1]` (duotone shade layers use `0.2`). Relative, implicit-command, and SVG smooth (`S`/`T`) forms are normalized by the host parser at load; arcs are required because outlined glyphs round corners with `a` commands.
- **Budgets**: ≤ 2048 bytes per icon and ≤ 64 KiB per pack, at most 64 icons; every path's first command must be a moveto; empty or invisible required glyphs are rejected.
- **Provenance**: first-party `@clay/*` packs are pinned to upstream sources with recorded integrity (`packages/icon-sources.json`); third-party packs must be enabled through the ordinary package adoption path before selection can resolve them.

## Activation and lifecycle

| State | Behavior |
| --- | --- |
| Zero configuration | Bundled Regular subset is active; every core key resolves; no init.js lines or package load entries needed. |
| `setIconPack("@clay/icons-*")` | Resolves from the compiled inventory without executing package code; returns `{ pack, iconCount, schemaVersion }`. |
| `setIconPack("<third-party>")` | Requires the record to already be enabled (load ≠ select); never installs, adopts, or promotes trust. |
| Package revoked/removed | Last valid generation is preserved for the running session; on the next invalid reload a sanitized diagnostic is recorded and the bundled fallback subset renders. |
| Unknown semantic key | Stable empty decorative slot; the accessible name (label text) is the sole meaning carrier — controls are never blank. |

Selection persists as the `iconPack` preference and is re-validated at every commit. The selection op is registered in the trusted runtime extension only, so package runtime callers cannot hijack the user-global selection.

## Semantic keys (core set)

`action.close`, `action.new`, `document.save`, `document.reload`, `document.open`, `message.send`, `generation.stop`, `navigation.back`, `navigation.up`, `disclosure.right`, `disclosure.down`, `file.folder`, `file.file`, `file.symlink`, `session.resume`, `session.search`, `git.branch`, `status.success`, `status.warning`, `status.error`, `preview.toggle`.

First-party packs ship all 21 keys; third-party packs may ship any subset of their own namespace keys. Host UI requests semantic names, never upstream library names, so a pack swap never changes call sites.

## Rendering and accessibility

- The client (`ClayIcon`) renders bounded pack geometry as decorative inline SVG (`aria-hidden`) sized from `dimension.icon.size` (floor) with typography tracking; informative icons use `role="img"` with an accessible name.
- Icon-only controls require accessible names, hover/focus tooltips, visible keyboard focus, and full-sized hit targets; errors, permissions, model choices, and destructive confirmations keep text labels.
- Icons consume theme color roles only; they embed no colors, so theme switches re-color an activated pack automatically.

## Security

Validation happens host-side at record, shell, and wire boundaries: raw SVG/XML, CSS, URLs, scripts/events, `foreignObject`, `use`/`image`/`filter`/animation elements, unsupported attributes, non-finite values, malformed commands, and oversized payloads are all rejected fail-closed. See [`theme.setIconPack`](clay-js-api/theme/set-icon-pack.md) for the activation API and its authority notes.

## Authoring checklist

1. Emit bounded geometry JSON (schema above) for every key you claim — the host rejects the whole declaration if any icon fails validation.
2. Namespace your keys (`<prefix>.<name>`); do not ship core keys unless you are a first-party compiled-inventory pack.
3. Keep the manifest under the 64 KiB pack budget; duotone styles encode shade layers as `opacity: 0.2` paths.
4. Declare the contribution only in `package.json` under `clay.contributions.iconPack`; `dist/` load entries stay inert.
5. Test with the third-party fixtures under `tests/fixtures/icon-packs/third-party/` (valid-partial and hostile examples).
