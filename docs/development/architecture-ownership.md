# Architecture Ownership Map (post-Plan 097 cutover)

> Current-state ownership for the Tauri v2 + React desktop client and the
> unchanged authoritative Clay server. The pre-migration native-client
> ownership record lives in git history and in
> [Tauri/React Desktop Cutover](../wiki/modules/tauri-react-cutover.md);
> the deleted native-client widgets are guarded against return by
> `removed_native_client_modules_cannot_return`.

One page of who owns what, so each change has exactly one named owner and no
change can relocate or duplicate authority. Dependency direction:
**coordinator → private responsibility module → existing state/typed result**;
no new trait unless multiple current implementations already require one.

## Server modules

| Module (lines) | State owns | Behavior owns | Execution owns | Persistence | Validation | Cleanup |
|---|---|---|---|---|---|---|
| `server/mod.rs` (6,353) `IpcServer` | Listener, shared `WorkspaceState`/`StaticSduiState`/`ParseCoordinator`/`RuntimeDiagnosticStore`, active theme/typography, `RuntimeGenerationStore` (active generation) | Accept loop, per-connection task spawn, `reload_runtime_generation` (swap only after success), `refresh_open_documents_after_reload`, typography broadcast | Tokio accept + spawned connection tasks; background parse/analysis | Workspace save/reload via `workspace.rs`; `layout.json` is client | Endpoint/workspace-root validation (`InvalidWorkspaceRoot`), generation swap-after-success | Connection cleanup delegated to `connection.rs`; `sweep_expired_tabs`; drop of store drops generation → runtime workers |
| `server/connection.rs` (11,535) | Per-connection `RoutedTabState`, `FileOpenCapabilityPool`, `RuntimeDiagnosticStore` instance, `ServerMenuSessions` (loop-local), `ConnectionOutputSubscriptions` | Handshake/welcome lane, bootstrap message sequence, select-loop dispatch of every `ClientMessage` family, menu session lifecycle, edit ack/resync | One Tokio task per connection; async broadcast lanes (theme/typography/prefs/snapshot/parse/diagnostics/analysis/completion/intelligence) | None directly; `persist_settings_change` routes to config watcher | `client_message_identity` → `ClientId`, fail-closed tab-state routing, behavior-version gate, capability tokens, menu session-id membership | `cleanup_connection_documents` is the ONLY outer exit boundary (all loop returns); `ReadPumpGuard` aborts pump; `ConnectionOutputSubscriptions::drop` |
| `server/js_runtime.rs` (13,333) | `ClayJsRuntimeService` → two `DomainRuntime` (trusted + third-party), `RuntimeWorker`, `ClayModuleLoader`, shared `ClayOpState`, `ClayRuntimeEvaluation` | Persistent deno_core facade: `evaluate_controlled_module`, `load_configuration_from_root*`, `dispatch_to_domain`, `replay_third_party_domain`, `production_reload` (trusted-only rebuild), `absorb_cross_domain_evaluation`, handler adapters (parse/completion/intelligence) | One V8 owner thread per domain per generation; watchdog thread per evaluation | None in module (grants/records persist in `authorization.rs`/`packages/`) | Facade allowlist (22 trusted / 13 public), deny-by-default module loader, op/import policy, `ClayRuntimeError::diagnostic` sanitization | `RuntimeWorker` drop → isolate teardown; timeout/heap poison worker; analysis-worker timeout poisons owning domain worker |
| `packages/record.rs` (5,406) | None (pure assembly) | `assemble_package_record` coordinator: manifest reuse + contribution-family parsers + docs/perf/API-dependency metadata | Load/enable/config-time only; never editor hot path | None | Package-prefixed IDs, permission checks, provenance, payload budgets, `PackageRecordError`/`PackageRecordRule` vocabulary, `Box<str>` diagnostics | None |
| `server/menu_sessions.rs` + `control_center.rs` | `ServerMenuSessions` per connection, `ControlCenter` (persisted `selected_index`) | Server-owned menu session lifecycle: open/replace/filter/move/activate/cancel, tab-switch cancel, generation-stamped catalogue | Connection task (server-authoritative) | None | Query clamp, `TRANSIENT_MENU_MAX_*`, stale-generation rejection, activation resolves from installed entries only | Drop-on-exit sweep (connection-close is the single cleanup path) |

## Desktop client modules (Tauri v2 + React)

