# Architecture Ownership Map (Plan 090 baseline)

Pre-refactor baseline for Plan 090: one page of who owns what, so each
extraction seam has exactly one named owner and no extraction can relocate or
duplicate authority. Dependency direction: **coordinator → private
responsibility module → existing state/typed result**; no new trait unless
multiple current implementations already require one.

## Server modules

| Module (lines) | State owns | Behavior owns | Execution owns | Persistence | Validation | Cleanup |
|---|---|---|---|---|---|---|
| `server/mod.rs` (6,353) `IpcServer` | Listener, shared `WorkspaceState`/`StaticSduiState`/`ParseCoordinator`/`RuntimeDiagnosticStore`, active theme/typography, `RuntimeGenerationStore` (active generation) | Accept loop, per-connection task spawn, `reload_runtime_generation` (swap only after success), `refresh_open_documents_after_reload`, typography broadcast | Tokio accept + spawned connection tasks; background parse/analysis | Workspace save/reload via `workspace.rs`; `layout.json` is client | Endpoint/workspace-root validation (`InvalidWorkspaceRoot`), generation swap-after-success | Connection cleanup delegated to `connection.rs`; `sweep_expired_tabs`; drop of store drops generation → runtime workers |
| `server/connection.rs` (11,535) | Per-connection `RoutedTabState`, `FileOpenCapabilityPool`, `RuntimeDiagnosticStore` instance, `ServerMenuSessions` (loop-local), `ConnectionOutputSubscriptions` | Handshake/welcome lane, bootstrap message sequence, select-loop dispatch of every `ClientMessage` family, menu session lifecycle, edit ack/resync | One Tokio task per connection; async broadcast lanes (theme/typography/prefs/snapshot/parse/diagnostics/analysis/completion/intelligence) | None directly; `persist_settings_change` routes to config watcher | `client_message_identity` → `ClientId`, fail-closed tab-state routing, behavior-version gate, capability tokens, menu session-id membership | `cleanup_connection_documents` is the ONLY outer exit boundary (all loop returns); `ReadPumpGuard` aborts pump; `ConnectionOutputSubscriptions::drop` |
| `server/js_runtime.rs` (13,333) | `ClayJsRuntimeService` → two `DomainRuntime` (trusted + third-party), `RuntimeWorker`, `ClayModuleLoader`, shared `ClayOpState`, `ClayRuntimeEvaluation` | Persistent deno_core facade: `evaluate_controlled_module`, `load_configuration_from_root*`, `dispatch_to_domain`, `replay_third_party_domain`, `production_reload` (trusted-only rebuild), `absorb_cross_domain_evaluation`, handler adapters (parse/completion/intelligence) | One V8 owner thread per domain per generation; watchdog thread per evaluation | None in module (grants/records persist in `authorization.rs`/`packages/`) | Facade allowlist (22 trusted / 13 public), deny-by-default module loader, op/import policy, `ClayRuntimeError::diagnostic` sanitization | `RuntimeWorker` drop → isolate teardown; timeout/heap poison worker; analysis-worker timeout poisons owning domain worker |
| `packages/record.rs` (5,406) | None (pure assembly) | `assemble_package_record` coordinator: manifest reuse + contribution-family parsers + docs/perf/API-dependency metadata | Load/enable/config-time only; never editor hot path | None | Package-prefixed IDs, permission checks, provenance, payload budgets, `PackageRecordError`/`PackageRecordRule` vocabulary, `Box<str>` diagnostics | None |
| `server/menu_sessions.rs` + `control_center.rs` | `ServerMenuSessions` per connection, `ControlCenter` (persisted `selected_index`) | Server-owned menu session lifecycle: open/replace/filter/move/activate/cancel, tab-switch cancel, generation-stamped catalogue | Connection task (server-authoritative) | None | Query clamp, `TRANSIENT_MENU_MAX_*`, stale-generation rejection, activation resolves from installed entries only | Drop-on-exit sweep (connection-close is the single cleanup path) |

## Client modules

