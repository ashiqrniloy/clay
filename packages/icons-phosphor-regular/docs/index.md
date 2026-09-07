# @clay/icons-phosphor-regular

First-party **inert geometry-data** icon pack for Clay: the Phosphor Regular style from [Phosphor Icons](https://github.com/phosphor-icons/core) v2.0.8, shipped as bounded normalized geometry under `clay.contributions.iconPack` in [package.json](../package.json).

This package carries **no executable authority**: `dist/index.js` and `dist/load.js` are no-ops, there are no permissions, no ops, and no raw SVG/CSS at runtime. Clay parses and bounds-checks the geometry (per-path payload, command count, coordinate ranges) at package-record assembly.

Select it explicitly after loading:

```js
await clay.loadPackage("@clay/icons-phosphor-regular");
clay.setIconPack("@clay/icons-phosphor-regular");
```

The bundled host fallback is generated from the Regular pack; this pack only takes effect through explicit selection, which never changes theme, design-system, or typography state.

## Semantic keys (21)

| Key | Paths |
|-----|-------|
| `action.close` | 1 |
| `action.new` | 1 |
| `document.save` | 1 |
| `document.reload` | 1 |
| `document.open` | 1 |
| `message.send` | 1 |
| `generation.stop` | 1 |
| `navigation.back` | 1 |
| `navigation.up` | 1 |
| `disclosure.right` | 1 |
| `disclosure.down` | 1 |
| `file.folder` | 1 |
| `file.file` | 1 |
| `file.symlink` | 1 |
| `session.resume` | 1 |
| `session.search` | 1 |
| `git.branch` | 1 |
| `status.success` | 1 |
| `status.warning` | 1 |
| `status.error` | 1 |
| `preview.toggle` | 1 |

## Provenance and license

- Upstream: [phosphor-icons/core v2.0.8](https://github.com/phosphor-icons/core/tree/v2.0.8), assets converted by `scripts/generate-icon-packs.mjs` from the vendored, sha256-pinned sources under `packages/icon-sources/`.
- License: MIT — Copyright (c) 2023 Phosphor Icons. See [LICENSE](../LICENSE).
