# Masonry Editor Widget Status Observability

## Source

- `src/masonry_editor.rs`
- `src/masonry_pane_document.rs` (Phase 22.2 per-pane view)
- `src/masonry_shell.rs`
- `src/client/mod.rs`
- `src/client/runtime_state.rs`
- `src/client/clipboard.rs`
- `src/editor/composition.rs`
- `src/editor/history.rs`
- `src/editor/surface.rs`
- `src/editor/theme.rs`
- `src/main.rs`
- `runtime/js/editor.js`
- `runtime/js/theme.js`
- `docs/reference/clay-js-api/editor/client-copy-selection.md`
- `docs/reference/clay-js-api/editor/client-cut-selection.md`
- `docs/reference/clay-js-api/editor/client-paste-clipboard.md`

## Overview

`EditorWidget` composes the native editor surface, hosted SDUI/panel/overlay child widgets, active editor theme registry, and bottom status chrome. The status chrome reflects connection state, document access, confirmed sync version, and the latest sanitized runtime diagnostic forwarded by `ClientConnectionEvent::RuntimeDiagnostic`.

After Phase 18.2, `EditorWidget` is no longer the top-level application layout. `src/masonry_shell.rs::ClayShellWidget` owns the Masonry root and working-area geometry, registers `EditorWidget` as the shell's editor child, and routes focus/action handling back to that child. `EditorWidget` remains responsible for local text input, caret/selection/viewport state, IME preedit overlay/commit, explicit selection copy/cut/paste clipboard commands, undo/redo inverse edits, edit queue emission, SDUI/panel/overlay event application + sync, status chrome, and accessibility. Plan 070 made `EditorWidget` a **container widget** hosting three children in `children_ids` order `[panel_host, region, overlay_host]` (`PackagePanelHost`/`SduiRegionWidget`/`PackageOverlayHost`); see [SDUI / Package-UI Retained Masonry Reconciliation](masonry-sdui-region.md).

Phase 15 adds `SduiStatusObservation`, a `pub(crate)` headless observability struct for tests and internal agent inspection. It is not a Clay JS API surface; it only exposes strings and version metadata already visible in GUI chrome.

## Responsibilities

- Apply `ClientConnectionEvent` values on the GUI thread and update editor, SDUI, active theme, or status state without blocking paint/input paths.
- Atomically install live `RuntimeStateSnapshot` generations through `install_runtime_state_snapshot` after `ClientRuntimeStateCandidate` validation; acknowledge only on success and fail-close without partial state on invalid snapshots.
- Act as the shell-owned editor component under `ClayShellWidget`; it is not responsible for working-area, split-tree, or pane-slot ownership.
- Keep focus/action routed events client-first and editor-local after the shell forwards them to the registered editor child.
- Render and expose accessible status text for connection, access, document, version, and runtime diagnostics.
- Copy the current non-empty editor selection to the OS clipboard on explicit native copy shortcuts (`Ctrl+C` on Linux/Windows, `Cmd+C` on macOS) using the client-owned `src/client/clipboard.rs` wrapper.
- Cut the current non-empty editor selection on explicit native cut shortcuts (`Ctrl+X` / `Cmd+X`): copy then delete as one ordinary local edit gesture.
- Paste OS clipboard UTF-8 text on explicit native paste shortcuts (`Ctrl+V` / `Cmd+V`) as an ordinary local insert/replace edit.
- Provide `EditorWidget::status_observation()` so tests can assert status chrome state without opening a window or painting.
- Keep diagnostics sanitized by displaying only the `RuntimeDiagnostic` code/message supplied by the server protocol or client clipboard wrapper.
- Apply `ClientConnectionEvent::DiagnosticSet` through `EditorSurface::apply_diagnostic_set` for paint-only squiggles; do not conflate range diagnostics with status-bar `RuntimeDiagnostic` text.

## How It Works