| Widget/Owner | State owns | Behavior owns | Presentation/exe | Validation | Cleanup |
|---|---|---|---|---|---|
| `Driver` (`src/driver/{mod,reconcile,restore}.rs`) | Per-tab `TabState` (edit-queue clone, pending opens, server `tab_id`), active tab, registry snapshot, restore machine | Event classification (`document_id()`), `route_document_event`/`route_document_opened`/`fan_out_event`, tab lifecycle (open/close/switch/reconnect), pending-open attribution, v2 persistence writer | Spawns per-tab event bridges; `PersistenceDue` debounce write | `decide_open_route` owner>pending>active; no-op duplicate opens; dirty-close gate | `apply_registry_reconcile` uninstall; reconnect cancellation flags; persist flush on quit |
| `ClayShellWidget` (`masonry_shell.rs`, 6,064) | `tabs: BTreeMap<ClientId, TabChrome>` + `active_tab`, tab-bar cards/geometry/scroll, `TabChrome` (working-area layout, `pane_hosts`, `pane_targets`, `registered_panes`, focus policy, orphans) | Window-level tab bar, pane host placement + reconcile, split drag/resize/collapse, pane focus policy, a11y tree (TabList/Tab/Status virtual nodes), announcements | Masonry layout/paint/pointer/a11y only; stash inactive tabs | Layout/package-UI update stale-version checks, `tab_card_display_name` sanitization, logical-window clamp | `pending_orphans`/`chrome_orphans` (editor pane host never detaches), `remove_child` before drop, registry-reconcile uninstall |
| `EditorWidget` (`masonry_editor.rs`) | Connection chrome: SDUI native state, `PackagePanelHost`/`PackageOverlayHost` children, shell prefs, runtime-snapshot validation, master `ClientEditQueue` | Applies chrome-scoped events, mirrors runtime baseline, forwards document events to pane-1 view, clipboard (explicit commands only) | GUI-thread event application; never blocks paint/input | `ClientRuntimeStateCandidate` validation before atomic install; sanitized diagnostics | Overlay/menu dismissal on disconnect/replace |
| `PaneDocumentView` (`masonry_pane_document.rs`) | Per-pane: `EditorSurface`, `DocumentSessionStore` (max 64), `EditorStatus`, `PaneMenuSync`, request bookkeeping | Per-document editing, IME, menu sync (local + server-owned dispatch), completion projection, save/conflict chrome | Delegated widget entry points; `close_pane` releases leases | Version/document/behavior staleness guards before applying results | `close_pane` (active + retained sessions), dirty-close gate, blank-surface reset |
| `PackageOverlayHost` (`masonry_editor.rs` child) | Retained overlay region | Hosts completion popup (anchor/geometry from pane view) + centered command-centre layer (scrim, modal containment) | Paints above region, below nothing; modeless completion keeps editor focus | Server snapshots hydrate as inert items; no action payloads cross | Re-layout on anchor/count change; dismiss on session close |

## Server-owned menu sessions — presentation bridge

`server TransientMenuSession → client snapshot → pane view (snapshot hydration + key intent enqueue) → PackageOverlayHost (geometry/focus/a11y projection)`. Server owns session/filter/selection/activation. Client presentation owner must be ONE: geometry, focus restoration, visual host, and accessibility projection for the command centre live in the overlay host + `shell/transient_menu.rs` hydration; the driver routes snapshots chrome+fanned to panes but owns no menu state.

## UI primitive constraints (apply to every shell/editor move)

- Reuse catalog primitives/components (`src/shell/components.rs`, `docs/reference/primitives`, clay-ui `references/components.md` + `tokens.md`) before any new widget; extraction cannot hand-roll bespoke widgets where a catalog entry covers the need.
- Token-only styling: colors/spacing/radii/opacity from typed Clay theme tokens; font role (`ui`/`monospace`/`proportional`) + `UiTextVariant` only. No raw colors, raw CSS, concrete families/sizes introduced by a move.
- Inert contributions: packages declare inert validated UI + typed tokens; no package JS in paint/layout/pointer/key; no Masonry widget handles/raw ops; `serverRequestLayoutIntent` is the only public layout seam.
- Additive-only catalog/token names; keep `references/components.md` / `tokens.md` current in the same change.
- Duplicate overlay reconciliation, per-frame state mirroring, and full-tree invalidation are forbidden; one `PackageOverlayHost`, one reconcile path.

## Performance hot paths and guards (before any move)

| Hot path | Budget / guard |
|---|---|
| Keypress → local paint | `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS` = 16 ms; `pane_document_typing_requires_no_server_or_js`, `shell_command_dispatch_requires_no_server_or_js_runtime` |
| Pane paint / tab switch | `PANE_PAINT_P95_BUDGET_MS` = 1 ms, `TAB_SWITCH_P95_BUDGET_MS` = 1 ms; `benches/window_baselines.rs` geometry benches |
| Edit ack / scroll-layout-render | `EDIT_ACK_P95_BUDGET_MS` = 40 ms, `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS` = 16 ms |
| Command centre open / filter | `COMMAND_CENTRE_OPEN_P95_BUDGET_MS` = 50 ms, `FILTER_UPDATE_P95_BUDGET_MS` = 4 ms; listing payload 64 KiB, `TRANSIENT_MENU_MAX_ITEMS` = 256 |
| Runtime eval / config / mode activation | `JS_RUNTIME_EVALUATION_TIMEOUT_MS` = 5 s, heap 128 MiB; `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS` = 25 ms, `MODE_ACTIVATION_P95_BUDGET_MS` = 100 ms; `benches/runtime_sdui_baselines.rs` |
| IPC frame / edit doc | `DEFAULT_MAX_FRAME_SIZE` = 1 MiB, `MAX_OPENABLE_FILE_BYTES` = 768 KiB, no full-doc IPC on ordinary edits |
| Tests pinning these | `tests/editor_performance_invariants.rs`, `tests/ui_primitive_conformance.rs`, `tests/performance_budgets.rs`, `tests/package_loading.rs`, codec/malformed-archive suites |

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