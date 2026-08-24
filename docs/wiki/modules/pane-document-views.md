# Pane Document Views (Phase 22.2)

> **Historical — removed in Plan 097 Phase 12.** Per-pane hosting moved to the
> React workspace: [React CodeMirror Editor](react-codemirror-editor.md) and
> [React Tabs, Splits, and Layout Persistence](react-tabs-and-splits.md).

## Source

- `src/masonry_pane_document.rs` — `PaneDocumentView` (per-pane editor view)
- `src/masonry_editor.rs` — `EditorWidget` (connection chrome, owns pane 1's view)
- `src/masonry_shell/mod.rs` — `ClayShellWidget` (pane hosts, `pane_targets`, focus actions)
- `src/masonry_pane_host.rs` — `PaneContentHost` / `PaneContent::Document`
- `src/client/mod.rs` — `ClientSyncState` per-document map, `ClientConnectionEvent::document_id()`
- `src/driver/mod.rs` — `Driver` event routing, pending-open attribution, close arm
- `src/editor/document_session.rs` — `DocumentSessionStore` per pane
- `src/server/behavior.rs`, `src/server/ops/mod.rs` — per-document behavior manifest layers (see [Behavior Manifests](behavior-manifests.md))
- `src/protocol/runtime.rs` — `DocumentRuntimeRenderState::behavior_manifest`

## Overview

Phase 22.2 (2026-08-05) turns pane leaves from placeholders into live document views: each tab hosts one workspace, each pane is an independent view that can open a document from that tab's workspace, per-pane major modes activate concurrently, and every pane keeps its own caret, selection, viewport, dirty state, and shadow-session stash. A file can be open in at most ONE pane of the workspace (duplicate opens focus the owner), and all open flows target the focused pane.

The architecture keeps ONE `EditorWidget` per connection as **connection chrome** (SDUI sidebar, package panels, overlays, shell preferences, runtime-snapshot validation) and extracts a lightweight `PaneDocumentView` per pane that owns the per-document editing state. A single `ClientEditQueue` is shared by all panes; its `ClientSyncState` was refactored from single-document to a per-document map so edits for different documents can be in flight concurrently.

## Plan 088 status chrome

`PaneDocumentView::paint_status_line` reads cached `ResolvedUiTheme` tokens (`surface.control`, `text.primary`, spacing, and border) with legacy `StyleRegistry` fallbacks. The status string still carries connection, access, document/version, dirty, diagnostic, pending-edit, and recovery text, so state is never communicated by color alone. Welcome layout bypasses only the Clay-owned workspace-browser slot; package fixed slots remain authoritative. Plan 089 closes the recovery synchronization defect: the parent now publishes the shared welcome virtual `Status` node from current `WelcomeState`, so the WelcomeWidget and pane/status chrome both report `Connection lost` / `Disconnected` after a connection event.

## Plan 089 recovery and loading closure

`set_status` still rebuilds the shared `WelcomeState`, but accessibility ownership is parent-driven: `PaneDocumentView::accessibility` and `EditorWidget::accessibility` push the current welcome `Status` node using the stable virtual status slot, while `WelcomeWidget::accessibility` retains only the child reference. This avoids stale child accessibility caches and keeps the status announcement synchronized without per-frame polling. Because Masonry caches child paint scenes independently, `EditorWidget::sync_region` also marks the visible `WelcomeWidget` child for `request_render()` after event-driven state changes; otherwise the parent status/accessibility tree could update while the painted welcome card stayed stale. The disconnected regression test drives this sync before redraw, and the live recovery capture verifies matching `Connection lost` / `Disconnected` copy.

The loading review path is also restore-aware. The capture harness starts its server in the isolated workspace, waits for the client shell, triggers one watcher reload after handshake, and opens `loading.txt`; the runtime SDUI tree is then reconciled into the retained sidebar. See [Repeatable UI Review Harness](ui-review-harness.md) and [SDUI / Package-UI Retained Masonry Reconciliation](masonry-sdui-region.md).

## How It Works

### Per-document sync state (`src/client/mod.rs`)

`ClientSyncState` now holds `HashMap<DocumentId, DocumentSyncState>` where each entry records `confirmed_version`, `optimistic_version`, pending edit reservations, `last_resync`, and the document's `lease_id`. `ClientEditQueue::enqueue_edit_event(document_id, ...)` reserves a pending edit and attaches the right lease per document; `snapshot_for(document_id)` exposes one document's state to pane views; `total_pending_len()` and `confirmed_version_for(document_id)` keep the connection loop's ack/reject logic per-document. `enqueue_completion_request` and `sync_snapshot_for` follow the same per-document resolution. Server lease validation remains per-document (`validate_lease`), so each outbound edit must carry that document's lease — the map makes that automatic. `DocumentReloaded` and `ResyncSnapshot` events update the specific document's state regardless of which pane is active.

### `PaneDocumentView` (`src/masonry_pane_document.rs`)

The per-pane view encapsulates what used to be `EditorWidget`'s document state:

- One `EditorSurface` (buffer, caret/selection, viewport, decorations, diagnostics, history, typography, theme) — the pane's live document, or a `blank_surface()` placeholder when no document is open.
- One `DocumentSessionStore` (Phase 20 shadow sessions, LRU-capped at 64 per pane) with `stash_active_session` / `activate_document` preserving the existing stash semantics locally.
- One `EditorStatus` painted as the pane's own status line at the bottom of its rect (`paint_status_line`).
- Local transient menus (completion, save-conflict, sync-recovery) mirrored to the chrome's overlay via `pending_menu_sync` (the chrome drains them on `MenuStateChanged`). Completion menus also carry fixed-point caret/IME bounds so the shared overlay host can place them beside the active line.
- Request-id allocators and the `last_decoration_viewport` dedup for decoration requests.

`RuntimeBaseline` (behavior manifest, active theme, active typography) seeds freshly mounted views from the chrome's current runtime state. The view's Widget trait methods are invoked through inherent `handle_*`/`on_*` methods delegated by the chrome or the driver, avoiding Widget-trait resolution across widget boundaries.

`close_pane()` sends capability-gated close requests for the active document AND every retained session in the store (`DocumentSessionStore::document_ids()` + `clear()`), then resets to a blank surface. `guard_pane_close()` returns true when the active document is dirty — the driver blocks the close and shows the save-conflict menu instead (no topology change, no lease release until resolved).

### Clay-owned welcome entry state (`src/masonry_welcome.rs`)

The server's per-tab welcome document is an empty editable sentinel; the client presents it through a retained `WelcomeWidget` until a real `DocumentOpened` event arrives. `PaneDocumentView::with_initial_state` marks an empty bootstrap snapshot as welcome-visible and carries a sanitized workspace basename into `WelcomeState`. `EditorWidget::default` also uses the entry surface for local-fallback/no-server windows.

The welcome surface is bounded and local: it paints a token-driven card, shortcut help, connection/access/runtime guidance, and two buttons routed through existing client-local commands (`documents.clientOpenFileDialog` and `workspace.clientOpenFolderDialog`). It performs no filesystem scan, recent-path query, JavaScript, IPC, or file authority operation in layout/paint. Workspace and status changes refresh the `Rc<RefCell<WelcomeState>>` before rendering; diagnostics pass through shared accessibility sanitization and the final label remains within the 256-character recovery ceiling.

While visible, the view stashes the native editor and rejects document text edits, exposes a group with a polite status child and button `Click` actions, and keeps the welcome pod registered for Masonry traversal. Global bindings use the editor surface's global-only chord route while welcome is visible, so shell topology, tab, and Command Centre commands remain available without enabling text edits. A real `DocumentOpened` sets `welcome_visible = false`, restores the multiline editor/input path, and preserves server-owned document IDs, leases, and session routing. The child remains registered but stashed when hidden, avoiding an orphaned accessibility walk.

### Event routing (`src/driver/mod.rs` Driver + `src/masonry_editor.rs` chrome)

`ClientConnectionEvent::document_id()` classifies events. The `Driver` routes:

- **Document-scoped events** (DocumentOpened/Saved/Reloaded, EditAck/Rejected, ResyncSnapshot, decorations, diagnostics, completions, intelligence): `route_document_event` walks panes focused-first and applies the event to the pane owning the document.
- **DocumentOpened**: `route_document_opened` decides Owner > Pending > Active — (1) if a pane already owns the document, apply there and focus it (duplicate open); (2) else if the requesting pane has a pending open matching this result, mount there (focused-pane-targeted open); (3) else fall back to the active pane. Mounting a view on a placeholder pane uses the chrome's `RuntimeBaseline` + a clone of the master `ClientEditQueue` (`edit_queue_shared`).
- **Connection-wide baseline events** (Theme, Typography, BehaviorManifest, CaretStyle, Disconnected, ConnectionError, RuntimeDiagnostic, ServerError): `fan_out_event` applies to the chrome AND every mounted `PaneDocumentView`.
- **PaneFocused(PaneId)** (submitted by `ClayShellWidget` on Tab cycling and pointer activation): the driver synchronizes Masonry focus to that pane's routing target.

The chrome's own `apply_connection_event` handles SDUI/panels/overlays/shell-preferences/runtime-snapshot validation and forwards document-scoped events to its embedded pane-1 view (also via the focused-first routing in the driver, which owns the pane→view map).

### Pending-open attribution

`route_document_opened` cannot pre-map a path to a document_id (path canonicalization is server authority), so the driver records pending opens at the three interception points (`ClientUiCommandResult::SelectedFile` native dialogs, `apply_native_dialog_completion`, `route_sdui_intent` for `workspace.openFile`/`openFuzzyFile`, and server-side keybindings via `RecordPendingOpenIntent`). `PendingOpenRequest` matches in-root browser/fuzzy opens by `(workspace_root_id, relative_path)` and native-dialog opens by absolute canonical path; `take_pending_open_for` consumes the match when `DocumentOpened` arrives. Pending entries are removed when their pane closes. The pure decision function `decide_open_route` (Owner > Pending > Active) is unit-tested without a window harness.

### Duplicate-open no-op and cross-pane switcher

Because the server returns the existing lease with full metadata on a duplicate open, the client's pane registry (pane→document map in the driver) detects "already owned" and focuses the owner; `PaneDocumentView::apply_connection_event` additionally no-ops redundant `DocumentOpened` frames for its live document so caret/content are never reinstalled over a live buffer.

`show_open_documents_menu(other_panes: &[CrossPaneDocumentEntry])` lists the focused pane's active + retained sessions plus every other pane's sessions (`pane N: <name>` labels, active/dirty markers). Activating a cross-pane entry emits `EditorAction::ActivateDocumentInPane(pane_id, document_id)`; the driver switches the OWNING pane's document (stashing its prior session) and focuses it — consistent with one-view-per-document.

### Shell integration (`src/masonry_shell/mod.rs`)

`PaneContent` gained `Document(PaneDocumentView)`; `set_document_view` / `clear_content` (via `std::mem::replace`) mount/unmount views on hosts. `ClayShellWidget` tracks `pane_targets: BTreeMap<PaneId, WidgetId>` for routing, submits `EditorAction::PaneFocused(pane_id)` on Tab/pointer focus changes, and `focus_fallback_widget_id()` returns the active pane's target. Pane close removes the routing target first, then reconciles hosts.

### Per-document behavior manifest layers

Major-mode manifests are now scoped: the server keeps a global manifest plus per-document layers (see [Behavior Manifests](behavior-manifests.md)); the client's `PaneDocumentView::apply_behavior_manifest` installs content only when the manifest's scope is global or matches the view's document, and otherwise bumps only the behavior version (`EditorSurface::update_behavior_version`) so outbound stamps stay current without cross-pane keymap/autocomplete bleed.

## Invariants and Constraints

- 1:1 client-local pane↔document mapping per workspace; a document never has two views.
- Keystrokes, menus, status, and IME follow pane focus; document-scoped events follow `document_id`.
- No cross-pane state bleed: surfaces, session stores, and behavior layers are per-pane; the shared edit queue tracks sync state per document.
- Closing a dirty pane is blocked until the save-conflict menu resolves; clean closes release active + retained leases through the server's capability-gated close path. Plan 086 manual verification still found a follow-up focus/accessibility panic during the dirty-close path (`Focused ID #4 is not in the node list`); see [Plan 086 release integrity](plan086-release-integrity-and-accessibility.md).
- Opens never grant authority client-side; the server's canonical-path duplicate detection and per-(client, document) leases remain authoritative.
- Hot path: keystroke handling in a 4-pane shell touches only the focused pane, no IPC (guarded by `pane_document_typing_requires_no_server_or_js`).

## Phase 22.6: Display-Name Plumbing for Pane Accessibility

Phase 22.6 gives pane hosts numbered, document-named a11y labels
(`Empty pane N of M` / `Pane N of M: editor` / `Pane N of M: {name}` /
`Pane N of M: document`). The host cannot read its child widget during
`accessibility()`, and `mount_document_view` takes no path, so the driver
routes the display name down:

- **`ClientConnectionEvent::metadata_path()`** (`src/client/mod.rs`) returns
  `Some(&metadata.path)` only for `DocumentOpened`/`DocumentReloaded`
  (the variants that carry a path; `ClientResyncSnapshot` has none).
- **Driver routing** (`src/driver/mod.rs`): `route_document_event` preserves
  `(pane, target)` pairs and calls `shell.set_pane_document_name(path)`
  when consuming; `route_document_opened` captures `opened_path` and sets
  the name in the owner and new-open branches. The alternative —
  changing `apply_connection_event`'s signature — was rejected (ripples
  through EditorWidget/view/tests); Masonry `Properties` flow downward,
  so they cannot carry the name up.
