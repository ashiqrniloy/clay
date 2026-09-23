# Package Management (Phase 3: Install, Update, Distribution)

Status: implemented (Plan 115, Phase 3). Clay package management is CLI-only
via a durable install ledger, a delegated npm-compatible manager, and an
installation channel for Clay itself. There is no in-app package UI and no
self-hosted registry; installation is strictly separate from execution
(install ≠ enable ≠ adopt ≠ execute).

## Source

- `src/packages/manager.rs` — `PackageSpec` v1 parser, `NpmBackend`/`PnpmBackend`, `resolve_manager_backend()`
- `src/packages/ledger.rs` — durable install ledger (`installs.json`)
- `src/packages/init_lines.rs` — exact-shape `init.js` load-line management
- `src/packages/self_update.rs` — install-channel marker + self-update execution
- `src/packages/binaries.rs` — deny-by-default binary-provisioning inventory
- `src/packages/verbs.rs` — shared CLI verb logic and output contract
- `src/packages/service.rs` — `PackageService` (install/remove/enable/adopt/…)
- `src/cli.rs`, `src/launch.rs`, `src/main.rs` — CLI verbs
- `distribution/install.sh`, `distribution/npm/` — curl and npm channels
- `docs/development/distribution.md` — distribution contract
- `tests/package_cli.rs`, `tests/package_exit_gate.rs`, `tests/package_loading.rs`

## Overview

Three trust domains stay separate (decision log 2026-07-21-0001): the
bundled first-party inventory, third-party packages, and the package manager
process. Clay implements no registry: every install is delegated to an
npm-compatible manager (pnpm preferred, npm fallback, `CLAY_PACKAGE_MANAGER`
override, fail-closed when neither is on PATH). All verbs are CLI-initiated
and off the editing hot path — no new deno_core ops, no new JS facade APIs.

## Responsibilities

- Parse exactly four v1 spec forms: `npm:<name>`, `npm:@scope/name`,
  `npm:<name>@1.2.3`, `npm:@scope/name@1.2.3`. Anything else (git URLs,
  tarballs, local paths, ranges, bare names) is rejected at parse time with
  a typed `PackageSpecError` before any backend spawn.
- Install into a Clay-owned store via the manager, recording durable
  provenance in the ledger: `npm install --prefix <store> <spec> --ignore-scripts`.
- Maintain an exact-shape load line in `~/.clay/init.js`
  (`// clay install npm:<name> — remove with `clay remove npm:<name>`` +
  `await loadPackage("<name>");`), appended idempotently at install,
  stripped byte-exactly at remove, never touching hand-edited lines.
- Update: floating-spec installs re-install on `clay update --extensions`;
  pinned installs (exact-version spec) are skipped with the pinned-skip
  hint; self-update runs the recorded install-channel command.
- Provision inventory binaries (obscura, graft, qmd, ripgrep) deny-by-default:
  presence check by default, install only with `--bin <name> --yes`.
- Distribute Clay itself through `@arnilo/clay` (npm wrapper → optional
  platform packages) or the curl installer, both writing a channel marker
  beside the binary so `clay update` knows how this install was made.

Not this module: package execution, enable-time conflict checks, and the
adopt approval store (`third-party-runtime-authority.md`), the load path
(`package-loading.md`), and the init.js config runtime
(`configuration-runtime.md`).

## How It Works

1. `parse_command` (src/cli.rs) routes `clay install|remove|list|update` to
   top-level verbs; `clay package add|remove|list` now return errors pointing
   at the new commands. Specs and `--bin` names validate at parse time.
2. `service.install()` parses the spec again (single enforcement point),
   resolves the manager backend, records the requested spec in the ledger,
   then spawns one manager process. Discovery after install uses exact
   `package.json` name matching (scoped names included), fixing the former
   `split('@')` bug that broke `@scope/name` reinstalls.
3. `verbs::install()` then appends the load line via `init_lines.rs` —
   idempotent, CRLF-aware, atomic temp-file + rename. The appended line is
   picked up by the existing config watcher; it never enables, adopts, or
   executes the package. Re-install prints `Load line already present`.
