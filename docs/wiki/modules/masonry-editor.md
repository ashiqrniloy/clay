# Masonry Editor Widget Status Observability

## Source

- `src/masonry_editor.rs`
- `src/masonry_shell.rs`
- `src/client/mod.rs`
- `src/client/clipboard.rs`
- `src/editor/surface.rs`
- `src/main.rs`
- `runtime/js/editor.ts`
- `docs/reference/clay-js-api/editor/client-copy-selection.md`

## Overview

`EditorWidget` composes the native editor surface, server-driven UI overlay, and bottom status chrome. The status chrome reflects connection state, document access, confirmed sync version, and the latest sanitized runtime diagnostic forwarded by `ClientConnectionEvent::RuntimeDiagnostic`.

After Phase 18.2, `EditorWidget` is no longer the top-level application layout. `src/masonry_shell.rs::ClayShellWidget` owns the Masonry root and working-area geometry, registers `EditorWidget` as the shell's editor child, and routes focus/action handling back to that child. `EditorWidget` remains responsible for local text input, caret/selection/viewport state, explicit selection-copy clipboard writes, edit queue emission, SDUI event application/rendering, status chrome, and accessibility.

Phase 15 adds `SduiStatusObservation`, a `pub(crate)` headless observability struct for tests and internal agent inspection. It is not a Clay JS API surface; it only exposes strings and version metadata already visible in GUI chrome.

## Responsibilities

- Apply `ClientConnectionEvent` values on the GUI thread and update editor, SDUI, or status state without blocking paint/input paths.
- Act as the shell-owned editor component under `ClayShellWidget`; it is not responsible for working-area, split-tree, or pane-slot ownership.
- Keep focus/action routed events client-first and editor-local after the shell forwards them to the registered editor child.
- Render and expose accessible status text for connection, access, document, version, and runtime diagnostics.
- Copy the current non-empty editor selection to the OS clipboard on explicit native copy shortcuts (`Ctrl+C` on Linux/Windows, `Cmd+C` on macOS) using the client-owned `src/client/clipboard.rs` wrapper.
- Provide `EditorWidget::status_observation()` so tests can assert status chrome state without opening a window or painting.
- Keep diagnostics sanitized by displaying only the `RuntimeDiagnostic` code/message supplied by the server protocol or client clipboard wrapper.

## How It Works

`EditorStatus` stores the current `EditorConnectionStatus`, optional document ID, optional confirmed document version, optional `DocumentAccess`, and optional `RuntimeDiagnostic`. Small label helpers derive the user-visible connection, access, document, version, and diagnostic strings. `EditorStatus::text()` builds the exact status line painted by the widget.

`EditorWidget::status_observation()` delegates to `EditorStatus::observation()`, returning a `SduiStatusObservation` with:

- `status_text`: the exact GUI chrome status text.
- `connection_label`: the connection portion, such as `Connected` or `Local Fallback`.
- `access_label`: the access portion, such as `Editable`, `Read-only Observer`, or `No Server`.
- `sync_version`: the current confirmed document version when known.
- `diagnostic_text`: the active runtime diagnostic text when present.

`EditorWidget::status_text()` reads from the same observation path, and `accessibility_label()` includes that status text, so tests do not need to parse a separate accessibility string to inspect status fields.