- **Shell boundary** (`src/masonry_shell/mod.rs`): `set_pane_document_name`
  maps `Option<&str>` through `sanitize_document_display_name`
  (pub(crate) in `src/editor/accessibility.rs`, so the bin crate cannot
  call it directly — sanitization happens here) and calls
  `host.widget.set_document_display_name`, which requests an a11y update.
  `clear_content` also clears the name. Hosts learn `M` via
  `set_pane_count` during shell reconcile (guarded by
  `registered_panes`, see the masonry-shell page).

## Phase 22.7: Field-Group Decomposition

Phase 22.7 (finding C4) decomposed `PaneDocumentView`'s 30 fields into two
helper structs — mechanical delegation, no logic change:

- `PaneRequestBookkeeping` (private fields): the seven request-id fields
  (`next_transaction_id`, `next_completion_request_id`,
  `active_completion_request_id`, `next_language_intelligence_request_id`,
  `active_language_intelligence_request_id`,
  `next_selection_query_request_id`, `pending_selection_query`) with a
  shared `bump(next)` allocator (`saturating_add(1).max(1)`), the
  `next_*_request_id` accessors, `clear_active()` (both active ids only),
  `reset()` (active ids + pending query), and
  `take_completion_if_current(id)` / `take_language_intelligence_if_current(id)`
  for the rejected/current-request checks.
