# @clay/icons-phosphor-duotone

First-party **inert geometry-data** icon pack for Clay: the Phosphor Duotone style from [Phosphor Icons](https://github.com/phosphor-icons/core) v2.0.8, shipped as bounded normalized geometry under `clay.contributions.iconPack` in [package.json](../package.json).

This package carries **no executable authority**: `dist/index.js` and `dist/load.js` are no-ops, there are no permissions, no ops, and no raw SVG/CSS at runtime. Clay parses and bounds-checks the geometry (per-path payload, command count, coordinate ranges) at package-record assembly.

Select it explicitly after loading:

```js
await clay.loadPackage("@clay/icons-phosphor-duotone");
clay.setIconPack("@clay/icons-phosphor-duotone");
```

The bundled host fallback is generated from the Regular pack; this pack only takes effect through explicit selection, which never changes theme, design-system, or typography state.

## Semantic keys (22)

| Key | Paths |
|-----|-------|
| `action.close` | 2 |
| `action.new` | 2 |
| `control-center.open` | 2 |
| `document.save` | 2 |
| `document.reload` | 2 |
| `document.open` | 2 |
| `message.send` | 2 |
| `generation.stop` | 2 |
| `navigation.back` | 2 |
| `navigation.up` | 2 |
| `disclosure.right` | 2 |
| `disclosure.down` | 2 |
| `file.folder` | 2 |
| `file.file` | 2 |
| `file.symlink` | 2 |
| `session.resume` | 2 |
| `session.search` | 2 |
| `git.branch` | 2 |
| `status.success` | 2 |
| `status.warning` | 2 |
| `status.error` | 2 |
| `preview.toggle` | 2 |

## Provenance and license

- Upstream: [phosphor-icons/core v2.0.8](https://github.com/phosphor-icons/core/tree/v2.0.8), assets converted by `scripts/generate-icon-packs.mjs` from the vendored, sha256-pinned sources under `packages/icon-sources/`.
- License: MIT — Copyright (c) 2023 Phosphor Icons. See [LICENSE](../LICENSE).