4. `clay update` reads the ledger: `--extensions` re-installs every floating
   record (skipping pinned with `pinned; reinstall with a new version to
   move it`), `npm:<spec>` re-installs one managed package, and the default
   mode resolves the self-update channel (`self_update.rs`): no marker or a
   dev checkout prints the documented no-op; a valid `Npm`/`Curl` marker
   execs the recorded argv. `accept_update` (src-tauri/src/release.rs)
   remains the only payload-apply gate and is never called by the CLI.
5. Binary provisioning (`binaries.rs`) is a table of `{name, feature,
   install_spec | presence-only, install_command}`. `clay install` with no
   spec prints one row per binary (present/absent, resolved path, the exact
   command Clay would run). A real install requires `--bin <name> --yes`
   (the confirmation names what will run) and routes through the same
   manager backend with `--ignore-scripts` by default; refusals print the
   manual command instead. Provisioning never touches the package ledger,
   installs map, grants, or feature availability.
6. Distribution: `distribution/install.sh` verifies a sha256 sidecar before
   installing, writes `channel.json` beside the binary
   (`{version:1, channel:"curl", argv:[…]}`), and prints the adopt-boundary
   warning. The npm channel shims through `@arnilo/clay` → optional
   `@arnilo/clay-<platform>-<arch>` platform packages. `clay update` on a
   managed install re-runs the recorded channel argv — the channel is
   re-invoked, nothing else gains apply authority.

## Security Boundaries

- Install never enables/adopts/executes: an appended load line for an
  un-adopted third-party package fails closed with the adoption diagnostic.
- Lifecycle scripts default to `--ignore-scripts`; `--allow-scripts` prints
  an explicit ENABLED warning. Whether the manager then runs scripts is the
  manager's own policy (npm ≥ 11.17 gates registry-dep scripts behind its
  `allowScripts` field regardless of flags) — Clay's contract is the
  suppression boundary at the manager process.
- The ledger fails closed on corruption, unknown version, oversized file, or
  unsafe permissions; the channel marker likewise (missing, corrupt JSON,
  world-readable). Both are owner-only files.
- Binary provisioning is deny-by-default: unknown names and `--bin`+spec
  combinations fail at parse; presence-only entries are never spawned.
- Unsigned, wrong-target, and non-newer update payloads stay rejected by
  `accept_update`; the CLI has no update-apply path of its own.

## Performance

All verbs are CLI-only, spawn at most one manager process per invocation,
and add nothing to the server hot path. The appended load line costs a
normal config reload at boot (~33 ms vs ~60 ms baseline in manual tests);
update/install wall-clock is dominated by the manager process.

## Tests

- `tests/package_cli.rs` (security suite) — verb parsing/routing, output
  contract, ledger + init-line round trips through `FakeBackend`.
- `tests/package_exit_gate.rs` (security suite) — seven automated roadmap
  exit-gate drills driving the real `clay` binary + real npm backend against
  an in-test static registry (scratch `HOME`, zero network): install/remove
  round trip, update pinned-vs-floating, `--ignore-scripts` boundary via an
  argv-recording npm shim, append-once/never-enable, self-update dev no-op
  + marker command execution, and distribution dry-run contract.
- `tests/package_loading.rs` — spec rejection before backend call, scoped
  re-discovery, ledger round trips, fail-closed un-adopted load.

## Code Examples

```bash
clay install npm:@arnilo/st           # floating; appends load line; never enables
clay install npm:markdown-it@13.0.2   # pinned — skipped by update --extensions
clay install --allow-scripts npm:foo  # ENABLED warning printed
clay install                          # binary presence report (never spawns)
clay install --bin graft --yes        # provision one inventory binary
clay list --bundled                   # installed + first-party inventory
clay remove npm:@arnilo/st            # strips only the exact Clay block
clay update                           # self: no-op on dev checkout, else channel argv
clay update --extensions              # re-install floating; skip pinned
npm install -g @arnilo/clay           # npm channel (platform package + bin shim)
curl -fsSL https://clay.dev/dist/install.sh | sh  # curl channel (writes channel.json)
# clay update self re-runs the recorded channel argv; npm-channel installs have no
# marker, so they report the unmanaged no-op until the marker contract covers them
```