- `PaneMenuSync` (private fields): `menu`, `pending_menu_sync`, and
  `menu_session_ids: Rc<Cell<u64>>` with `push(menu)` (clone + set pair),
  `take_pending()` (the one-shot tri-state: None = none, Some(Some) =
  show, Some(None) = clear), and `next_session_id()` (interior
  mutability). `PaneDocumentView::new` overrides `session_ids` from the
  caller's `Rc<Cell<u64>>`.

`PaneDocumentView` dropped 30 → 20 fields; `pending_definition_navigation`
and `last_decoration_viewport` stay on the view. The 30 `self.menu` read
sites re-point to `self.menu_sync.menu`. Tests:
`request_bookkeeping_allocates_unique_ids` (allocators never collide,
`clear_active`/`reset` semantics, `take_*_if_current` matches only the
current request id) and `menu_sync_pending_semantics` (one-shot tri-state
`take_pending`, `next_session_id` allocation) in
`src/masonry_pane_document.rs`.

## Plan 087: bounded completion projection

`PaneDocumentView::apply_completion_result` consumes only the current request ID,
then rechecks document ID, document version, and behavior version before building
a menu. Empty, rejected, stale, and expired results clear the current completion
surface before the driver reconciles overlays. Provider timeout/error statuses use
sanitized non-blocking status diagnostics rather than a `No completions` panel.

