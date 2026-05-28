# Client Behavior Routing

## Source

- `src/client/behavior.rs`
- `src/client/mod.rs`
- `src/editor/surface.rs`
- `src/masonry_editor.rs`
- `src/protocol/mod.rs`

## Overview

Client behavior routing installs server-issued behavior manifests atomically and uses the active manifest to classify hot-path editor keys. The router is inert: it maps key strokes to built-in local edit actions or server-intent declarations, and it does not execute JavaScript, load extensions, touch files, call AI tools, or perform network/workspace side effects.

## How It Works

`ClientBehaviorState` owns one active `BehaviorManifest`. Construction and replacement both call `validate_manifest` before the active manifest is changed. If validation fails, the previous manifest remains active and the connection task emits `ClientConnectionEvent::BehaviorManifestRejected` with the rejected behavior version.

The client handshake validates the initial `ServerMessage::BehaviorManifest` before returning `ClientInitialState`. During the background connection loop, later `BehaviorManifest` messages are treated as replacements:

1. Clone and validate the candidate manifest.
2. Swap the active manifest only when validation succeeds.
3. Emit `BehaviorManifestInstalled { behavior_version, manifest }` so GUI state can install the same manifest.
4. Emit `BehaviorManifestRejected` on validation failure without mutating active state.

`EditorSurface::route_key_with_event` builds a router from the installed manifest and routes key strokes. Client-first predictable routing invokes local rule execution and produces `EditorEditEvent` values with the active behavior version. Enter preserves leading whitespace and continues simple line comments from manifest declarations, Tab inserts the manifest-configured tab text, and bracket/quote pair rules insert both sides at the caret or wrap the selected range. Server-first routing returns a `ServerIntentRoute` without changing local text.

`EditorWidget::on_text_event` uses manifest routing for ordinary character insertion, Enter, and Tab. Printable character input is still treated as text when Shift is the only modifier, so shifted letters and symbols insert the already-resolved character text while Ctrl/Alt/Super combinations remain available for shortcuts or platform input behavior. Autocomplete trigger declarations are classified by `ClientBehaviorState` as inert UI-reactive triggers for later completion work; this classification does not run completion logic or mutate the document. Local mutations still enqueue edit transactions through `ClientEditQueue::try_send`, so input handling does not await IPC, server work, JavaScript, file IO, AI, or full-document serialization.

## Invariants and Constraints

- Manifest replacement is all-or-nothing from the client point of view.
- Key routing uses bounded in-memory manifest lookups and current line/selection slices only.
- Autocomplete triggers are declarations only; they do not execute extensions or side effects.
- Declared server-first commands do not mutate the local editor before a server response.
- Local edit events carry the active behavior version for server validation.
- The Masonry input handler never awaits outbound IPC.

## Tests

- `src/client/behavior.rs`
  - `client_installs_valid_manifest_atomically`
  - `client_keeps_previous_manifest_when_replacement_invalid`
  - `client_routes_client_first_key_without_ipc_wait`
  - `client_routes_shifted_printable_key_as_text_input`
  - `client_does_not_route_control_character_as_text_input`
  - `client_routes_tab_from_manifest_rules`
  - `autocomplete_trigger_declared_without_client_side_side_effect`
  - `client_routes_server_first_command_as_intent`
- `src/client/mod.rs`
  - `client_installs_behavior_manifest_replacement_event`
  - `client_rejects_invalid_behavior_manifest_replacement_event`
- `src/editor/surface.rs`
  - `editor_routes_client_first_key_through_manifest`
  - `editor_routes_server_first_key_without_local_mutation`
  - `enter_rule_preserves_leading_indentation`
  - `tab_rule_inserts_configured_spaces`
  - `pair_rule_wraps_selection_or_inserts_pair`
  - `comment_continuation_rule_continues_simple_comment_prefix`
- Command: `cargo test --quiet`.

## Related

- [Behavior Manifests](../modules/behavior-manifests.md)
- [Client Edit Emission](client-edit-emission.md)
- [Client/Server Edit Acknowledgement Flow](client-server-edit-ack.md)