`EditorStatus` stores the current `EditorConnectionStatus`, optional document ID, optional confirmed document version, optional `DocumentAccess`, and optional `RuntimeDiagnostic`. Small label helpers derive the user-visible connection, access, document, version, and diagnostic strings. `EditorStatus::text()` builds the exact status line painted by the widget. Range diagnostics (`DiagnosticSet`) are a separate client cache and render path; see [Range Diagnostics](range-diagnostics.md).

`EditorSurface` owns the active `StyleRegistry`. During bootstrap, `ClientInitialState.active_theme` is converted with `StyleRegistry::from_active_theme` and installed before first paint. The same snapshot also builds `ResolvedUiTheme::from_active_theme(&active_theme.design_tokens)` and installs it on `SduiNativeState` via `set_ui_theme` (bootstrap, live `ActiveTheme`, and runtime snapshot paths). Later `ClientConnectionEvent::ActiveTheme` frames rebuild both registries atomically. The editor paint path reads colors from `StyleRegistry`; SDUI/shell geometry reads cached panel defaults and density from `ResolvedUiTheme`.

For a same-document `ResyncSnapshot`, `EditorWidget` calls `EditorSurface::load_resync_snapshot`: server text/access/version replace optimistic state, but the current caret byte offset is restored and UTF-8-clamped instead of jumping to byte zero. It also updates the edit queue's lease and shared sync state, so a stale/lease rejection cannot strand later edits on obsolete authority.

`EditorWidget` also forwards every accepted `ActiveTypography` snapshot to both `EditorSurface` and `SduiNativeState`. `UiTextMetrics` resolves all seven UI variants from the cached UI profile and installed hierarchy; status paint pushes its resolved `FontStack`/size into Parley and derives its bar height from the same line metric. A UI-profile or hierarchy update resets SDUI client-local geometry, requests Masonry layout/render/accessibility work, and never calls the server, configuration JavaScript, or font discovery from paint/input.

Live runtime reloads use a different path. `ClientConnectionEvent::RuntimeStateSnapshot` is validated into a `ClientRuntimeStateCandidate` and installed in one editor pass: behavior, theme, typography (via `install_runtime_typography`, preserving caret/selection/viewport), SDUI, package UI version replacement, and optional decoration/diagnostic resets for the open document. Acknowledgement (`RuntimeGenerationInstalled`) is sent only after that pass; invalid snapshots disconnect without mutating installed state.

`EditorWidget::status_observation()` delegates to `EditorStatus::observation()`, returning a `SduiStatusObservation` with:

- `status_text`: the exact GUI chrome status text.
- `connection_label`: the connection portion, such as `Connected` or `Local Fallback`.
- `access_label`: the access portion, such as `Editable`, `Read-only Observer`, or `No Server`.
- `sync_version`: the current confirmed document version when known.
- `diagnostic_text`: the active runtime diagnostic text when present.
- `theme_label`: compact active-theme package label (`default`, `theme-gruvbox-material-dark`, …) from the last installed `ActiveTheme` specifier.
- `dirty` / `document_display_name`: active-document dirty bit and basename-only title (never an absolute host path).
- `composing` / `pending_edit_count` / `recovery_summary`: IME composing flag, outbound pending-edit depth, and sanitized recovery/prompt text from an active transient menu or file/conflict diagnostic.

`EditorWidget::status_text()` reads from the same observation path, and `accessibility_label()` is composed by `src/editor/accessibility.rs` helpers so status, theme, composing, dirty, and recovery markers stay consistent for assistive tools and structural tests.

Clipboard copy/cut/paste are intentionally client-only and user-mediated. `EditorSurface::selected_text()` normalizes the current anchor/focus byte range and extracts only that UTF-8 rope slice through `EditorBuffer::text_range`. `EditorWidget::copy_selection_to_system_clipboard()` writes the returned text through `SystemClipboard` (`ClipboardSink` / text-only `arboard` in `src/client/clipboard.rs`). `SystemClipboard` keeps one backend in GUI-thread-local storage: X11 selection ownership otherwise disappeared when the temporary provider was dropped on hosts without a clipboard manager. Thread-local drop still runs arboard's shutdown/handoff when the UI thread exits. Cut reuses that write then deletes the selection through the ordinary local edit path. Paste reads with `ClipboardSink::get_text`, normalizes line endings, and inserts/replaces through `EditorSurface::paste_text_with_event`. Collapsed cut/copy are no-ops; empty paste text is a no-op. Clipboard failures become sanitized `client.clipboard.write_failed` / `client.clipboard.read_failed` diagnostics that never include full clipboard contents. OS clipboard work happens only on explicit cut/copy/paste commands, never during paint/layout/scroll or ordinary key insertion.