For non-empty results, `completion_anchor()` uses the editor's IME cursor area
(including bounded preedit width) and the view publishes it after layout to the
retained `PackageOverlayHost`. `completion_overlay_rect` clamps the result to
the active pane, prefers below/above-caret placement, caps width at 480 logical
pixels, and exposes at most eight visible rows. The shared `SduiScrollViewport`
keeps the selected list row visible; completion items remain local accept payloads,
not command targets. Centered Command Centre/path sessions continue using the
window-level modal layer, scrim, and focus-restoration path.

Focus/accessibility: the completion popup is modeless by construction — the pane
never takes Masonry or AT-SPI focus away from the editor Entry, the menu is a
non-Dialog `Menu` with `MenuItem` rows and a polite status, and the selected row
carries the `selected` state. The welcome surface (also hosted by this view)
exposes a `Group` with two `Button` children and a polite `Status` virtual node
(slot `STATUS`) and refuses text input while visible; its label stays within the
256-character sanitized ceiling. Both surfaces are fed through
`accesskit_consumer::Tree` in tests and are represented in the review harness
artifacts (`scripts/capture-ui-review.sh`, see
[Repeatable UI Review Harness](ui-review-harness.md)); interactive completion
recapture remains host-dependent and must not be inferred from structural tests.

## Known Ceilings

