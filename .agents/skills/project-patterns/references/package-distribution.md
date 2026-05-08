# Package Distribution

Decision source: `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`.

- Clay should expose package management through both a `clay package ...` CLI and an in-app package UI, backed by one shared package-management service/path.
- Clay should not implement its own package manager or registry. Delegate package fetching, dependency resolution, version ranges, lockfiles, integrity, caching, and registry access to an existing npm-compatible JavaScript package manager, with pnpm/npm-compatible packages as the preferred direction.
- Clay owns the Clay-specific package contract: manifest validation, package identity/prefix, permissions, runtime vs load-time entry separation, behavior manifest contributions, mode declarations, conflict metadata, enable/disable/remove state, and documentation/registry integration.
- Keep installation separate from execution:
  - Install downloads and records a package through the underlying package manager.
  - Enable/load validates Clay metadata, permissions, docs, and compatibility.
  - Runtime JavaScript executes server-side through Clay's JavaScript runtime.
  - Clients receive validated behavior manifests, SDUI updates, or protocol updates, not arbitrary package JavaScript.
- Package APIs must declare and use a package name or registered package prefix for exported Clay JS APIs, e.g. `vimEnableMode`, so users and AI agents can identify provenance.
- Package metadata should eventually include at least package prefix, runtime entry, load-time/behavior entry when present, permissions, modes, docs, and Clay JS API dependencies.
- Git, JSR/Deno, local path, tarball, or OCI-backed sources may be considered later, but npm-compatible package management is the default unless superseded by an approved decision.
- Plans that add package behavior must include documentation-as-code coverage for package APIs, commands, key bindings, configuration options, permissions, modes, and behavior manifest contributions.

## Example Direction

```bash
clay package add @clay/vim
clay package remove @clay/vim
clay package update
clay package list
```

```json
{
  "name": "@clay/vim",
  "version": "0.1.0",
  "type": "module",
  "exports": {
    ".": "./dist/index.js"
  },
  "clay": {
    "apiPrefix": "vim",
    "entry": "./dist/index.js",
    "loadEntry": "./dist/load.js",
    "permissions": [],
    "modes": ["vim"],
    "docs": "./docs/index.md"
  }
}
```
