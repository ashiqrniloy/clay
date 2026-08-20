# Transient Menu Session

## Source

- `src/shell/transient_menu.rs`
- `src/shell/mod.rs`
- `plans/036-Phase18.8-Bottom-Pane-Transient-Menu-and-Command-Execution-Foundation.md`
- `docs/reference/primitives/registry.md#TransientMenuSession`
- `docs/reference/primitives/shell-layout-strategy.md`

## Overview

`TransientMenuSession` is the Clay-owned typed state model for bottom-pane transient menus. It is intentionally generic: command palettes, completion pickers, file search, symbol search, Git pickers, and package-provided quick-pick workflows can all reuse the same session shape. Control Center is the first consumer, not the only consumer, and the session contains no Control Center-specific fields.

The session stores prompt text, query text, a bounded item list, selection index, status text, focus policy, accessibility labels, and inert activation actions. It performs local bounded filtering and selection movement without package JavaScript, IPC round trips, or command side effects. Activation produces an inert `TransientMenuAction` carrying a command ID and bounded JSON arguments; callers turn that action into a server-owned `CommandExecutionRequest` through the Phase 18.8 `CommandExecutor`. Plan 087's completion path adds only an internal `Completion` origin and fixed-point caret anchor; it does not widen package menu APIs or move completion authority out of the pane/client path.

## How It Works

`TransientMenuSessionId` is a stable numeric session identifier. A new session starts with a prompt, an empty query, no items, selection at zero, and an `Empty` status. Items are supplied through `with_items`, which caps the list at `MAX_ITEMS` (256), resets selection to zero, and sets `Active` status when items exist.

`TransientMenuItem` holds a display label, optional detail text, accessibility label, provenance (`BuiltIn` or `Package { name, version }`), and an inert `TransientMenuAction`. Item labels are capped at `MAX_LABEL_CHARS` (128), details at `MAX_DETAIL_CHARS` (256), and accessibility labels at `MAX_ACCESSIBILITY_LABEL_CHARS` (256). Before hosted Masonry projection, `compose_menu_item_accessibility_label` removes controls/path separators, falls back from an invalid accessibility label to the display label and then `Menu item`, and keeps the selected suffix inside the 256-character ceiling; display/action fields remain unchanged. Command-palette actions carry only a command ID plus bounded JSON arguments. Completion actions carry only `CompletionMenuAcceptAction` text-replacement metadata: request/document/version IDs, replacement range, insert text, and commit characters. No item carries callbacks, native handles, raw CSS, raw op names, or executable code.

`update_query` replaces the query text (capped at `MAX_QUERY_CHARS` / 256) and resets selection to zero. The session does not automatically re-filter its backing list; callers (such as a future Control Center builder) own the filtering policy and call `with_items` with a freshly filtered snapshot. This keeps the session state simple, deterministic, and bounded.

Selection movement uses `select_next` and `select_previous`, which wrap at list boundaries. `activate_selected` returns the action of the selected item, or `None` when the list is empty. `cancel` sets `Cancelled` status; `is_active` returns false only when cancelled.

`TransientMenuFocusPolicy` is either `Modal` or `Modeless`. The default is `Modal`, matching the focused command-palette pattern. Future HUD-style pickers can switch to `Modeless` without changing the session model.

## Code Example

These types are `pub(crate)` and are not part of the public Clay JS API surface. Packages and external code never import or construct `TransientMenuSession` directly; only Clay-owned server/shell code builds sessions. The snippet below illustrates the internal usage shape only:

