# Phase 18.13 Git Discovery Service Primitive Review

## Source

- `plans/041-Phase18.13-Git-Discovery-Service-and-First-Party-Clay-Git-Package.md`
- `roadmap.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/package-loading.md`
- `docs/reference/primitives/package-security.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/wiki/modules/workspace-file-browser.md`
- `docs/wiki/modules/transient-menu-session.md`
- `docs/wiki/modules/control-center.md`
- `docs/wiki/modules/command-registry.md`
- `docs/wiki/modules/package-loading.md`
- `docs/wiki/modules/slot-aware-package-ui.md`
- `src/server/workspace.rs`
- `src/server/command_execution.rs`
- `src/server/control_center.rs`
- `src/shell/transient_menu.rs`
- `src/shell/file_browser.rs`
- `runtime/js/workspace.js`
- `runtime/js/commands.js`
- `tests/primitives_docs.rs`

## Overview

Phase 18.13 should add Git discovery as a typed server-owned service and build first-party `@clay/git` UI on existing package/shell primitives. This review completes the primitive-first gate before implementation. It inventories the existing workspace, command, transient menu, Control Center, package loading, package UI, configuration, docs registry, and process-boundary surfaces; records what `@clay/git` can do without new UI or workspace primitives; identifies the small generic gaps needed for Git/status workflows; and rejects arbitrary shell, network, package filesystem, and client-side Git execution authority.

The key finding is boring: Git needs one new reusable server primitive, not a custom app subsystem. `WorkspaceState` already owns workspace roots. `CommandExecution` already owns server-first activation. `TransientMenuSession` already supports branch/action pickers. Control Center already lists executable server commands. Package UI primitives already support inert status items/panels/actions. `loadPackage("@clay/git")` already matches first-party package loading policy. The genuine gap is a generic, typed, read-only Git discovery service with a narrow `git` CLI command table, per-workspace cache/refresh state, bounded timeout/output, sanitized diagnostics, and Clay JS facade/docs coverage.

## Existing Primitive Inventory

### Workspace roots and file authority

- `src/server/workspace.rs::WorkspaceState` is the server source of truth for workspace roots, open documents, canonical paths, selected-file grants, and bounded directory listing.
- `WorkspaceRootDiscovery` and `BoundedFileListService` from Phase 18.12 already provide known workspace roots and server-owned root/listing authority. Git must consume those roots rather than rediscover workspaces independently.
- `runtime/js/workspace.js::serverListWorkspaceRoots`, `serverAddWorkspaceRoot`, `serverDiscoverWorkspaceRootForPath`, and `serverListDirectory` are documented Clay JS facades for workspace discovery/listing. `@clay/git` should read root identity/status through a Git API that is keyed by these workspace roots.
- The package boundary stays unchanged: packages cannot add roots, marker files, ignore rules, raw path scans, or arbitrary filesystem listing providers.

### Command execution and Control Center

- `src/server/command_execution.rs::CommandExecutor` is the shared server-owned execution boundary for SDUI actions, package UI actions, keybindings, and transient-menu selections.
- `CommandExecution` validates command ID, routing policy, package provenance, permissions, target context, argument payload, and session/action freshness before side effects.
- `src/server/control_center.rs::ControlCenter` already projects executable command metadata into `TransientMenuSession` items and routes activation through `CommandExecutor`.
- Git status/refresh/branch-palette commands can reuse built-in or package-registered server-first commands. They do not need a Git-specific dispatcher.

### Transient menu and picker UI

- `src/shell/transient_menu.rs::TransientMenuSession` stores bounded prompt/query/items/status/selection plus inert actions. It is explicitly intended for command palettes, completion pickers, file search, symbol search, Git pickers, and package quick-pick workflows.
- Query filtering and selection movement operate on installed bounded metadata. Activation emits a `TransientMenuAction` carrying only a command ID and bounded JSON arguments.
- Git branch/status/action pickers should build `TransientMenuSession` snapshots from cached Git status/branch metadata and activate typed commands through `CommandExecution`.

### Package loading and first-party package defaults

- `runtime/js/packages.js::loadPackage` is the one-line explicit package loading path from `~/.config/clay/init.js`. Package behavior should not silently auto-load just because a workspace is a Git repository.
- `PackageService`/package metadata validation already records package identity, source/provenance, `apiPrefix`, permissions/capabilities, entry/loadEntry confinement, and contribution conflicts.
- `@clay/git` should be a bundled first-party package with `apiPrefix = "git"` and read-only Git/workspace status capability, loaded explicitly through `await loadPackage("@clay/git")`.
- The package must consume documented Clay JS APIs. It must not call raw `Deno.core.ops`, spawn processes, read arbitrary files, or parse `.git` directly.

### Package UI, status items, panels, and action intents

