# Tauri Desktop Shell

## What it is

`src-tauri/src/` is the desktop process shell: windowing, the Clay server
*process* lifecycle, and the narrow Tauri command surface the webview may
call. It owns no document, package, configuration, or agent authority — all
of that stays in the Clay server (`clay` crate). The bridge session that
talks to a running server lives next door in
[`src-tauri/src/bridge/`](desktop-typed-bridge.md); this page covers the rest.

## Modules

| File | Responsibility |
| --- | --- |
| `main.rs` | Entry point; calls `clay_desktop::run()`. |
| `lib.rs` | `run()` resolves the endpoint fail-closed, builds managed state (`Supervisor`, `BridgeState`, `DialogState`), registers the invoke handler, and maps `RunEvent::ExitRequested` to supervisor shutdown so no child server outlives the window. |
| `server.rs` | `Supervisor`: adopt-or-spawn lifecycle for the `clay-server` sidecar. Typed `ServerStatus { Connecting, Connected { pid }, Disconnected { reason } }`. |
| `commands.rs` | The complete Tauri command surface (`server_status`, `session_*`, `agent_*`, `tab_*`, dialog commands). Every command delegates to managed state; none contains protocol or policy logic. |
| `release.rs` | Release identity and endpoint resolution: `desktop_endpoint()` derives the IPC endpoint from release identity, locates the sidecar binary by target triple (`CLAY_HOST_TRIPLE`, set in `build.rs`), and fails closed on mismatch. |

## Supervisor semantics

- **Adopt first.** Before spawning, the supervisor probes the endpoint
  (`endpoint_accepts`). An already-running server is adopted: status becomes
  `Connected` with `pid: null` and no child process exists.
- **Spawn with probe thread.** On spawn it starts `clay-server <endpoint>`,
  then a generation-tagged probe thread polls until the endpoint accepts.
  Stale probes from an old generation exit without touching state.
- **Fail closed.** A missing binary or failed endpoint resolution produces a
  typed `Disconnected { reason }` — never a fallback spawn of something else,
  and never a panic at startup (`mark_disconnected` path still opens the
  window so the UI can render the rejection).
- **Restart** replaces the child under the same endpoint; shutdown drops the
  child so no orphan survives the desktop shell.

## Command surface

The webview can only call the registered commands. There is deliberately no
filesystem, shell, process, or network plugin compiled in; the `main`
capability grants only `core:default` permissions, and the CSP is
`default-src 'none'` with no remote origins. Dialog commands
(`dialog_open_file`, `dialog_open_folder`, `tab_open_dialog`) open OS dialogs
under per-kind mutexes (one dialog at a time) and return the selected path to
the existing server grant paths — selection grants authority only after the
Clay server accepts it, exactly as with any other grant flow.

## Why shaped this way

Server authority plus a dumb shell keeps one trust boundary: the webview is a
renderer for typed projections, packages never touch Tauri APIs, and a
compromised webview cannot reach files or processes beyond what a user
explicitly granted through dialogs. Adoption makes `clay server` +
`clay client` against one daemon work naturally.

## Tests

- `src-tauri/src/server.rs` unit tests: missing-binary reporting,
  `mark_disconnected`, status serialization, spawn/connect/shutdown
  orphan-freedom, restart.
- `src-tauri/tests/config_security.rs`: release identity, icon presence,
  updater-artifact restrictions, capability/CSP posture;
  `tests/bridge_session.rs` and `tests/dto_roundtrips.rs` cover the session
  and DTO contracts.
- Run: `cargo test -p clay-desktop`.

## Related

- [Desktop Typed Bridge](desktop-typed-bridge.md) — session/request/event path.
- [Desktop Release Hardening](desktop-release-hardening.md) — packaging/updaters.
- [React Client Bridge](react-client-bridge.md) — frontend counterpart.