```rust
// src/shell/transient_menu.rs (crate-internal types)
use crate::shell::transient_menu::{
    TransientMenuAction, TransientMenuFocusPolicy, TransientMenuItem,
    TransientMenuItemProvenance, TransientMenuSession, TransientMenuSessionId,
};

let session = TransientMenuSession::new(TransientMenuSessionId(1), "Control Center")
    .with_focus_policy(TransientMenuFocusPolicy::Modal)
    .with_items(vec![
        TransientMenuItem::new(
            "open-recent",
            "Open Recent Document",
            TransientMenuAction::new("document.open_recent"),
        )
        .with_detail("Reopen a recently closed document")
        .with_accessibility_label("Open recent document"),
        TransientMenuItem::new(
            "markdown-preview",
            "Toggle Markdown Preview",
            TransientMenuAction::new("markdown.togglePreview"),
        )
        .with_package_provenance("@clay/markdown", "1.0.0"),
    ]);
```

Packages reach a transient menu only through server-owned workflows such as the Control Center, and package commands become menu items only by being registered through `commands.serverRegisterCommand`; packages cannot open, populate, or drive a transient menu session directly.

## Integration with Shell, Control Center, and Command Execution

`TransientMenuSession` lives in `src/shell/transient_menu.rs` and is declared in `src/shell/mod.rs`. It does not render itself. Phase 18.8 Task 6 projects the session onto existing shell transient-overlay and component primitives through `TransientPackageOverlay::from_menu_session` in `src/shell/package_ui.rs`. The projection creates a bottom-anchored overlay with a `stack` root containing prompt/query labels, an empty-status `statusItem`, or a `list` of selectable items. The selected item is marked so Masonry renders a highlight. The overlay is anchored to the bottom of the main editor pane and does not consume fixed-slot geometry, so editor region and caret hit-testing remain unchanged while the menu is visible.

`SduiNativeState` stores an optional active menu and includes it in overlay observation and paint. `PaneDocumentView::local_key` in `src/masonry_pane_document.rs` runs `route_menu_key` before editor key routing: arrow keys move selection locally, Enter/Tab enqueues a server-first command intent for the selected item's inert action (completion items produce a local accept edit instead), and Escape cancels and clears the menu. Editor command routing resumes when no menu is active. No package JavaScript, command execution, or IPC round-trip runs inside Masonry paint/layout/pointer/key/text handlers.

Activation from the overlay or from keyboard handlers produces a `TransientMenuAction`. Command actions are normalized into a `CommandExecutionRequest` and routed through `CommandExecutor` from `src/server/command_execution.rs`; for command menu items, the activation path routes it through `CommandExecutor`. Completion actions never enter command execution: `SduiNativeState::menu_activate_completion` returns the inert accept payload to `EditorWidget`, which calls `EditorSurface::accept_completion_with_event` to validate document/version/behavior/range metadata and produce a local text replacement edit for the active document only. `TransientPackageOverlay::from_menu_session` deliberately omits command action targets for completion items, so pointer/action routing cannot turn a completion item into a command.

## Language-intelligence projection

Phase 18.20's `language_intelligence_result_to_menu_session` adapter keeps intelligence UI on this generic primitive. Hover and signature help become modeless inert-text sessions; definitions and code actions become modal selectable sessions. Definition actions carry validated document/root/path/byte-offset metadata, command-backed code actions carry registered command IDs, and edit-only actions carry display-only preview metadata. Markdown is reduced to inert plain text. Empty, timeout, and provider-error statuses remain bounded session status text. `EditorWidget` rejects stale request IDs before installing any session.

## Invariants and Constraints

- Sessions are generic UI state, not Control Center-specific state.
- Item lists are bounded; oversized inputs are truncated or capped.
- Query/filtering is local bounded metadata work; no package JavaScript, IPC wait, or command side effects happen inside selection movement.
- Items carry only inert command IDs and bounded JSON arguments, or inert completion text-replacement payloads (plain text or inert LSP snippet syntax expanded client-local; no executable transforms).
- No callbacks, raw ops, native handles, raw CSS, renderer callbacks, filesystem paths beyond sanitized labels, credentials, commands hidden inside completion items, or executable package code may be stored in session items.
- `TransientMenuSession` does not own rendering, focus restoration, or command execution semantics; those belong to the overlay renderer and `CommandExecutor`.

## Phase 20.5: Surface Origin

