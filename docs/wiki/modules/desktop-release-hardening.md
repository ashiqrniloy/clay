# Desktop Release Hardening (Plan 097 Phase 11)

Status: implemented (Phase 11). The Tauri desktop shell is the only client
bridge for local, adopted, container-shared, and multi-client sessions.
Packaging is Linux-first. There is no in-app updater.

## Source

- `src-tauri/src/release.rs`
- `src-tauri/src/server.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/tauri.conf.json`
- `src-tauri/capabilities/main.json`
- `src-tauri/tests/config_security.rs`
- `scripts/package-smoke.sh`
- `scripts/security-audit.sh`

## Overview

Phase 11 pins the operational and security surface that Phases 2–10 already
use: one supervisor, one typed bridge, deny-by-default webview, matching
artifact versions, and fail-closed updates.

## Responsibilities

- Resolve `CLAY_ENDPOINT` to a **local** Unix socket or Windows named pipe.
  Network URLs (`://`, `tcp:`) are rejected before spawn.
- Resolve `clay-server` as `CLAY_SERVER_BIN` → sibling → Tauri sidecar
  (`clay-server-<target-triple>`) → `PATH`. Missing binaries become a typed
  `Disconnected` status without leaking absolute paths.
- Adopt an already-running server (container / `clay server`) instead of
  double-spawning. Shutdown kills only children this process spawned.
- Keep webview capabilities at `core:default` and CSP at `default-src 'none'`.
- Refuse unsigned, wrong-target, and non-newer update manifests. Do not
  compile `tauri-plugin-updater` until signing keys exist outside the repo.

Not this module: document authority, package trust domains, AG-UI mapping.

## How It Works

1. `run()` calls `desktop_endpoint()`. On rejection it opens the window with
   `Supervisor::mark_disconnected` so the status line can explain the failure.
2. `Supervisor::start` adopts a live endpoint or spawns the resolved sidecar.
   Spawn `NotFound` is classified by `classify_spawn_error`.
3. `accept_update(current, host, manifest)` is the only update policy:
   empty signature → `Unsigned`; target ≠ host triple → `WrongTarget`;
   version ≤ current → `WrongVersion`. No apply path exists today.
4. `scripts/package-smoke.sh` checks version identity across the crate,
   desktop member, `tauri.conf.json`, frontend, and `clay-agent`, then runs
   the release/missing-binary tests and the frontend gzip budget when `dist/`
   exists. `CLAY_TAURI_BUNDLE=1` optionally runs `cargo tauri build`.
5. `scripts/security-audit.sh` runs `cargo audit` plus the capability/CSP
   suite. Frontend `npm audit` is advisory.

## Code Examples

```bash
CLAY_ENDPOINT=/tmp/clay-container.sock target/debug/clay-desktop
CLAY_SERVER_BIN=target/debug/clay-server target/debug/clay-desktop
scripts/security-audit.sh
scripts/package-smoke.sh
```

```rust
assert_eq!(
    accept_update("0.1.0", "x86_64-unknown-linux-gnu", &unsigned),
    Err(UpdateReject::Unsigned)
);
```

## Invariants

- Linux is the blocking host. Windows named-pipe code remains, but Windows
  packaging is not a required CI gate.
- Install/uninstall of `.deb`/`.rpm` is the host package manager. The app
  does not self-update.
- Multi-client isolation stays in the existing tab-scoped bridge sessions.

## Tests

- `src-tauri/src/release.rs` unit tests (network reject, missing-binary
  wording, version identity, unsigned/wrong-target/wrong-version).
- `src-tauri/src/server.rs` supervisor tests (typed missing binary,
  `mark_disconnected`, adopt, reap).
- `src-tauri/tests/config_security.rs` (CSP, `core:default`, no updater
  plugin, icon + Linux bundle targets, matching versions).

## Related

- [Desktop Typed Bridge](desktop-typed-bridge.md)
- `docs/development/security.md`
- `docs/development/build-and-test.md`
- `docs/development/performance.md`
- `docs/development/windows.md`