- SDUI sidebars and package panels/overlays are window-scoped chrome (per-client), not per-pane; packages cannot contribute per-pane chrome yet.
- No per-pane tab strips or document chrome beyond the status line until Phase 22.3.
- Completion rows are retained widgets rather than virtualized; the shared result cap (256 items) and eight visible-row budget bound work.
- Topology and active per-pane document identity persist through the
  client-owned `layout.json` v2 path (Phase 22.5); unsaved edits, caret,
  viewport, and retained inactive sessions are intentionally not persisted.

## Tests

- `src/masonry_pane_document.rs`: event isolation, session stash/activate, close-pane release + blank reset, dirty-close gate with conflict menu, per-document edit queueing, duplicate-open no-op, cross-pane menu aggregation + routing, failed-opens leave state unchanged, runtime-snapshot baseline restore, scope-aware manifest install, and completion empty/error/stale dismissal. Command: `cargo test --lib masonry_pane_document --quiet`.
- `src/masonry_shell/mod.rs`: panes host independent document views with document-scoped routing, typing isolation hot-path guard, routing-target cleanup on pane close, concurrent per-pane major modes isolated across behavior manifests. Command: `cargo test --lib masonry_shell --quiet`.
- `src/client/mod.rs`: per-document sync state, stale-ack filtering by document, per-document reload updates. Command: `cargo test --lib client --quiet`.
- `src/driver/mod.rs`: `decide_open_route` pure-function tests (owner > pending > active, path matching);
  Phase 22.6 label routing (`route_document_event`/`route_document_opened`
  keep `(pane, target)` pairs and set display names).
- `src/masonry_package_region.rs`: `menu_selection_keeps_selected_row_in_scroll_viewport` and `centered_command_center_scrolls_60_results_without_overflow` verify long-menu selection visibility and centered containment.
- `src/masonry_sdui.rs`: `completion_menu_observation_uses_caret_bounded_geometry` verifies structural overlay bounds.
- Guard suites scanning the new module: `tests/editor_performance_invariants.rs`, `tests/ui_primitive_conformance.rs`.
- Full Linux verification: `cargo test --all-targets`; focused Plan 088
  checks include `cargo test --lib masonry_pane_document`, `cargo test --lib
  masonry_editor`, `cargo test --test editor editor_performance_invariants`,
  and `cargo test --test editor ui_primitive_conformance`.

## Related

- [Multi-Document Sessions](multi-document-sessions.md) — session store mechanics per pane
- [Behavior Manifests](behavior-manifests.md) — per-document manifest layers
- [Masonry Shell Runtime](masonry-shell.md) — pane hosts, focus actions, routing targets
- [Masonry Editor Widget Status Observability](masonry-editor.md) — connection chrome responsibilities
- [Server Document State](server-document-state.md) — server lease/duplicate authority
- `docs/reference/primitives/shell-layout-strategy.md` (Phase 22.2 section), `docs/reference/clay-js-api/editor/client-show-open-documents.md`, `docs/reference/clay-js-api/shell/client-close-pane.md`
- `test-plan/13-window-splits.md` (D1–D15), `test-plan/03-files-and-workspace.md` (F3a–F3c, F12a)
- [Repeatable UI Review Harness](ui-review-harness.md) — plan 087 fixture/capture workflow (welcome and completion states)
