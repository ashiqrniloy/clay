# Masonry Editor Widget Status Observability

## Source

- `src/masonry_editor.rs`
- `src/client/mod.rs`

## Overview

`EditorWidget` composes the native editor surface, server-driven UI overlay, and bottom status chrome. The status chrome reflects connection state, document access, confirmed sync version, and the latest sanitized runtime diagnostic forwarded by `ClientConnectionEvent::RuntimeDiagnostic`.

Phase 15 adds `SduiStatusObservation`, a `pub(crate)` headless observability struct for tests and internal agent inspection. It is not a Clay JS API surface; it only exposes strings and version metadata already visible in GUI chrome.

## Responsibilities

- Apply `ClientConnectionEvent` values on the GUI thread and update editor, SDUI, or status state without blocking paint/input paths.
- Render and expose accessible status text for connection, access, document, version, and runtime diagnostics.
- Provide `EditorWidget::status_observation()` so tests can assert status chrome state without opening a window or painting.
- Keep diagnostics sanitized by displaying only the `RuntimeDiagnostic` code/message supplied by the server protocol.

## How It Works

`EditorStatus` stores the current `EditorConnectionStatus`, optional document ID, optional confirmed document version, optional `DocumentAccess`, and optional `RuntimeDiagnostic`. Small label helpers derive the user-visible connection, access, document, version, and diagnostic strings. `EditorStatus::text()` builds the exact status line painted by the widget.

`EditorWidget::status_observation()` delegates to `EditorStatus::observation()`, returning a `SduiStatusObservation` with:

- `status_text`: the exact GUI chrome status text.
- `connection_label`: the connection portion, such as `Connected` or `Local Fallback`.
- `access_label`: the access portion, such as `Editable`, `Read-only Observer`, or `No Server`.
- `sync_version`: the current confirmed document version when known.
- `diagnostic_text`: the active runtime diagnostic text when present.

`EditorWidget::status_text()` reads from the same observation path, and `accessibility_label()` includes that status text, so tests do not need to parse a separate accessibility string to inspect status fields.

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
- Ordinary text input and paint do not wait for IPC, server work, JavaScript, or diagnostic processing.

## Tests

- `src/masonry_editor.rs`: `status_observation_local_fallback_state` validates local/no-server observation fields.
- `src/masonry_editor.rs`: `status_observation_connected_editable_with_version` validates confirmed version and editable state after an edit acknowledgement.
- `src/masonry_editor.rs`: `status_observation_diagnostic_present_after_runtime_diagnostic_event` validates diagnostic forwarding into observable GUI chrome.
- `src/masonry_editor.rs`: `status_observation_does_not_regress_accessibility_label` validates consistency between status observation and accessibility text.
- Command: `cargo test -p clay --lib masonry_editor`

## Related

- [Client Snapshot Bootstrap](client-snapshot-bootstrap.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- `src/client/mod.rs`
