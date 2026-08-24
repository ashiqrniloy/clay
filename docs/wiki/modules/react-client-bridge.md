# React Client Bridge

## What it is

`frontend/src/bridge/` plus the small app wiring that consumes it. Everything
Tauri-specific lives here; the rest of the frontend sees only the typed
bootstrap DTO, an envelope stream, and a request function.

| File | Responsibility |
| --- | --- |
| `bridge/types.ts` | `BootstrapDto` (session identity, workspace roots, resolved theme/typography, package UI, documents, diagnostics) and `BridgeEnvelope` (`event` / `routed` / `disconnected` / `themeSnapshot` / `runtimeSnapshot`). Mirrors the Rust `dto.rs` shapes with camelCase fields and string-encoded ids where the wire uses strings. |
| `bridge/client.ts` | The only module importing `@tauri-apps/api/core`. Exposes `bootstrapSession`, `reconnectSession`, `subscribeToEvents` (Tauri `Channel`), `unsubscribeFromEvents`, and `request(payload)` — raw JSON text of one protocol `ClientMessage`. |
| `bridge/errors.ts` | `normalizeBridgeError`: maps bridge/IPC failures to a bounded `{ code, message }`; never surfaces raw process or path details. |

## App wiring

- `app/connection.ts` derives a pure `ConnectionView`
  (loading/ready/error + retryable) from the supervisor's `ServerStatus`.
  Kept free of React so the state machine is unit-testable.
- `app/use-clay-session.ts` owns lifecycle: **subscribe before bootstrap**,
  then bootstrap once on mount; reconnect is explicit. Ordering matters —
  events emitted during bootstrap must not be dropped.
- `shell/workspace-controller.ts` is the envelope router: runtime snapshots,
  document events, transient menus, diagnostics, and routed client-command
  requests each have one handler and update pane/tab state.

## Single-flight bootstrap

React StrictMode double-mounts effects in dev. Two concurrent
`session_bootstrap` calls would return `busy` for the loser and clobber the
connection store with a false disconnect. `client.ts` therefore keeps one
in-flight promise: concurrent callers share it, and it clears in `finally`.
This is a frontend-side mirror of the bridge's idempotent-while-connected
bootstrap.

## Invariants

- No component calls `invoke` directly except through this module.
- Requests are JSON text of protocol messages; `Hello` is bridge-owned and
  rejected server-side if sent by the webview.
- Envelope handling is total: unknown envelopes are ignored, not thrown.
- No browser storage (`localStorage` et al) anywhere — configuration
  authority stays server-side (guard-tested).

## Tests

- `frontend/src/test/*`: store transitions, dispatcher routing, error
  normalization.
- `cargo test -p clay-desktop` covers the Rust side of the same contract
  (see [Desktop Typed Bridge](desktop-typed-bridge.md)).

## Related

- [Tauri Desktop Shell](tauri-desktop-shell.md) — process/command surface.
- [Frontend Edit Synchronization](../flows/frontend-edit-synchronization.md).