`arboard` remains direct by evidence rather than dependency inertia. Masonry 0.4 owns a private `copypasta::ClipboardContext`: `EventCtx::set_clipboard` can emit writes and masonry_winit intercepts native paste, but `DriverCtx` exposes no read fallback for bindable `clientPasteClipboard`. On Linux, copypasta 0.10's public `ClipboardContext` aliases X11 even when its Wayland module is built; native Wayland construction requires Masonry's raw display pointer, which Clay does not own. Replacing `arboard` would therefore lose the explicit command fallback or add unsafe/event-loop coupling. Clay disables arboard's unused image feature, removing its image codec dependency subtree while retaining text parity. The ignored live clipboard test runs only on an active desktop and restores prior text when available.

Undo and redo are client-local inverse operations built on the normal edit path. `EditHistory` (`src/editor/history.rs`) stores the last `EDIT_HISTORY_MAX_DEPTH` (256) local `EditOperation` entries. Every edit path — typing, selection replace, cut, paste, IME commit — captures `prior_text` and `selection_before` before buffer mutation via the centralized `apply_and_record_local_edit` helper, then records the inverse `EditOperation` (Insert → Delete of the inserted range, Delete → Insert of the original text, Replace → Replace back to prior text). Each inverse is an ordinary `Edit` transaction the server processes identically to forward edits; the server remains undo-unaware. `Ctrl+Z` (`clientUndo`) pops the most recent undo entry, applies it locally, and pushes its own inverse onto the redo stack. `Ctrl+Shift+Z` / `Ctrl+Y` (`clientRedo`) pops from the redo stack. The redo stack is cleared by any non-undo/redo local edit. Per-entry text is capped at 64 KiB (`EDIT_HISTORY_MAX_ENTRY_BYTES`); entries exceeding this limit clear both stacks to prevent unbounded memory growth. Undo and redo both guard on `DocumentAccess::is_editable()` and cancel any active IME composition before applying. History is per-document-surface and is stashed/restored alongside the surface during multi-document switching. Native shortcut matching uses `is_primary_character_shortcut` (primary modifier `Ctrl` on Linux/Windows, `Cmd` on macOS); redo shortcut is checked before undo shortcut to prevent `Ctrl+Shift+Z` from matching the undo branch.

IME composition is client-local and paint-only until commit. `CompositionState` (`src/editor/composition.rs`) stores preedit text and an optional byte-indexed cursor span on `EditorSurface`. `Ime::Preedit` updates the overlay without mutating the rope or enqueueing edits; empty preedit clears it. `Ime::Commit` clears composition and applies one ordinary insert/replace through the existing local-edit path. `Ime::Enabled` refreshes candidate-window geometry via Masonry `set_ime_area`; `Ime::Disabled`, window focus loss, pointer caret moves, undo/redo, cut/paste, and hard open/resync cancel unfinished composition without committing. Accessibility exposes a `Composing.` flag without including raw preedit text. Dirty/display-name/recovery markers share the same centralized helpers and AccessKit status child. Preedit updates never wait on IPC, server work, or package JavaScript.

## Code Examples

```rust
let mut widget = EditorWidget::with_initial_state(initial_state(
    DocumentAccess::Editable { lease_id: 99 },
    4,
));
widget.apply_connection_event(ClientConnectionEvent::EditAck {
    document_id: 7,
    version: 5,
    transaction_id: 1,
});
let observation = widget.status_observation();
assert_eq!(observation.connection_label, "Connected");
assert_eq!(observation.access_label, "Editable");
assert_eq!(observation.sync_version, Some(5));
```

