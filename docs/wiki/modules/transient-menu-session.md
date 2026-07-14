# Transient Menu Session

## Source

- `src/shell/transient_menu.rs`
- `src/shell/mod.rs`
- `plans/036-Phase18.8-Bottom-Pane-Transient-Menu-and-Command-Execution-Foundation.md`
- `docs/reference/primitives/registry.md#TransientMenuSession`
- `docs/reference/primitives/shell-layout-strategy.md`

## Overview

`TransientMenuSession` is the Clay-owned typed state model for bottom-pane transient menus. It is intentionally generic: command palettes, completion pickers, file search, symbol search, Git pickers, and package-provided quick-pick workflows can all reuse the same session shape. Control Center is the first consumer, not the only consumer, and the session contains no Control Center-specific fields.

The session stores prompt text, query text, a bounded item list, selection index, status text, focus policy, accessibility labels, and inert activation actions. It performs local bounded filtering and selection movement without package JavaScript, IPC round trips, or command side effects. Activation produces an inert `TransientMenuAction` carrying a command ID and bounded JSON arguments; callers turn that action into a server-owned `CommandExecutionRequest` through the Phase 18.8 `CommandExecutor`.

## How It Works

`TransientMenuSessionId` is a stable numeric session identifier. A new session starts with a prompt, an empty query, no items, selection at zero, and an `Empty` status. Items are supplied through `with_items`, which caps the list at `MAX_ITEMS` (256), resets selection to zero, and sets `Active` status when items exist.

`TransientMenuItem` holds a display label, optional detail text, accessibility label, provenance (`BuiltIn` or `Package { name, version }`), and an inert `TransientMenuAction`. Item labels are capped at `MAX_LABEL_CHARS` (128), details at `MAX_DETAIL_CHARS` (256), and accessibility labels at `MAX_ACCESSIBILITY_LABEL_CHARS` (256). Command-palette actions carry only a command ID plus bounded JSON arguments. Completion actions carry only `CompletionMenuAcceptAction` text-replacement metadata: request/document/version IDs, replacement range, insert text, and commit characters. No item carries callbacks, native handles, raw CSS, raw op names, or executable code.

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

Packages reach a transient menu only through server-owned workflows such as the Control Center, and package commands become menu items only by being registered through `clay.commands.serverRegisterCommand`; packages cannot open, populate, or drive a transient menu session directly.

## Integration with Shell, Control Center, and Command Execution

`TransientMenuSession` lives in `src/shell/transient_menu.rs` and is declared in `src/shell/mod.rs`. It does not render itself. Phase 18.8 Task 6 projects the session onto existing shell transient-overlay and component primitives through `TransientPackageOverlay::from_menu_session` in `src/shell/package_ui.rs`. The projection creates a bottom-anchored overlay with a `stack` root containing prompt/query labels, an empty-status `statusItem`, or a `list` of selectable items. The selected item is marked so Masonry renders a highlight. The overlay is anchored to the bottom of the main editor pane and does not consume fixed-slot geometry, so editor region and caret hit-testing remain unchanged while the menu is visible.

`SduiNativeState` stores an optional active menu and includes it in overlay observation and paint. `EditorWidget::local_key` in `src/masonry_editor.rs` routes ArrowUp/ArrowDown/Enter/Escape to the active menu: arrow keys move selection locally, Enter enqueues a server-first command intent for the selected item's inert action, and Escape cancels and clears the menu. Editor command routing resumes when no menu is active. No package JavaScript, command execution, or IPC round-trip runs inside Masonry paint/layout/pointer/key/text handlers.

Activation from the overlay or from keyboard handlers produces a `TransientMenuAction`. Command actions are normalized into a `CommandExecutionRequest` and routed through `CommandExecutor` from `src/server/command_execution.rs`; for command menu items, the activation path routes it through `CommandExecutor`. Completion actions never enter command execution: `SduiNativeState::menu_activate_completion` returns the inert accept payload to `EditorWidget`, which calls `EditorSurface::accept_completion_with_event` to validate document/version/behavior/range metadata and produce a local text replacement edit for the active document only. `TransientPackageOverlay::from_menu_session` deliberately omits command action targets for completion items, so pointer/action routing cannot turn a completion item into a command.

## Invariants and Constraints

- Sessions are generic UI state, not Control Center-specific state.
- Item lists are bounded; oversized inputs are truncated or capped.
- Query/filtering is local bounded metadata work; no package JavaScript, IPC wait, or command side effects happen inside selection movement.
- Items carry only inert command IDs and bounded JSON arguments, or inert completion text-replacement payloads (plain text or inert LSP snippet syntax expanded client-local; no executable transforms).
- No callbacks, raw ops, native handles, raw CSS, renderer callbacks, filesystem paths beyond sanitized labels, credentials, commands hidden inside completion items, or executable package code may be stored in session items.
- `TransientMenuSession` does not own rendering, focus restoration, or command execution semantics; those belong to the overlay renderer and `CommandExecutor`.

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
- `src/shell/package_ui.rs`: `completion_menu_projection_has_no_command_action_targets`
- `src/masonry_sdui.rs`: `active_menu_appears_in_overlay_observation`
- `src/masonry_sdui.rs`: `cancelled_menu_does_not_appear_in_overlay_observation`
- `src/masonry_sdui.rs`: `menu_overlay_does_not_change_editor_region`
- `src/masonry_sdui.rs`: `menu_navigation_updates_selection`
- `src/masonry_sdui.rs`: `menu_activate_selected_returns_inert_action_intent`
- `src/masonry_editor.rs`: `completion_result_installs_bottom_transient_menu_for_active_request`
- `src/masonry_editor.rs`: `stale_completion_result_is_ignored_after_newer_request`
- `src/editor/surface.rs`: `editor_accepts_completion_as_local_replacement`
- `src/editor/surface.rs`: `editor_accepts_snippet_as_local_expansion_and_selects_first_placeholder` (Phase 18.19)
- `src/editor/surface.rs`: `snippet_tab_navigation_moves_forward_backward_and_ends_at_final_tabstop` (Phase 18.19)
- `src/editor/surface.rs`: `editing_active_placeholder_shifts_later_snippet_ranges` (Phase 18.19)
- `src/shell/transient_menu.rs`: `completion_result_projects_snippet_text_format_to_menu_accept_action` (Phase 18.19)
- `src/shell/transient_menu.rs`: `cancelled_session_rejects_activation`
- `src/shell/transient_menu.rs`: `item_detail_and_accessibility_budgets_are_enforced`
- `src/shell/transient_menu.rs`: `item_action_is_inert_command_intent_only`

Run with:

```text
CARGO_TARGET_DIR=target/pi-verify cargo test --lib transient_menu --quiet
CARGO_TARGET_DIR=target/pi-verify cargo test --lib shell --quiet
CARGO_TARGET_DIR=target/pi-verify cargo test --lib masonry_sdui --quiet
```

## Related

- [Command Registry](command-registry.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [Phase 18.8 Transient Menu and Command Execution Primitive Review](phase18.8-transient-menu-command-execution-primitive-review.md)
- [Completion Snippet Expansion](completion-snippet-expansion.md) — Phase 18.19 snippet accept path and session
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
