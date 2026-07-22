# Git Discovery Service

## Source

- `src/server/git.rs`
- `src/server/workspace.rs`
- `src/server/mod.rs`
- `src/server/ops/git.rs`
- `runtime/js/git.js`
- `tests`: `cargo test server::git`; `cargo test git_facade_lists_refreshes_and_commands_statuses`
- Plan: `plans/041-Phase18.13-Git-Discovery-Service-and-First-Party-Clay-Git-Package.md`
- Primitive review: `docs/wiki/modules/phase18.13-git-discovery-primitive-review.md`

## Overview

`GitDiscoveryService` is Clay's server-owned read-only Git status primitive. It reports Git repository root, current branch or detached HEAD, dirty state, changed-file count, and typed refresh status for known workspace roots.

`GitStatusCache` sits on top of discovery. It stores one cached status per workspace root, supports explicit refresh, coalesces concurrent refreshes for the same root, polls stale entries only after `GIT_STATUS_POLL_INTERVAL`, and keeps the previous good snapshot when a later refresh fails.

Packages do not run Git. They consume `clay:git` APIs backed by this service/cache: `serverListGitStatuses()` for cached reads and `serverRefreshGitStatus({ workspaceRootId })` for explicit refresh. The first-party `@clay/git` package (`packages/git/`) is the reference consumer: it declares no permissions and publishes a sanitized read-only status panel from cached `clay:git` data.

## Flow

1. `WorkspaceState::directory_roots()` returns canonical directory workspace roots.
2. `GitDiscoveryService::discover_workspace_statuses()` starts one Tokio task per root; every discovery acquires a shared `GIT_ROOT_CONCURRENCY = 4` semaphore permit for its complete command sequence. Roots therefore run concurrently without creating unbounded Git subprocesses.
3. Each permitted root is canonicalized and checked as a directory before spawning anything. Its commands remain strictly sequential (`repository root` → branch/detached head → status), while completed root snapshots are sorted back by workspace-root ID so results retain authority association.
4. The service runs a closed command table only:
   - `git --no-optional-locks rev-parse --show-toplevel`
   - `git --no-optional-locks symbolic-ref --quiet --short HEAD`
   - `git --no-optional-locks rev-parse --short HEAD`
   - `git --no-optional-locks status --porcelain=v1 --untracked-files=normal`
5. Output is captured through capped async readers, with timeout and sanitized diagnostics.
6. Results become `GitStatusSnapshot` with `GitRefreshStatus` values for success, non-repo, timeout, command error, invalid output, or boundary rejection.
7. `GitStatusCache::list_cached()` returns current cached data without spawning Git.
8. `GitStatusCache::refresh_root()` marks one root `Refreshing`, runs discovery without holding the cache mutex, then records `LastSuccess` or `LastError` with timestamps.
9. Concurrent refreshes for the same root wait on a per-entry `Notify`; refreshes for different roots run independently.
10. `GitStatusCache::refresh_stale_workspace()` refreshes only roots whose last success/error is older than `GIT_STATUS_POLL_INTERVAL`.
11. `src/server/ops/git.rs` serializes cache snapshots into the public `clay:git` facade shape.
12. Built-in command IDs `clay.git.listStatuses` and `clay.git.refreshStatus` expose the same read-only data through server-first command execution.

## Boundaries

- No shell strings.
- No package-provided argv.
- No Git mutations.
- No remotes/network commands.
- No client-side Git execution.
- The discovery cwd must canonicalize under a known workspace root.
- Repository root outside workspace root is rejected.
- stdout/stderr are byte-capped; diagnostics are character-capped and control-character stripped.
- Public APIs do not accept shell commands, argv, repository paths, remotes, branch names, or mutation options.

## Tests

Run:

```bash
cargo test server::git --quiet
cargo test git_facade_lists_refreshes_and_commands_statuses --quiet
```

Coverage includes repo/non-repo roots, branch and detached HEAD, dirty path counting, timeout, root-boundary rejection, known workspace-root iteration, status parser path de-duplication, cached reads, explicit refresh, coalesced same-root refreshes, independent multi-root refreshes, stale polling, last-error diagnostics preserving the previous snapshot, `clay:git` facade import/use, and server-first Git command execution. Plan 060 T8's `workspace_discovery_bounds_root_concurrency_and_preserves_association` holds three fake roots at a rendezvous with a concurrency budget of two, proves only two start, then verifies each root's command order and returned ID/path association.

Cache timing tests are deterministic under parallel load: the coalescing test
uses a fake-git rendezvous (the leader's first command blocks on a release
sentinel), the stale-polling test pins `finished_at` explicitly for the
fresh/stale cases, and `fake_git()` waits briefly after marking the executable
to avoid Linux `ETXTBSY` races.

## Related

- [serverListGitStatuses API](../../reference/clay-js-api/git/server-list-git-statuses.md)
- [serverRefreshGitStatus API](../../reference/clay-js-api/git/server-refresh-git-status.md)
- [Phase 18.13 Git Discovery Service Primitive Review](phase18.13-git-discovery-primitive-review.md)
- [Workspace Discovery and File Browser](workspace-file-browser.md)