| Owner | State owns | Behavior owns | Presentation/exe | Validation | Cleanup |
|---|---|---|---|---|---|
| `src-tauri/src/server.rs` `Supervisor` | Child process handle, endpoint status | Spawn/adopt/kill `clay-server`, classify spawn errors, fail-closed disconnected marking | Owns nothing visual; exposes typed status commands only | Endpoint acceptance check before adoption | Kill on shell exit |
| `src-tauri/src/bridge/*` (session/forwarder/dto/editor/layout/agent) | One live `clay::client` session, bounded ordered forwarder with latest-wins decoration coalescing, per-tab edit queues | Bootstrap/reconnect generations, sanitized size-capped request stamping, AG-UI event adaptation + channel fan-out | serde-derived protocol JSON over Tauri commands/channels; no UI logic | Envelope/tab-id validation, bounded frame sizes, orphan-free supervision | Forwarder drop closes channels; session drop releases server connection |
| `frontend/src/app` (`AppShell`, router, `use-clay-session`) | Route, live phase/status text | Event subscription before bootstrap, reconnect UX | Landmarks (one `main`), header tabs, footer status live region | Status derived from connection phase; diagnostics override display only | Unmount unsubscribes events |
| `frontend/src/shell` (`WorkspacePanes`, workspace controller, tab store) | Per-tab pane/split tree, per-tab workspaces, transient menu snapshot, settings-open flag | Server-authoritative menu intent routing (query/move/activate/cancel), client command routing (deny-by-default), dialog capability conversion via narrow Tauri commands | Split rendering, Command Centre / Path Browser modals, dirty-close confirm | Layout restore version checks; stale-version package-UI updates rejected | Tab close tears down pane sessions; layout persisted through existing Rust parser |
| `frontend/src/editor` (`ClayEditor`, controller, extensions) | CodeMirror state, optimistic shadow + pending edit queue, decoration/diagnostic/completion/intelligence state | Local-first edits (bounded ordered deltas queued async), keymap projection from behavior manifests, save/reload/close flows | Editor region labeling, status chrome, theme styles as CSS custom properties | Version/document/behavior staleness guards before applying results; no server/JS round trip before local paint | Close releases leases; disconnect keeps local state for resync |
| `frontend/src/sdui` + `PackageWorkspace` | Generation-stamped SDUI/package snapshots, stable-ID reconciliation map | Typed inert action emission (`command_id` + bounded args only) | Slot-composed panels/overlays/empty-tab content, provenance labels, trust-domain badges | Duplicate-ID/slot/visibility validation before install; ui_version match on actions | Snapshot replacement uninstalls superseded generation atomically |
| `frontend/src/chat` (ChatPanel + agent transport) | AG-UI message snapshot, run status | `AbstractAgent` transport over Tauri channel; submit/cancel intents to declared commands | Transcript/composer presentation; composer interaction is local-only | Credential-free events; inert tool payloads | Abort cancels run; unsubscribe drops channel |
| Retained neutral Rust modules (`src/shell/{layout,layout_persist,file_browser,path_browser,fuzzy,transient_menu}`, `src/editor/{position_map,theme,typography}`, `src/client`) | Layout tree/persistence schema, browser/path/fuzzy data, position maps, typography roles, `ClientEditQueue`/session event types | Pure computation used by both bridge and tests | No painting — renderer-neutral by contract | Same validation rules as before cutover | Drop semantics unchanged |

## Server-owned menu sessions — presentation bridge

`server TransientMenuSession → Tauri bridge forwarder → React workspace controller (snapshot state + typed intent enqueue) → CommandCentre modal (focus containment, listbox projection, polite count)`. Server owns session/filter/selection/activation. Client presentation ownership is ONE surface per concern: modal geometry/focus/restoration in the React Aria wrapper, intent emission through the bounded edit queue, accessibility strings from the sanitized snapshot. The workspace controller routes snapshots to panes but owns no menu session state.

## UI primitive constraints (apply to every shell/editor move)

