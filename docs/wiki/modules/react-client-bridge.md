# React Client Bridge

## What it is

`frontend/src/bridge/` plus the small app wiring that consumes it. Everything
Tauri-specific lives here; the rest of the frontend sees only the typed
bootstrap DTO, an envelope stream, and a request function.

| File               | Responsibility                                                                                                                                                                                                                                                                                                                 |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `bridge/types.ts`  | Hand-written part of the webview contract: branded ids (`MenuSessionId`, `DocumentId`), the `ShellEvent` narrowing of client events, and the composed `BridgeEnvelope` (`event` / `routed` plus the generated members). Generated shapes live in `bridge/generated/` (see [Desktop Typed Bridge](desktop-typed-bridge.md#generated-webview-contract-plan-119-sc-1)) — this file never restates them. |
| `bridge/client.ts` | The only module importing `@tauri-apps/api/core`. Exposes `bootstrapSession`, `reconnectSession`, `subscribeToEvents` (Tauri `Channel`), `unsubscribeFromEvents`, and `request(payload)` — raw JSON text of one protocol `ClientMessage`.                                                                                      |
| `bridge/errors.ts` | `normalizeBridgeError`: maps bridge/IPC failures to a bounded `{ code, message }`; never surfaces raw process or path details.                                                                                                                                                                                                 |

## App wiring

- `app/connection.ts` derives a pure `ConnectionView`
  (loading/ready/error + retryable) from the supervisor's `ServerStatus`.
  Kept free of React so the state machine is unit-testable.
- `app/use-clay-session.ts` owns lifecycle: **subscribe before bootstrap**,
  then bootstrap once on mount; reconnect is explicit. Ordering matters —
  events emitted during bootstrap must not be dropped.
- `shell/workspace-controller.ts` is the envelope router: runtime snapshots,
  document events, transient menus, diagnostics, viewport patches, and routed
  client-command requests all reach the owning pane session — there is no
  app-wide document session mirror (`session-singleton.ts` was deleted in Plan
  099). Requests each have one handler and update pane/tab state. Plan 105
  split its dispatch: `workspace-envelope.ts` (`handleEnvelope` + `EnvelopeContext`)
  for server envelopes, `workspace-commands.ts` (`dispatchClientCommand` +
  `CommandContext`) for client commands; the controller (589 lines, down from
  962) keeps session wiring, context adapters, and tests
  (`workspace-controller.test.ts` covers restore/open routing through the seams).

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
- `ViewportRenderPatch` is included in the document session's bounded feature
  event stream and handled as one complete event; stale request IDs are dropped
  by the owning editor projection, and its terminal status—not member arrival—
  releases viewport pacing.
- No browser storage (`localStorage` et al) anywhere — configuration
  authority stays server-side (guard-tested).

## Tests

- `frontend/src/editor/sync/session.test.ts`: document-session forwarding keeps
  atomic viewport patches available to mounted and remounted editor projections.
- `frontend/src/test/*`: store transitions, dispatcher routing, error
  normalization, and editor integration.
- `frontend/src/shell/workspace-controller.test.ts`: restore/open path routing,
  shell status snapshots, and persistence isolation.
- `cargo test -p clay-desktop` covers the Rust side of the same contract
  (see [Desktop Typed Bridge](desktop-typed-bridge.md)).

## Related

- [Tauri Desktop Shell](tauri-desktop-shell.md) — process/command surface.
- [Frontend Edit Synchronization](../flows/frontend-edit-synchronization.md).
- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md).
