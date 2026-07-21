# Package Distribution

Decision sources:

- `decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md`
- `decision-logs/2026-06-27-2014-unified-user-authorized-package-authority.md` (user-controlled package graph retained; same-runtime/source-ceiling rules superseded)
- `decision-logs/2026-07-21-0001-two-package-runtime-trust-domains.md`

- Clay should expose package management through both a `clay package ...` CLI and an in-app package UI, backed by one shared package-management service/path.
- Clay should not implement its own package manager or registry. Delegate package fetching, dependency resolution, version ranges, lockfiles, integrity, caching, and registry access to an existing npm-compatible JavaScript package manager, with pnpm/npm-compatible packages as the preferred direction.
- Clay owns the Clay-specific package contract: manifest validation, package identity/prefix, capabilities/permissions, runtime vs load-time entry separation, behavior manifest contributions, mode declarations, package graph relations, conflict metadata, enable/disable/remove state, user authorization state, and documentation/registry integration.
- Keep installation separate from execution:
  - Install downloads and records a package through the underlying package manager.
  - Enable/load validates Clay metadata, permissions, docs, and compatibility.
  - Runtime JavaScript executes server-side through Clay's JavaScript runtime.
  - Clients receive validated behavior manifests, SDUI updates, or protocol updates, not arbitrary package JavaScript.
- Clay uses two package-runtime trust domains. Integrity-verified bundled packages run with Clay in the trusted runtime; adopted packages run together in a separate shared third-party runtime and cannot be promoted through normal user approval. Third-party packages can receive documented public capabilities but never Clay-internal ops.
- Package APIs must declare and use a package name or registered package prefix for exported Clay JS APIs, e.g. `vimEnableMode`, so users and AI agents can identify provenance.
- Each JS package should be explicitly loaded from `~/.config/clay/init.js`; packages should not become behavior-changing defaults silently.
- The preferred end-user package setup is a one-line explicit load command, such as `loadPackage("@clay/markdown")` or the implemented equivalent. Package-specific customization may be available, but ordinary defaults should work without copied manifests or primitive-by-primitive boilerplate.
- If a package cannot support one-line default loading because Clay lacks a required generic primitive, plans should identify the primitive gap and document any longer setup as a temporary fallback/limitation, not the preferred convention.
- Package metadata should eventually include at least package prefix, runtime entry, load-time/behavior entry when present, permissions, modes, docs, and Clay JS API dependencies.
- npm registry, GitHub/git URL, tarball, and local path specs are valid package sources when routed through the shared package manager/source resolver. npm-compatible package management remains the default implementation boundary.
- Packages may request capabilities to depend on, import/use, extend, disable, or replace other packages. Cross-domain mutation requires a target-declared public extension point plus explicit user approval; a user-approved third-party replacement remains in the third-party runtime and retains its own provenance. Clay must show, record, enforce, and revoke every grant.
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