- `clay:ui` contribution primitives support package panels, components, overlays, theme tokens, input metadata, UI state scopes, layout overrides, and package options as inert validated declarations.
- Component catalog/status surfaces can show branch name, dirty/clean state, changed-file count, refresh state, and diagnostics without adding a Git-specific native widget.
- `UiActionIntent` and SDUI actions carry registered command IDs plus bounded primitive arguments. Git refresh/open-status/open-picker actions should use the same command/action path.
- `src/shell/file_browser.rs` is the closest composition model: Clay-owned state feeds inert left-panel/fuzzy-open UI. `@clay/git` should similarly consume server-prepared Git status snapshots and declare package UI; it should not own native widgets or filesystem/process authority.

### Configuration and documentation registry

- `clay.configuration.setPackageOption` is available for package-owned user-visible settings when a real behavior-changing option is needed.
- Fixed internal safe defaults are preferable for Phase 18.13 unless refresh interval/timeout/status visibility genuinely needs user customization.
- Public programmatic Git behavior must go through Clay JS facades, Markdown docs, `docs/index.md`, generated registry artifacts, inventory entries, lookup tests, and Rust visibility/API-boundary tests.

### Existing process/timeout helpers

- The primitive inventory found no package-facing generic shell/process primitive suitable for Git. Existing package-manager process execution is install-time infrastructure and must not be reused as a runtime shell escape hatch.
- The Git implementation therefore needs a narrow internal process helper or service-local runner with a closed command enum, fixed argv, workspace-rooted cwd, timeout, output caps, and sanitized diagnostics.
- This is a Git discovery primitive, not a general `clay:shell` API.

## What `@clay/git` Can Achieve With Existing Primitives

With no new UI primitives, `@clay/git` can:

- Load explicitly from user config with `await loadPackage("@clay/git")`.
- Read typed cached Git status through planned `clay:git` facades such as `serverListGitStatuses` and `serverRefreshGitStatus`.
- Show a status item/panel using existing package UI component/status primitives.
- Register package-prefixed read-only commands and expose them through Control Center.
- Build transient branch/status/action pickers from `TransientMenuSession` metadata and activate selections through `CommandExecution`.
- Respect workspace-root authority by accepting workspace root IDs, not arbitrary paths.
- Use existing docs registry, API inventory, package guide, and wiki coverage rules.

No existing primitive lets a package spawn `git`, read `.git`, scan workspaces, mutate branches, contact remotes, or execute shell. That absence is correct.

## Generic Phase 18.13 Primitive Gaps

### `GitDiscoveryService`

Add a server-owned, read-only, typed Git discovery primitive keyed by `WorkspaceRootId`.

Required shape:

```rust
pub(crate) struct GitStatusSnapshot {
    pub(crate) workspace_root_id: WorkspaceRootId,
    pub(crate) repository_root: Option<PathBuf>,
    pub(crate) head: GitHeadState,
    pub(crate) dirty: bool,
    pub(crate) changed_file_count: usize,
    pub(crate) last_refresh: GitRefreshStatus,
}

pub(crate) enum GitDiscoveryCommand {
    RepositoryRoot,
    Head,
    StatusShort,
}
```

Implementation requirements:

- `cwd` is canonicalized and must remain inside a known workspace root before any process is spawned.
- Commands are a closed enum/table with fixed argv; no raw command strings, package-provided argv, shell strings, aliases, remotes, hooks configuration, or arbitrary subcommands.
- Use read-only CLI queries: repository root, current branch/detached HEAD, and short/porcelain status.
- Bound process runtime with a named timeout and bound stdout/stderr bytes before storing diagnostics.
- Sanitize diagnostics: no unbounded stderr/stdout, no credentials, no needless absolute paths.
- Return typed status for repo, non-repo, timeout, invalid output, and command error.
- Keep it internal/server-owned until exposed through documented `clay:git` APIs.

### `GitStatusCache`

Add a per-workspace cache/refresh layer on top of `GitDiscoveryService`.

Required behavior:

- Store latest snapshot per `WorkspaceRootId`.
- Return cached data for UI reads; explicit refresh starts/coalesces background Git work.
- Record `Idle`, `Refreshing`, `LastSuccess`, and `LastError`/stale state with timestamps where useful.
- Avoid global serialization: independent workspace roots refresh independently.
- Never block typing, rendering, completion, paint, layout, pointer, keypress, or text-event handlers.
- Keep refresh policy conservative and bounded. Public config only if a real user-facing setting is introduced.

### `clay:git` read-only facades

Expose the service through stable documented Clay JS APIs rather than raw ops.

Likely minimal API shape:

```ts
import { serverListGitStatuses, serverRefreshGitStatus } from "clay:git";

const statuses = await serverListGitStatuses();
await serverRefreshGitStatus({ workspaceRootId: statuses[0].workspaceRootId });
```

Docs/registry requirements:

- Stable IDs such as `clay.git.serverListGitStatuses` and `clay.git.serverRefreshGitStatus`.
- Backing Rust path, op wrapper path/name, JS facade path/export, user-facing name, key binding metadata, custom properties, permission/security notes, lookup tags, and examples.
- Tests that fail if docs/index/generated registry/facade/inventory entries drift.

## Hot-Path Classification