## Invariants and Constraints

- `SduiStatusObservation` remains `pub(crate)` internal test/agent infrastructure, not a public Clay JS API.
- The observation is a pure `&self` read and allocates only the visible status strings it returns.
- Runtime diagnostic text must remain limited to sanitized protocol diagnostics; no source snippets, secrets, absolute paths, or server process internals are added by the GUI.
- Ordinary text input, IME preedit paint, and paint do not wait for IPC, server work, JavaScript, shell layout validation, clipboard work, or diagnostic processing.
- Clipboard authority is limited to explicit user cut/copy/paste commands on the client. Phase 20 does not invent package/configuration/AI clipboard-contents APIs or a server clipboard proxy.
- IME preedit is paint-only until commit; preedit updates never wait on IPC/server/JS; diagnostics and accessibility do not record raw composition strings beyond a composing flag / sanitized failure codes.
- Shell layout and pane/slot state do not grant packages native widget handles, raw CSS, raw ops, Vello/Parley callbacks, or client-side JavaScript authority over the editor component.
- The editor main region reserves the Clay-owned left file-browser slot by SDUI panel presence, not editor-binding match, so a freshly opened workspace file under a new document ID cannot overlap the file browser; the editor-only hidden workspace snapshot therefore reclaims the full width. `EditorSurface::paint_in_rect` fills the full editor rect with the editor background and paints no decorative accent circle or visible inset card; a small `TEXT_INSET` keeps text from hugging the edges.
- The main editor paints a slim vertical scrollbar indicator. `EditorSurface::scrollbar_thumb_rect` computes the thumb deterministically: hidden when content fits; thumb height proportional to viewport/content; thumb position tracks total document progress (`first_visible_line * line_height + visual_scroll_y` over `max_first * line_height`) so it advances smoothly across the whole document; for single-page content taller than the viewport (e.g. one wrapped line) it falls back to the visual-only budget. It is shared by paint and tests, never overlaps the file browser or status bar, and adds no second scroll model.
- Pixel scrolling is continuous: `scroll_vertical_pixels` accumulates a sub-line `visual_scroll_y` offset and advances the logical `first_visible_line` by one each time the shared `TypographyRegistry::document_line_height()` baseline is crossed, subtracting it (not resetting to zero). The baseline is the largest active monospace/proportional profile times Clay's one line-height multiplier, so mixed inline roles cannot underestimate logical progress. Visible Parley layout height and cursor rectangles remain source of truth for rendered overflow/caret placement. This avoids the backward jump that the old "exhaust overscan budget then reset" model produced; line/page deltas (`scroll_lines`) still snap to whole lines.
- `EditorSurface` keeps a one-shot `pin_caret_visible` flag. Caret movement sets it so the next paint can fine-tune sub-line scroll to keep the caret visible; explicit scroll clears it so `LayoutState::ensure_rect_visible` does not snap the view back to the caret after the user scrolls away.
- Syntax/semantic decorations resolve through `StyleRegistry::style_for` and become ranged Parley foreground brushes, so theme colors change glyph color rather than painting highlight rectangles behind text. The cached layout owns the brush table; selection and diagnostic squiggles remain separate native paint layers. Local insert/delete/replace operations synchronously interpolate retained inert syntax geometry: strict-interior edits resize it, narrow syntax (Keyword, Function, Type, Variable, Number) extends at token end only for same-word suffixes (every inserted character is a Unicode word character or `_`), broad comment/string/prose/code families inherit edge insertions unconditionally, and unsafe non-syntax overlap invalidates narrowly. This runs before asynchronous server work and bumps the existing layout-style revision only when painted spans change. Current versioned `DecorationSet` chunks replace only their declared half-open viewport: overlapping provisional spans from the same package/layer are split at authority boundaries, outside residuals survive and coalesce locally, and syntax replacement leaves semantic chunks intact.
- The editor accessibility root publishes the same profile-derived status rectangle and always includes the hosted `region`/`panel_host`/`overlay_host` child IDs in its AccessKit children, even when a region has no visible SDUI tree; this keeps Masonry's traversal and attached children consistent. SDUI/panel/overlay accessibility flows through the hosted Masonry widget subtree. Accessibility updates are requested only after accepted UI/profile/tree changes.