- Reuse catalog primitives/components (`docs/reference/ui-components.md`, clay-ui `references/components.md` + `tokens.md`, React Aria Components) before any new component; a move cannot hand-roll bespoke UI where a catalog entry covers the need.
- Token-only styling: colors/spacing/radii/opacity arrive as resolved `--clay-*` CSS custom properties from the Rust theme resolver; font role (`ui`/`monospace`/`proportional`) + `UiTextVariant` only. No raw colors, raw CSS values, concrete families/sizes introduced by a move.
- Inert contributions: packages declare inert validated UI + typed tokens; no package JS in render/layout/pointer/key; no DOM handles/raw ops; `serverRequestLayoutIntent` remains the only public layout seam.
- Additive-only catalog/token names; keep `references/components.md` / `tokens.md` current in the same change.
- Duplicate overlay mounting, mirrored per-frame state, and full-tree re-render on every update are forbidden; one SduiRenderer reconcile path keyed by stable node IDs.

## Performance hot paths and guards (before any move)

| Hot path | Budget / guard |
|---|---|
| Keypress → local glyph/caret update | `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` = 16 ms; CodeMirror applies edits locally first, bounded ordered deltas queue asynchronously (`frontend/src/editor` tests + `tests/suites/protocol.rs` hot-path guards) |
| Pane render / tab switch | `PANE_PAINT_P95_BUDGET_MS` = 1 ms, `TAB_SWITCH_P95_BUDGET_MS` = 1 ms; React commit measured by the frontend test suite; no synchronous server/JS round trip in the keystroke path |
| Edit ack / scroll-layout-render | `EDIT_ACK_P95_BUDGET_MS` = 40 ms, `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` = 16 ms |
| Command centre open / filter | `COMMAND_CENTRE_OPEN_P95_BUDGET_MS` = 50 ms, `FILTER_UPDATE_P95_BUDGET_MS` = 4 ms; listing payload 64 KiB, `TRANSIENT_MENU_MAX_ITEMS` = 256 |
| Runtime eval / config / mode activation | `JS_RUNTIME_EVALUATION_TIMEOUT_MS` = 5 s, heap 128 MiB; `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS` = 25 ms, `MODE_ACTIVATION_P95_BUDGET_MS` = 100 ms |
| IPC frame / edit doc | `DEFAULT_MAX_FRAME_SIZE` = 1 MiB, `MAX_OPENABLE_FILE_BYTES` = 768 KiB, no full-doc IPC on ordinary edits (deltas only) |
| Frontend bundle budgets | Startup shell ≤ 180 kB gzip, total application ≤ 400 kB gzip; enforced by `frontend/scripts/bundle-budget.mjs` in CI |
| Tests pinning these | `src/perf/budgets.rs` constants, `benches/protocol_server_baselines.rs`, `tests/performance_budgets.rs`, `tests/package_loading.rs`, codec/malformed-archive suites, frontend Vitest editor/SDUI/agent suites |

## Security: canonical identity and cleanup authority (not relocatable)

- Documents/files/workspaces: server `DocumentState` + `WorkspaceState` open registry, leases, region locks, version ordering; client shadows only.
- Connection identity: server-assigned `ClientId`/`TabId` (tab registry), `OutputRouter` authorization, `MAX_ACTIVE_CONNECTIONS` cap, file-open capability tokens.
- Packages/runtime: `RuntimeDomain` from compiled bundled inventory + exact provenance, never name/prefix; adopted runtime lacks internal ops/modules; cross-domain values typed/bounded/inert; `language-server` grants config-root-only, sealed before load.
- Cleanup exactly-once: connection → `cleanup_connection_documents`; menu sessions → drop-on-exit; read pump → `ReadPumpGuard`; runtime workers → `RuntimeWorker` drop/poison; pane hosts → shell reconcile; stale tabs → server sweep. No extraction may create a second cleanup path or a second canonical identity.

## Ownership review checklist (task 1 gate, apply before each extraction)

- [ ] Every mutable state struct has exactly one owning module/struct (grep for field writes; no mirrored copies added).
- [ ] Every cleanup symbol above has exactly one call site or one owning struct (`Drop`/poison/sweep) — count call sites before moving.
- [ ] Every message family has one dispatch owner; extraction keeps the loop as coordinator, never a second match on the same enum crate-wide.
- [ ] Moved code is private/`pub(crate)` — no public surface appears; JS API surface and protocol unchanged.
- [ ] Hot-path guards from the table still compile and pass after the move.
- [ ] New module has ≥ 2 coherent responsibilities/callers OR owns one state machine (reject line-count slicing).
- [ ] Go/no-go per seam: stop/revert any extraction whose only effect is more indirection or undisprovable parity.