Phase 20.5 added `TransientMenuOrigin` (`src/shell/transient_menu.rs`) to distinguish the surface that spawned a session:

| Origin | Anchor | Focus policy default | Use case |
|--------|--------|---------------------|----------|
| `CommandPalette` | `Bottom` | `Modal` | Command palette |
| `Completion` | caret-adjacent | `Modeless` | Clay-native completion picker |
| `ContextMenu` | `Pointer` | `Modeless` | Right-click context menu |
| `MenuBar` | `Main` | `Modeless` | Menu bar dropdown |

`TransientMenuSession` gains an `origin: TransientMenuOrigin` field (default `CommandPalette`), a `with_origin()` builder, and an `origin()` accessor. `TransientPackageOverlay::from_menu_session` (`src/shell/package_ui.rs`) reads `session.origin()` to select the overlay anchor instead of hardcoding `Bottom`. Keyboard navigation (`route_menu_key` in `src/masonry_pane_document.rs`) is unchanged — ArrowUp/Down, Enter/Tab, Escape, and commit characters apply to all origins.

## Phase 24.1: Server-Owned Sessions

24.1 adds an additive second class: **server-owned** interactive sessions (the Command Centre round trip; see [Transient Menu Round Trip](transient-menu-round-trip.md)). The server owns the session and pushes bounded snapshots; the client renders and forwards keystrokes only. The session shell gained three `pub(crate)` builders for this:

- `from_snapshot_data(data)` — hydrates an inert wire DTO (`TransientMenuSnapshotData`) into a session: `new(session_id, prompt)` → `with_items` → `with_query` → `with_focus_policy` → `with_origin` → `with_empty_status` (wire `Empty`) → `with_selected_index`. Items are inert (action = `TransientMenuAction::new(id)`), no provenance (not on the wire).
- `with_query(query)` — truncates to `TRANSIENT_MENU_MAX_QUERY_CHARS`; no status/selection side effects (unlike `update_query`).
- `with_selected_index(index)` — restores a persisted selection, clamped to `items.len().saturating_sub(1)`, empty list maps to 0.

Client ownership lives in `PaneMenuSync` (`src/masonry_pane_document.rs`): `server_owned: bool` plus `server_query_buffer: String` (a send-buffer mirroring what was sent; visuals update only from pushed snapshots — no optimistic echo). `route_menu_key` dispatches on ownership: server-owned keys go through the pure `dispatch_server_menu_key` (printable → `MenuQueryUpdate`, Backspace → `MenuBackspace`, arrows → `MenuSelectionMove ±1`, Enter/Tab → `MenuActivate` kind `Primary`, Alt+Enter → kind `Secondary`, Escape → `MenuCancel`), never mutating local selection/query; Backspace is intercepted in `handle_text_event`'s `NamedKey::Backspace` arm, which otherwise never reaches `route_menu_key`. A local menu opening while a server session is active enqueues `MenuCancel` first (`push_menu` hook) — one active menu per pane in both directions.

## Phase 24.2: Shared Fuzzy Query Scoring

The session itself never ranks items — filtering policy stays with the caller. Phase 24.2 centralizes that policy in one shared bounded scorer, [Fuzzy Matching](fuzzy-matching.md) (`src/shell/fuzzy.rs`): `ControlCenter::session` scores every catalogue item against the query (score descending, then label, then ID; source order for an empty query) and `FileBrowserState::fuzzy_session` scores entry names, both replacing per-caller substring filters. Query and candidate inputs are Unicode-aware and capped (`MAX_INPUT_CHARS` 256); the scan is bounded per query and never re-consults registries or runs package JS.

## Phase 24.3: Path Browser projection

The [Path Browser](path-browser.md) is the second server-owned session kind and the first that treats the query line as an editable path bar: the session derives a filter fragment from the input (split at the last platform separator), scores its **installed** bounded entries with the same shared scorer, and projects prompt `Browse · {canonical_dir}` / query = input / inert empty-string actions through the identical builder chain (`with_items`/`with_selected_index`/`with_empty_status`). Filter-only edits never touch the filesystem — only directory-prefix changes relist.