## Tests

- `src/masonry_editor.rs`: `status_observation_local_fallback_state` validates local/no-server observation fields.
- `src/masonry_editor.rs`: `status_observation_connected_editable_with_version` validates confirmed version and editable state after an edit acknowledgement.
- `src/masonry_editor.rs`: `status_observation_diagnostic_present_after_runtime_diagnostic_event` validates diagnostic forwarding into observable GUI chrome.
- `src/masonry_editor.rs`: `status_observation_does_not_regress_accessibility_label` validates consistency between status observation and accessibility text.
- `src/masonry_editor.rs`: `status_observation_exposes_active_theme_label` validates `theme_label` observability and accessibility `Theme …` marker after `ActiveTheme` install.
- `src/masonry_editor.rs`: dirty/display-name/recovery accessibility tests assert basename-only titles, dirty markers after open/local edit, and menu prompt recovery summaries.
- `src/masonry_editor.rs`: `document_saved_clears_dirty_and_keeps_status_chrome_clean`, `stale_save_conflict_keeps_dirty_and_opens_recovery_menu`, `dirty_reload_conflict_offers_save_first_and_keeps_local_text`, `document_reloaded_replaces_text_and_clears_dirty`, and `save_and_reload_command_intents_enqueue_protocol_file_messages` cover save/conflict chrome and protocol enqueue.
- `src/editor/accessibility.rs`: sanitize/compose helper unit tests plus deterministic virtual-node slot derivation used by editor, pane, shell, and menu status nodes.
- `src/masonry_sdui.rs`: active transient menus expose `Role::Menu` / `Role::MenuItem` accessibility entries.
- `src/editor/surface.rs`: `selected_text_returns_forward_backward_unicode_ranges` and `selected_text_returns_none_for_collapsed_selection` validate UTF-8 selection extraction.
- `src/masonry_editor.rs`: copy/cut/paste unit tests validate write/read/no-op/failure behavior with a fake sink, including cut-then-delete and paste insert/replace.
- `src/client/clipboard.rs`: fake/memory sink tests cover `set_text` / `get_text` without requiring a desktop clipboard.
- `src/editor/composition.rs` / `src/editor/surface.rs`: preedit does not change canonical text; empty preedit clears; load/undo cancel unfinished composition.
- `src/masonry_editor.rs`: accessibility composing flag omits raw preedit; commit-after-preedit inserts once; undo cancels composition.
- `src/masonry_sdui.rs`: `workspace_browser_reserves_left_slot_after_document_id_changes` validates the editor region still excludes the left slot after the active document ID changes; `ui_size_change_scales_row_hit_and_accessibility_bounds_together` locks shared UI geometry.
- `src/masonry_editor.rs`: `live_typography_update_requests_layout_render_and_accessibility` proves an UI-profile live update reaches SDUI and requests one native layout/accessibility refresh; Plan 086 consumer tests verify the editor status node remains attached and stable across redraws.
- `src/masonry_editor.rs`: `editor_pointer_hit_testing_uses_non_overlapping_editor_region_after_open` validates that clicks in the left file browser do not place a caret after a document opens.
- `src/editor/surface.rs`: `editor_surface_paint_has_no_decorative_accent_circle` and `editor_surface_uses_full_rect_background_without_visible_card_inset` source-guard the removed decorative chrome.
- `src/editor/surface.rs`: `scroll_after_caret_move_clears_caret_pin`, `scroll_vertical_pixels_advances_viewport_after_visual_budget`, `custom_typography_keeps_scrollbar_and_viewport_geometry_bounded`, and `empty_document_caret_uses_default_document_profile` validate bounded scroll, profile-driven geometry, reset, and placeholder-caret behavior.
- `src/editor/theme.rs`: theme registry unit tests validate Clay defaults, token/kind dispatch, modifier attributes, hex parsing, override merge, and theme text-attribute defaults.
- `src/editor/surface.rs`: `syntax_decoration_colors_are_distinct_by_token_family` locks the default per-token-family syntax color mapping through the registry; `src/editor/layout.rs::decoration_range_uses_a_non_default_text_brush` proves a syntax range receives a non-default Parley foreground brush.
- `src/editor/surface.rs`: `visible_caret_offset_returns_none_when_caret_above_viewport` locks the overflow guard when the caret is above the visible snapshot after scrolling.
- `src/masonry_editor.rs`: `client_installs_behavior_theme_typography_ui_and_render_generation_atomically`, `invalid_snapshot_installs_nothing_and_disconnects_without_ack`, `runtime_install_preserves_caret_selection_viewport_and_focus_status`, and `runtime_install_invalidates_layout_once` validate atomic runtime-generation install.
- Command: `cargo test -p clay --lib masonry_editor`

