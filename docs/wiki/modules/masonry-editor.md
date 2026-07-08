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

## Tests

- `src/masonry_editor.rs`: `status_observation_local_fallback_state` validates local/no-server observation fields.
- `src/masonry_editor.rs`: `status_observation_connected_editable_with_version` validates confirmed version and editable state after an edit acknowledgement.
- `src/masonry_editor.rs`: `status_observation_diagnostic_present_after_runtime_diagnostic_event` validates diagnostic forwarding into observable GUI chrome.
- `src/masonry_editor.rs`: `status_observation_does_not_regress_accessibility_label` validates consistency between status observation and accessibility text.
- `src/editor/surface.rs`: `selected_text_returns_forward_backward_unicode_ranges` and `selected_text_returns_none_for_collapsed_selection` validate UTF-8 selection extraction.
- `src/masonry_editor.rs`: `copy_selection_writes_selected_text_without_edit_event`, `copy_selection_is_noop_when_selection_is_collapsed`, and `copy_selection_failure_reports_runtime_diagnostic` validate clipboard write/no-op/failure behavior with a fake sink.
- `src/client/clipboard.rs`: `clipboard_sink_accepts_utf8_text` validates the test sink contract without requiring a desktop clipboard.
- Command: `cargo test -p clay --lib masonry_editor`

## Related

- [Masonry Shell Runtime](masonry-shell.md)
- [Client Snapshot Bootstrap](client-snapshot-bootstrap.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [End-to-End File Browser Workflow Primitive Review](end-to-end-file-browser-workflow-primitive-review.md)
- [Client Copy Selection Clay JS API](../../reference/clay-js-api/editor/client-copy-selection.md)
- `src/client/mod.rs`
