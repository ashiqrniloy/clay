# First-Party `@clay/git` Package

## Source

- `packages/git/package.json`
- `packages/git/dist/index.js`
- `packages/git/dist/load.js`
- `packages/git/dist/status.js`
- `packages/git/docs/index.md`
- `runtime/js/git.js`
- `src/server/ops/git.rs`
- `src/server/ops/sdui.rs`
- `src/server/command_execution.rs`
- `tests`: `src/server/js_runtime.rs` (`git_package_loads_and_publishes_read_only_status`, `git_package_declares_no_mutation_or_network_authority`)
- Plan: `plans/041-Phase18.13-Git-Discovery-Service-and-First-Party-Clay-Git-Package.md`
- Primitive review: `docs/wiki/modules/phase18.13-git-discovery-primitive-review.md`

## Overview

`@clay/git` is Clay's first-party read-only Git status package. It composes a
sanitized status panel from the server-owned `clay:git` discovery facade and
publishes it as an inert SDUI tree. The package itself declares **no
permissions** and receives no shell, network, filesystem, or mutating Git
authority — all Git execution stays inside the server's `GitDiscoveryService`.

## Responsibilities

- Provide the one-line default load path: `loadPackage("@clay/git")`.
- Build and publish a read-only SDUI status panel for each known workspace
  root.
- Sanitize all displayed text: strip control characters, collapse path-like
  substrings, and never emit absolute repository or workspace paths.
- Add no modes, no commands, no parse handlers, no decorations, and no
  configuration surface.
- Remain additive: unloading `@clay/git` leaves the server-owned
  `clay.git.listStatuses` and `clay.git.refreshStatus` commands available.

## How It Works

1. The package manifest (`packages/git/package.json`) declares:
   - `apiPrefix: "git"`
   - `permissions: []`
   - `apiDependencies: ["clay.git.serverListGitStatuses", "clay.sdui.publishTree"]`
   - a single SDUI contribution on region `git.status`
2. `loadPackage("@clay/git")` resolves the first-party specifier to
   `packages/git/dist/load.js` and invokes its default export.
3. `load.js` (`loadGit` / `loadGitPackage`) calls `serverLoadPackage` to
   register the manifest, imports SDUI helpers from `clay:sdui`, and then
   calls `publishGitStatus` to build and publish the status tree.
4. `status.js` fetches cached statuses with `serverListGitStatuses()` from
   `clay:git`, converts each status into sanitized labels for root basename,
   head state, dirty state, and refresh state, and builds a tree of only
   `panel`/`stack`/`label` SDUI nodes.
5. The published tree has **no action targets** and **no callbacks**; it is
   pure display state rendered by the native Masonry/SDUI reconciler.
6. The server-side ops (`src/server/ops/git.rs`) read the shared
   `GitStatusCache` (`list_cached` / `refresh_root`) after validating the
   workspace root ID. The built-in commands (`clay.git.listStatuses`,
   `clay.git.refreshStatus`) in `src/server/command_execution.rs` expose the
   same data through the generic command-execution path.

## Code Examples

Default user activation in `~/.config/clay/init.js`:

```js
import { loadPackage } from "clay:packages";

await loadPackage("@clay/git");
```

The status adapter produces labels such as:

```js
{
  id: "git.head.1",
  text: "Branch: main"
}
{
  id: "git.dirty.1",
  text: "Status: 3 changed"
}
```

## Invariants and Constraints

- `permissions: []` is the read-only ceiling. Any future mutation support must
  come through a new explicit user-approved permission grant, not by widening
  this package's manifest.
- No Git command strings, argv, repository paths, branch names, remotes, or
  shell fragments are accepted by package code.
- Absolute workspace/repository paths are never emitted to the UI; only the
  workspace root basename is shown.
- The package runs no JavaScript in paint, layout, pointer, scroll, keypress,
  or text-event hot paths; it reads cached state at load/update time.

## Future Mutation Authority

Phase 18.13 ships read-only discovery only. Mutating operations (checkout,
switch, add, stage, commit, reset, rebase, stash, push, pull, fetch, merge,
cherry-pick, revert, tag, clone, mv, rm, restore) are not implemented and have
no executable path in this package. Future work must add explicit
server-validated command IDs and a separate permission grant; `@clay/git` must
not silently become a mutation package.

## Tests

Run:

```bash
cargo test --test protocol --quiet
cargo test --lib server::js_runtime git_package --quiet
```

Key coverage:

- `git_package_loads_and_publishes_read_only_status` — verifies
  `loadPackage("@clay/git")` publishes a status panel and exposes branch/dirty
  data with zero package permissions.
- `git_package_declares_no_mutation_or_network_authority` — asserts
  `permissions: []`, no package commands, no configuration/package options, and
  no leaked mutating or network authorities.

## Related

- [Git Discovery Service](git-discovery-service.md)
- [Phase 18.13 Git Discovery Service Primitive Review](phase18.13-git-discovery-primitive-review.md)
- [serverListGitStatuses API](../../reference/clay-js-api/git/server-list-git-statuses.md)
- [serverRefreshGitStatus API](../../reference/clay-js-api/git/server-refresh-git-status.md)