## Multi-document sessions (Phase 20)

`EditorWidget` keeps a bounded `DocumentSessionStore` of inactive documents. `DocumentOpened` for another `DocumentId` stashes the prior `EditorSurface` (text, caret/selection, viewport, history, dirty chrome) instead of destroying it. `show_open_documents_menu` / `activate_document` switch locally without re-downloading text. Shared presentation state is document-independent and must survive every surface replacement (`stash_active_session` / `activate_document` carry it explicitly): `StyleRegistry` theme, theme specifier, `ResolvedUiTheme` (scrollbar/chrome tokens), typography registry, behavior manifest, and the runtime caret-style override. Server open-registry/lease/dirty authority is unchanged. Ceiling: `CLIENT_DOCUMENT_SESSION_MAX` (64).

## Save / conflict persistence UX (Phase 20)

Local accepted edits mark `EditorStatus.dirty` optimistically. Configuration-owned keymaps are reapplied after package major-mode activation, so opening/classifying a file cannot discard the configured `Ctrl+S` chord. Bound `Ctrl+S` (`documents.serverSaveDocument`) is intercepted client-side and enqueued as `ClientMessage::SaveDocument` for the active document (never on the paint path). `DocumentSaved` updates dirty/version chrome; a clean save clears stale conflict diagnostics. `DocumentReloaded` replaces text like a same-document resync and clears dirty from metadata. `FileOperationFailed` with `StaleFileMetadata` or `DirtyDocument` keeps dirty text and opens a `TransientMenuSession` recovery menu: reload-from-disk (force), keep unsaved edits, compare later (stale save), or save-first / discard-and-reload / keep (dirty reload). Force-save overwrite is intentionally not offered; resolving a stale disk change requires an explicit reload or keeping local edits.

## Pending-edit / disconnect / resync recovery (Phase 20)

Outbound pending-edit depth is visible in status/accessibility via `SduiStatusObservation.pending_edit_count`. `EditRejected` updates sanitized diagnostics: auto-resync classes (stale/future/lease/read-only/region-lock/invalid behavior) note that resync was requested while the connection task continues auto-`RequestResync`; actionable `InvalidRange` / `InvalidDocument` and `ServerError` open Resync/Dismiss menus. Disconnect/connection-error events mark `Disconnected` with reconnect guidance (restart Clay) and a Dismiss menu without leaking host paths. Explicit commands `editor.clientRequestResync` and `editor.clientDismissRecovery` are bindable client UI routes. Successful `ResyncSnapshot` clears sync-recovery menus and diagnostics.

## Pixel / GPU snapshot stance (Phase 20)

Phase 20 revisited Masonry `TestHarness` / `assert_render_snapshot` and re-deferred pixel/GPU goldens: the harness hardcodes Vello `use_cpu: true`, so it does not validate Clay's production GPU path. Editor/SDUI regression stays on structural `SduiObservableSnapshot` / `SduiStatusObservation` (`decision-logs/2026-07-18-0352-phase20-pixel-snapshot-redeferral.md`).