Selection copy is intentionally client-only. `EditorSurface::selected_text()` normalizes the current anchor/focus byte range and extracts only that UTF-8 rope slice through `EditorBuffer::text_range`. `EditorWidget::copy_selection_to_system_clipboard()` writes the returned text through `SystemClipboard`, a tiny `arboard` wrapper in `src/client/clipboard.rs`. Collapsed selections return `None` and do nothing. Clipboard failures become `clay.client.clipboard.write_failed` diagnostics; no edit event, server message, JavaScript execution, filesystem work, clipboard read, paste, or cut path is involved.

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
- Ordinary text input and paint do not wait for IPC, server work, JavaScript, shell layout validation, clipboard work, or diagnostic processing.
- Clipboard authority is write-only for the current editor selection after an explicit user copy shortcut. Server code, packages, and configuration cannot read clipboard contents or set arbitrary clipboard text.
- Shell layout and pane/slot state do not grant packages native widget handles, raw CSS, raw ops, Vello/Parley callbacks, or client-side JavaScript authority over the editor component.
- The editor main region reserves the Clay-owned left file-browser slot by SDUI panel presence, not editor-binding match, so a freshly opened workspace file under a new document ID cannot overlap the file browser. `EditorSurface::paint_in_rect` fills the full editor rect with the editor background and paints no decorative accent circle or visible inset card; a small `TEXT_INSET` keeps text from hugging the edges.
- The main editor paints a slim vertical scrollbar indicator. `EditorSurface::scrollbar_thumb_rect` computes the thumb deterministically: hidden when content fits; thumb height proportional to viewport/content; thumb position tracks total document progress (`first_visible_line * line_height + visual_scroll_y` over `max_first * line_height`) so it advances smoothly across the whole document; for single-page content taller than the viewport (e.g. one wrapped line) it falls back to the visual-only budget. It is shared by paint and tests, never overlaps the file browser or status bar, and adds no second scroll model.
- Pixel scrolling is continuous: `scroll_vertical_pixels` accumulates a sub-line `visual_scroll_y` offset and advances the logical `first_visible_line` by one each time a full `line_height` is crossed, subtracting `line_height` (not resetting to zero). This avoids the backward jump that the old "exhaust overscan budget then reset" model produced, and the view never snaps back to the top at the end of the document. Line/page deltas (`scroll_lines`) still snap to whole lines.
- `EditorSurface` keeps a one-shot `pin_caret_visible` flag. Caret movement sets it so the next paint can fine-tune sub-line scroll to keep the caret visible; explicit scroll clears it so `LayoutState::ensure_rect_visible` does not snap the view back to the caret after the user scrolls away.
- Syntax decorations from tree-sitter parses are rendered as background tints behind text. `decoration_color` maps token families (`keyword`, `string`, `comment`, `punctuation`, and `markup`) to distinct, visible colors instead of a single faint tint so Rust/TypeScript/JavaScript/Markdown highlighting is perceivable.

## Tests

- `src/masonry_editor.rs`: `status_observation_local_fallback_state` validates local/no-server observation fields.
- `src/masonry_editor.rs`: `status_observation_connected_editable_with_version` validates confirmed version and editable state after an edit acknowledgement.
- `src/masonry_editor.rs`: `status_observation_diagnostic_present_after_runtime_diagnostic_event` validates diagnostic forwarding into observable GUI chrome.
- `src/masonry_editor.rs`: `status_observation_does_not_regress_accessibility_label` validates consistency between status observation and accessibility text.
- `src/editor/surface.rs`: `selected_text_returns_forward_backward_unicode_ranges` and `selected_text_returns_none_for_collapsed_selection` validate UTF-8 selection extraction.
- `src/masonry_editor.rs`: `copy_selection_writes_selected_text_without_edit_event`, `copy_selection_is_noop_when_selection_is_collapsed`, and `copy_selection_failure_reports_runtime_diagnostic` validate clipboard write/no-op/failure behavior with a fake sink.
- `src/client/clipboard.rs`: `clipboard_sink_accepts_utf8_text` validates the test sink contract without requiring a desktop clipboard.
- `src/masonry_sdui.rs`: `workspace_browser_reserves_left_slot_after_document_id_changes` validates the editor region still excludes the left slot after the active document ID changes.
- `src/masonry_editor.rs`: `editor_pointer_hit_testing_uses_non_overlapping_editor_region_after_open` validates that clicks in the left file browser do not place a caret after a document opens.
- `src/editor/surface.rs`: `editor_surface_paint_has_no_decorative_accent_circle` and `editor_surface_uses_full_rect_background_without_visible_card_inset` source-guard the removed decorative chrome.
- `src/editor/surface.rs`: `scroll_after_caret_move_clears_caret_pin` and `scroll_vertical_pixels_advances_viewport_after_visual_budget` validate that caret pinning is cleared by explicit scroll and that the viewport advances once the visual overflow budget is consumed.
- `src/editor/surface.rs`: `syntax_decoration_colors_are_distinct_by_token_family` locks the per-token-family syntax color mapping.
- `src/editor/surface.rs`: `visible_caret_offset_returns_none_when_caret_above_viewport` locks the overflow guard when the caret is above the visible snapshot after scrolling.
- Command: `cargo test -p clay --lib masonry_editor`

## Related

- [Masonry Shell Runtime](masonry-shell.md)
- [Client Snapshot Bootstrap](client-snapshot-bootstrap.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [End-to-End File Browser Workflow Primitive Review](end-to-end-file-browser-workflow-primitive-review.md)
- [Client Copy Selection Clay JS API](../../reference/clay-js-api/editor/client-copy-selection.md)
- `src/client/mod.rs`
