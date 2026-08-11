# @clay/git

`@clay/git` is Clay's first-party read-only Git status package. It composes a sanitized status panel from the server-owned `clay:git` discovery facade. It declares no modes, no commands, no parse handlers, no decorations, and no permissions: all Git authority stays server-owned, and the package only publishes inert display state.

## Package Contract

- Package name: `@clay/git`
- API prefix: `git`
- Major modes: none (Git is not a document mode)
- Runtime entry: `./dist/index.js`
- Load entry: `./dist/load.js` (exports `loadGitPackage(clay, options)` and default `loadGit`; re-exported from `./dist/index.js`)
- Status adapter: `./dist/status.js`
- Documentation entry: `./docs/index.md`
- SDUI region: `git.status` (read-only branch/dirty/refresh labels)
- Configuration: none. Phase 18.13 uses fixed safe defaults (bounded discovery timeout, stale-poll interval). No `clay.contributions.configuration` entries and no `package-configuration` permission.

## Read-Only Scope

This package is read-only. It does not stage, commit, checkout, reset, rebase, stash, push, pull, or fetch. It does not request filesystem, network, shell, AI, WASM, raw Deno op, native widget, client runtime, package install/enable, or workspace mutation authority. Mutating Git operations are deferred to a later phase with their own command authority UX; see `roadmap.md` Phase 18.13.

The package consumes the server-owned `clay:git` facade (`serverListGitStatuses`). Git is executed only by the server's `GitDiscoveryService` behind a closed read-only command table, per-root cwd confinement, bounded timeouts, and capped output. The package never spawns Git, accepts Git argv, or passes shell strings.

## Future Mutation Authority

Phase 18.13 ships read-only Git discovery only. The following mutating operations are **not implemented** and have no executable path, no command ID, and no API in this phase:

```text
checkout | switch | add | stage | commit | reset | rebase | stash | push | pull | fetch | merge | cherry-pick | revert | tag | clone | mv | rm | restore
```

Bringing any of these to Clay will require its own authority UX, not a flag on this package:

- **Explicit server-owned command IDs** (e.g. `git.stageFile`) added to a closed command table, each validated against the workspace root — never arbitrary argv, never a generic shell escape hatch. The current `GitDiscoveryCommand` enum is the model: a variant per operation, fixed argv, no caller-supplied subcommand.
- **A new permission grant** surfaced through explicit user approval (the package's current `permissions: []` is the read-only ceiling; `@clay/git` must not silently gain mutation authority).
- **Conflict/state handling**: staging areas, index/worktree divergence, merge conflicts, detached-HEAD safety, and upstream/remote concerns (push/pull/fetch) need dedicated flows, including network authority for remote operations — which is explicitly out of scope here.
- **No speculative abstractions**: do not add half-wired mutation plumbing in advance. Each operation lands with its command, validation, tests, and docs together.

Until that work exists, `@clay/git` reads cached status and renders a sanitized panel. UI surfaces for future mutation (if any) must be clearly labeled and non-executable.

## Control Center Commands

Branch/status commands are server-owned built-ins, available regardless of package load:

- `git.listStatuses` — List Git Statuses
- `git.refreshStatus` — Refresh Git Status

Loading `@clay/git` adds the read-only status panel on top of these always-available commands. Without the package, the commands and `clay:git` facade remain usable; only the package-owned status UI is absent.

## Default Load Path

```js
// ~/.config/clay/init.js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/git");
```

The package is not auto-loaded. Without this line, no Git status panel is published. The package is not a more capable package because it is `@clay/*`; it declares no permissions and receives no authority beyond reading the cached `clay:git` data the server already owns.

## Status Data Path

On load, the load entry calls `serverListGitStatuses()` from `clay:git` and publishes an inert SDUI tree of `panel`/`stack`/`label` nodes. Each known workspace root contributes four sanitized labels:

- root label (basename only — absolute paths are never emitted)
- head state (branch name, detached short SHA, unborn, or unknown)
- dirty state (clean, `N changed`, or dirty)
- refresh state (idle, refreshing, current, or the typed last-error kind)

Control characters and path-like substrings are stripped/collapsed. The tree has no action targets and no callbacks: it is display-only. Refresh happens through the server-owned `git.refreshStatus` command; the package re-publishes cached state at load/update time and runs no JavaScript in paint, layout, pointer, scroll, keypress, or text-event hot paths.

## Validation

Manifest metadata is validated at package load time through the shared package metadata gate. The SDUI region contribution is inert conflict/diagnostic metadata. Published trees pass the shared `RUNTIME_SDUI_TREE_PAYLOAD_BUDGET_BYTES` validation and action-target checks. See `docs/reference/packages/creating-packages.md` for the authoring contract.