## Phase 20.4: pointer-state feeding and token-driven status bar

Phase 20.4 (restyle-only) wired `EditorWidget::on_pointer_event` to feed interaction state into the editor chrome (SDUI interaction state moved to per-widget Masonry `EventCtx`/`QueryCtx` in Plan 070; the god-object `set_pointer_pos`/`set_pointer_pressed`/`set_focused_action` calls were deleted):

- **Editor chrome state.** `Down` sets `editor.set_pointer_pos` + `set_pointer_pressed(true)`; `Move` sets `pointer_pos`; `Up` clears `pointer_pressed` (keeps `pointer_pos` for hover persistence); `Cancel`/`Leave` call `clear_pointer_chrome_state()`. A press inside `scrollbar_thumb_rect` skips caret placement and pointer capture (`(false, true)` + repaint) so the thumb press does not start a text selection. `EditorSurface::scrollbar_interaction_state` (O(1) hit-test) drives `paint_scroll_chrome` (Rest/Disabled theme-provided alpha, Hover/Active/Focus +64 alpha boost).
- **Sidebar scroll routing.** `EditorWidget::on_pointer_event` checks `SduiNativeState::scrolls_point` to distinguish sidebar vs editor scroll and returns `(false, false)` for sidebar scroll events (the `SduiScrollViewport` child handles them via Masonry event bubbling; it does not call `set_handled`, so `EditorWidget` must skip re-handling).
- **Status bar insets.** `paint_status_line` reads `inset = editor.ui_theme().scalar_f64("spacing.sm") * spacing_scale()` (symmetric, preserves prior layout at default density) and paints a `paint_divider` hairline at the top; the hardcoded `12.0`/`24.0` insets were removed. `EditorSurface::ui_theme()` is a `pub(crate)` read-only accessor.

Plan 070 moved editor content from `post_paint` to `paint` (background fill + `EditorSurface::paint_in_rect` before the children pass) so overlays render above editor text; only `paint_status_line` remains in `post_paint`. All repaint call sites use `request_render` (not `request_paint_only`, which would skip `post_paint` status).

Caret/selection/diagnostics stay on `StyleRegistry` (`base.caret`/`base.selection`/`diagnostic_style`); no new `BaseUiColorKey`. See [Phase 20.4 Core Component Uplift](phase20.4-core-component-uplift-primitive-review.md) and [Server-Driven UI Protocol Schema](server-driven-ui.md).

## Plan 071: movement, multi-cursor, caret, text objects

Plan 071 extends the widget and surface with first-class editing primitives; see [Editor Movement, Selection, Caret, Ligatures, and Text Objects](editor-movement-selection-caret.md) for the full implementation page. Widget-level facts:

- Default key bindings for word/paragraph movement, line/word selection, multi-cursor (`Ctrl+Alt+Up/Down`, `Shift+Alt+arrows`, `Ctrl+D` select-next-match, `Ctrl+Shift+L`, `Ctrl+U` cursor undo) live in `EditorWidget::on_text_event` alongside the pre-existing arrow/home/end defaults; package `bindKey` manifests can override them.
- Direction-specific command IDs (`editor.clientMoveCursor.nextWordStart`, `...clientAddCursor.below`, textobject/smart-select IDs) dispatch client-locally through `EditorClientCommand::from_command_id` — no server round-trip; textobject/smart-select IDs instead enqueue a `SelectionQueryRequest` and apply the asynchronous result with stale-version guards (`pending_selection_query`).
- Caret blinking is the first Masonry animation-frame consumer: `on_anim_frame` advances the surface `CaretBlink` state machine while `ctx.request_anim_frame()` keeps the loop alive; only the primary caret blinks, secondaries paint solid.
- The selection set replaced the old single cursor+selection pair (`EditorSurface.selections: SelectionState`); copy/cut/paste, undo/redo, IME commit, and paint all iterate the set (edits apply right-to-left by byte offset).
- Escape priority chain: completion menu > snippet session > multi-selection cancel.