## Phase 24.4: centered dialog accessibility and containment

Command Centre command/path snapshots use `TransientMenuOrigin::Centered`.
`TransientPackageOverlay::from_menu_session` carries the sanitized prompt,
selected item labels, and a bounded result-count string to the retained
`PackageRegionWidget`. The window-level `PackageOverlayHost` is the modal
`Role::Dialog`; its child region is the `Role::Menu` with `Role::MenuItem`
children and one stable polite `Role::Status` count node.

Masonry focus stays on the originating pane. `PaneDocumentView` routes
server-owned modal keys through the existing intent queue and consumes unknown
keys, queue failures, clipboard paste, and IME events instead of allowing
editor mutation. The centered root layer swallows scrim pointer events. Query
and selection snapshots reconcile the same root/region and synthetic AccessKit
IDs; selection changes with unchanged count do not re-announce.

## Plan 087: caret-adjacent completion projection

Completion results use `TransientMenuOrigin::Completion` and store pane-local
caret/IME bounds in `CompletionAnchor` as fixed-point coordinates. The session
is still an inert bounded item model: `PaneDocumentView::apply_completion_result`
checks the active request and document/version/behavior stamps first, dismisses
empty or stale results before overlay reconciliation, and reports timeout/error
as non-blocking `RuntimeDiagnostic` status text. Non-empty results carry the
anchor through `EditorWidget` to `PackageOverlayHost`.

`completion_overlay_rect` in `src/shell/package_ui.rs` is the single geometry
helper. It clamps the popup to the active pane, prefers below-caret placement
then above-caret placement, limits width to 480 logical pixels, and limits the
visible list to eight rows. The shared retained `SduiScrollViewport` wraps menu
lists (including centered Command Centre lists), and the selected row supplies a
bounded scroll target during reconciliation. Completion items have no command
action targets; their existing local accept payload remains the only activation
path. Centered command/path sessions retain their centered modal layer and
focus-restoration behavior.

## Tests