| Work | Classification | Allowed path |
| --- | --- | --- |
| Detect workspace roots | Existing Phase 18.12 startup/open-time server work | Consume `WorkspaceState` roots only |
| Run `git` CLI | Background/server work | Typed command enum, timeout, output cap, workspace-rooted cwd |
| Read Git status for UI | Cached server data | Return stale/refreshing snapshots without blocking UI |
| Explicit refresh | Server-first command/API | Coalesced per root; no paint/key/text path work |
| Status item/panel render | Paint/layout read of inert package UI data | Existing component/status primitives |
| Branch/action picker filtering | Local bounded transient menu metadata | `TransientMenuSession` query/selection state |
| Picker/action activation | Server-first `CommandExecution` | Revalidate command/args/permissions/session freshness |

Ordinary typing, caret movement, local edit application, scroll, paint, layout, pointer hit testing, keypress dispatch, and text-event handling must not synchronously run `git`, spawn processes, scan `.git`, read directories, call package JavaScript, wait on IPC, contact remotes, execute shell, or serialize full documents.

## Rejected Implementation Shapes

- Do not add a Git-specific native widget, Masonry branch/status widget, or `if package == "@clay/git"` rendering branch.
- Do not let `@clay/git` spawn `git`, read `.git`, list files, or call raw ops directly.
- Do not add a generic shell API or package-facing process execution primitive for this phase.
- Do not accept raw Git subcommands, raw argv, shell strings, aliases, environment scripts, remote names, or URLs from package/user input.
- Do not auto-load `@clay/git` when a repository is detected. Use explicit `loadPackage("@clay/git")`.
- Do not implement checkout, switch, stage, commit, reset, rebase, stash, push, pull, fetch, or arbitrary Git mutations in Phase 18.13.
- Do not add workspace root discovery, marker detection, or file listing logic to Git code; consume Phase 18.12 workspace roots.
- Do not use package-manager process code as a runtime shell escape hatch.
- Do not expose raw `Deno.core.ops.op_*` as the package/user API.

## Security and Authority Boundary

Phase 18.13 should introduce read-only Git status authority only.

Allowed:

- Server-owned, read-only `git` CLI calls through a closed command table.
- `cwd` rooted in a known `WorkspaceRootId` and canonicalized under that root.
- Bounded stdout/stderr, timeout, sanitized typed diagnostics.
- `@clay/git` package reading status only through documented `clay:git` facades.
- UI actions represented as inert command intents and revalidated through `CommandExecution`.

Not allowed:

- Arbitrary shell execution.
- Network/remotes/fetch/push/pull.
- Mutating Git operations.
- Package filesystem authority beyond existing workspace APIs.
- Package process authority.
- Client-side Git execution or client-side package JavaScript.
- Raw op, native widget, raw CSS, WASM, AI, package-manager, or credential authority.

Future mutating Git workflows require a separate plan/decision with explicit command authority, conflict/dirty-worktree handling, audit diagnostics, docs, and tests.

## Planned Documentation and Test Coverage

- `docs/wiki/modules/phase18.13-git-discovery-primitive-review.md` (this page) records inventory, generic gaps, hot-path classification, rejected shapes, and security boundary.
- `docs/reference/primitives/registry.md` should add/extend a `GitDiscoveryService` row if the implementation promotes it as a reusable primitive.
- `docs/reference/primitives/backlog.md` should record Phase 18.13's Git discovery/cache/API primitive gap.
- `docs/reference/packages/creating-packages.md` should document that Git status packages consume `clay:git`; packages do not get shell or `.git` authority.
- `packages/git/docs/index.md` should document the first-party package, one-line load path, and read-only limits.
- `tests/primitives_docs.rs` should require this review page to be linked from `docs/wiki/index.md` and `docs/wiki/modules/primitive-architecture.md`, and should assert inventory, gaps, hot-path split, rejected shapes, and no-shell/no-network/no-mutation boundary.

Implementation-time tests should cover repo/non-repo roots, branch and detached HEAD parsing, dirty status changed-file counts, timeouts, invalid output, sanitized diagnostics, cache/coalescing, workspace-root boundary rejection, no raw argv, no mutating command registry entries, package default load, and docs/registry freshness.

## Invariants and Constraints

- `WorkspaceState` remains the only source of workspace roots consumed by Git discovery.
- Git command execution is server-owned, read-only, bounded, typed, and cache-backed.
- `@clay/git` consumes documented APIs and inert UI/action primitives; it does not own process/filesystem authority.
- No Git work runs in Masonry paint/layout/pointer/scroll/keypress/text-event handlers.
- No mutating Git operation ships in Phase 18.13.
- Public Git APIs follow Clay JS facade/docs/registry rules; raw ops and raw Rust functions are not public API.

## Tests

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- Focused documentation coverage:

```text
cargo test --test protocol primitives_docs::
```

## Related

- [Workspace Discovery and File Browser](workspace-file-browser.md)
- [Transient Menu Session](transient-menu-session.md)
- [Control Center](control-center.md)
- [Command Registry](command-registry.md)
- [Package Loading](package-loading.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