## Phase 22.2: EditorWidget as Connection Chrome

Phase 22.2 (2026-08-05) splits the widget: `EditorWidget` keeps everything that is **per-connection** — SDUI native state, `PackagePanelHost`/`PackageOverlayHost` children, shell-preferences, `RuntimeStateSnapshot` validation, the master `ClientEditQueue` (`edit_queue_shared()` hands out clones) — and delegates all **per-document** editing state to a `PaneDocumentView` child for pane 1 (other panes host their own views). See [Pane Document Views](pane-document-views.md) for the full model; widget-level facts:

- `EditorWidget` embeds its pane-1 view as a child pod (three-child z-order unchanged: panel_host, region, overlay_host; the view paints above region, below overlay_host — `paint_status_line` remains in `post_paint`).
- `apply_connection_event` keeps chrome-only events (SduiSnapshot/Update, ActiveTheme mirroring, shell prefs, runtime snapshots) and forwards document-scoped events to the view (unconditionally for `DocumentOpened`, so new documents reach the pane before it owns them).
- Widget trait entry points (`on_pointer_event`, `on_text_event`, `layout`, `paint`, focus handling) delegate to the view; `update` submits `EditorAction::PaneFocused(self.pane_id)` on child focus gain. `take_layout_invalidation` drains both the chrome's and the view's layout flags (short-circuit OR fixed to avoid dropped invalidations).
- `RuntimeBaseline` (behavior manifest + active theme + typography) is exposed to the driver so newly mounted pane views start from current runtime state.
- Test-facing shims: `view_mut()`/`editor_state_for_test()` accessors; the surviving unit tests reach the pane surface via `widget.view_mut().editor_mut()`.

## Phase 22.3: per-tab chrome and action surface

Phase 22.3 (2026-08-06) makes the widget one tab's connection chrome: each
`EditorWidget` instance is owned by one tab's `TabChrome` in
`ClayShellWidget` (the active tab's editor is the previous single-tab
widget; inactive tabs keep their chrome registered at zero size). The
action surface gained `EditorAction::TabBar(TabBarAction)` — `Activate {
client_id }`, `Close { client_id }`, `NewTab` — submitted by the shell's
pointer handling and handled by the driver (optimistic switch,
dirty-guarded close, folder-picker new-tab flow; see [Tabs and Independent
Client Views](tabs-and-clients.md)). `EditorAction` carries `DriverSession`
(a `PartialEq` wrapper over the client session comparing `client_id`) so
reconnect and open-tab events stay testable. Reconnect delegates
(`reconnect`, `documents_for_reopen`) proxy to the pane views.

## Related

- [Masonry Shell Runtime](masonry-shell.md)
- [Tabs and Independent Client Views](tabs-and-clients.md)
- [Pane Document Views](pane-document-views.md)
- [Client Snapshot Bootstrap](client-snapshot-bootstrap.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [End-to-End File Browser Workflow Primitive Review](end-to-end-file-browser-workflow-primitive-review.md)
- [Editor Theme Registry](editor-theme-registry.md)
- [Editor Movement, Selection, Caret, Ligatures, and Text Objects](editor-movement-selection-caret.md)
- [Range Diagnostics](range-diagnostics.md)
- [Client Copy Selection Clay JS API](../../reference/clay-js-api/editor/client-copy-selection.md)
- [Client Cut Selection Clay JS API](../../reference/clay-js-api/editor/client-cut-selection.md)
- [Client Paste Clipboard Clay JS API](../../reference/clay-js-api/editor/client-paste-clipboard.md)
- [`theme.setTheme`](../../reference/clay-js-api/theme/set-theme.md)
- [Package Authoring Guide](../../reference/packages/creating-packages.md) — Phase 20 package non-goals for multi-document / recovery chrome
- [File Open, Save, and Reload Workflow](../../development/file-open-save-reload-workflow.md)
- `src/client/mod.rs`