- `src/shell/transient_menu.rs`: `session_stores_prompt_and_starts_empty`
- `src/shell/transient_menu.rs`: `with_items_bounds_count_and_resets_selection`
- `src/shell/transient_menu.rs`: `query_update_truncates_and_resets_selection`
- `src/shell/transient_menu.rs`: `selection_wraps_at_boundaries`
- `src/shell/transient_menu.rs`: `empty_session_selection_is_no_op`
- `src/shell/transient_menu.rs`: `activate_selected_returns_action`
- `src/shell/transient_menu.rs`: `cancel_marks_session_inactive`
- `src/shell/transient_menu.rs`: `item_labels_and_details_are_truncated`
- `src/shell/transient_menu.rs`: `package_provenance_is_stored`
- `src/shell/transient_menu.rs`: `item_uses_accessibility_label_when_set`
- `src/shell/transient_menu.rs`: `focus_policy_can_be_modeless`
- `src/shell/transient_menu.rs`: `completion_result_projects_to_transient_menu_session`
- `src/shell/transient_menu.rs`: `completion_error_status_projects_to_empty_menu_status`
- `src/shell/package_ui.rs`: `menu_session_projects_to_bottom_transient_overlay`
- `src/shell/package_ui.rs`: `bottom_menu_overlay_does_not_consume_fixed_slot_geometry`
- `src/shell/package_ui.rs`: `empty_menu_session_shows_status_without_action_targets`
- `src/shell/package_ui.rs`: `completion_menu_projection_has_no_command_action_targets`, `completion_overlay_clamps_above_or_below_caret_inside_main_rect`
- `src/masonry_sdui.rs`: `active_menu_appears_in_overlay_observation`, `completion_menu_observation_uses_caret_bounded_geometry`
- `src/masonry_sdui.rs`: `cancelled_menu_does_not_appear_in_overlay_observation`
- `src/masonry_sdui.rs`: `menu_overlay_does_not_change_editor_region`
- `src/masonry_sdui.rs`: `menu_navigation_updates_selection`
- `src/masonry_sdui.rs`: `menu_activate_selected_returns_inert_action_intent`
- `src/masonry_editor.rs`: `completion_result_installs_bottom_transient_menu_for_active_request`
- `src/masonry_editor.rs`: `stale_completion_result_is_ignored_after_newer_request`
- `src/masonry_pane_document.rs`: `empty_completion_result_dismisses_current_overlay`, `completion_provider_failure_uses_status_without_overlay`, `stale_completion_result_closes_matching_current_menu`, and `server_menu_typing_sends_exactly_one_query_update_with_the_full_buffer`
- `src/masonry_pane_document.rs`: `server_menu_arrows_send_selection_moves_without_local_mutation`
- `src/masonry_pane_document.rs`: `server_menu_enter_and_escape_send_activate_and_cancel`
- `src/masonry_pane_document.rs`: `server_menu_snapshot_hydration_preserves_display_fields`
- `src/masonry_pane_document.rs`: `server_menu_closed_clears_only_the_matching_session`
- `src/masonry_pane_document.rs`: `server_menu_snapshot_replaces_and_resyncs_query_buffer`
- `src/masonry_package_region.rs`: `menu_selection_keeps_selected_row_in_scroll_viewport`, `centered_command_center_scrolls_60_results_without_overflow`, `package_menu_accessibility_labels_are_sanitized_bounded_and_consumer_valid`
- `src/masonry_pane_document.rs`: `local_menu_open_cancels_the_active_server_session`
- `src/masonry_pane_document.rs`: `menu_sync_pending_semantics` (2-arg `push` + `push_server`)
- `src/editor/surface/mod.rs`: `editor_accepts_completion_as_local_replacement`
- `src/editor/surface/mod.rs`: `editor_accepts_snippet_as_local_expansion_and_selects_first_placeholder` (Phase 18.19)
- `src/editor/surface/mod.rs`: `snippet_tab_navigation_moves_forward_backward_and_ends_at_final_tabstop` (Phase 18.19)
- `src/editor/surface/mod.rs`: `editing_active_placeholder_shifts_later_snippet_ranges` (Phase 18.19)
- `src/shell/transient_menu.rs`: `completion_result_projects_snippet_text_format_to_menu_accept_action` (Phase 18.19)
- `src/shell/transient_menu.rs`: `cancelled_session_rejects_activation`
- `src/editor/accessibility.rs`: `menu_item_accessibility_labels_are_safe_and_bounded`
- `src/shell/transient_menu.rs`: `item_detail_and_accessibility_budgets_are_enforced`
- `src/shell/transient_menu.rs`: `item_action_is_inert_command_intent_only`

Run with:

```text
cargo test --lib transient_menu --quiet
cargo test --lib shell --quiet
cargo test --lib masonry_sdui --quiet
cargo test --lib masonry_pane_document --quiet
```

## Related

- [Command Registry](command-registry.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [Phase 18.8 Transient Menu and Command Execution Primitive Review](phase18.8-transient-menu-command-execution-primitive-review.md)
- [Completion Snippet Expansion](completion-snippet-expansion.md) — Phase 18.19 snippet accept path and session
- [Language Intelligence](language-intelligence.md) — Phase 18.20 hover/signature/definition/code-action projection
- [Transient Menu Round Trip](transient-menu-round-trip.md) — Phase 24.1/24.2 server-owned sessions: protocol, store, lifecycle, client routing
- [Fuzzy Matching](fuzzy-matching.md) — the shared bounded query scorer (Phase 24.2)
- [Phase 20.5 Overlay, Menu, and Input Components](phase20.5-overlay-menu-input-components.md) — `TransientMenuOrigin`, z-level stacking, new component kinds
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
- [Repeatable UI Review Harness](ui-review-harness.md) — plan 087 fixture/capture workflow exercising completion and centered menus live
