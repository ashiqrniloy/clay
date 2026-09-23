# Distribution (v1)

Clay self-distributes through one npm package and one curl installer,
following the pi model (decision `2026-08-30-2153-st-package-name-arnilo-scope-and-npm-distribution.md`).
This phase ships the skeleton and documentation only — **nothing is
published or hosted yet**; the publish and hosting steps below are the
post-phase pipeline.

## Warning

Clay packages run with full system access — review before installing.
`clay install` never executes package code; `clay package adopt` is the
reviewable gate. Distribution artifacts themselves contain no lifecycle
scripts.

## Components

| Component              | Location                | Purpose                                                       |
| ---------------------- | ----------------------- | ------------------------------------------------------------- |
| npm wrapper package    | `distribution/npm/`     | `@arnilo/clay`; bin shim execs a native platform binary       |
| curl installer         | `distribution/install.sh` | verifies digest, installs `clay`, writes the channel marker |
| Distribution docs      | this file               | packaging, dry-run verification, marker contract, rollback    |

## npm wrapper package (`@arnilo/clay`)

- `distribution/npm/package.json` declares the `clay` bin shim
  (`bin/clay.js`) and **no lifecycle scripts** (`"scripts": {}`); the shim
  only execs, adding no cost beyond process spawn.
- Native binaries ride in optional platform packages with the documented
  layout `@arnilo/clay-<platform>-<arch>` containing `bin/clay`
  (`@arnilo/clay-linux-x64`, `@arnilo/clay-linux-arm64`). The skeleton
  creates no platform packages; add them when the artifact pipeline exists.
- The wrapper version must match the crate version; `scripts/package-smoke.sh`
  enforces this alongside the other release artifacts.

## Packaging steps

```sh
cargo build -p clay -p clay-desktop --release   # native binaries
cargo tauri build --bundles deb                 # optional desktop bundle (opt-in via CLAY_TAURI_BUNDLE=1)
# sha256sum target/release/clay > clay-<version>-linux-x64.sha256 (content = digest + filename)
```

Platform packages would repeat the same layout per target
(`@arnilo/clay-linux-x64/bin/clay` + `bin/clay.js` shim in the wrapper).

## Publish dry-run verification

From the wrapper package directory (no network, nothing is published):

```sh
cd distribution/npm
npm publish --dry-run
```

Confirms the packed file list (`bin/`, `README.md`), the absence of any
`postinstall`/`preinstall` hook, and version alignment. Host the installer
and digests under `https://clay.dev/dist/` before real curl installs.

## Install / update commands

```sh
# curl channel
curl -fsSL https://clay.dev/install.sh | sh -s -- --version 0.1.0

# npm channel (no lifecycle scripts run)
npm install -g @arnilo/clay
```

`clay update` (self) reads the channel marker and re-runs the recorded
command; `--force` re-runs even when up to date. `clay update` never
applies desktop update payloads — `src-tauri/src/release.rs::accept_update`
remains the only apply gate, and unsigned payloads stay rejected.

## Channel marker contract

The installer writes `channel.json` next to the installed `clay` binary,
owner-only (`0600`):

```json
{ "version": 1, "channel": "curl", "argv": ["sh", "...", "--version", "0.1.0", "..."] }
```

`src/packages/self_update.rs` consumes it fail-closed: missing, corrupt,
unknown-version, empty-argv, world-readable, or oversize markers all
resolve to "unmanaged" and `clay update` self reports a skip instead of
running anything. There is no third updater and no third update path.

## Binary provisioning

`clay install --bin <name> --yes` provisions feature binaries (graft, …)
through the shared package manager into the Clay-owned package store —
see `src/packages/binaries.rs` and decision
`2026-09-08-2141-binary-provisioning-deny-by-default-table.md`. Distribution
artifacts do not pre-provision any feature binary.

## Rollback

Reinstall the previous version through the same channel — no rollback
machinery exists:

```sh
curl -fsSL https://clay.dev/install.sh | sh -s -- --version <previous>
npm install -g @arnilo/clay@<previous>
```

## Local verification

`scripts/package-smoke.sh` exercises the distribution additions: wrapper
version alignment, `npm pack --dry-run` when node/npm are present, and a
fixture install that proves digest verification, marker schema, and
tamper rejection end to end.
