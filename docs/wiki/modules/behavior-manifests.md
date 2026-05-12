# Behavior Manifests

## Source

- `src/protocol/mod.rs`
- `src/behavior/manifest.rs`
- `src/server/behavior.rs`
- `src/server/connection.rs`
- `src/client/behavior.rs`
- `src/client/mod.rs`
- `src/editor/surface.rs`
- `src/masonry_editor.rs`
- `src/protocol/codec.rs`

## Overview

Behavior manifests are server-owned, server-issued, inert declarations that let the client execute predictable editor behavior without a synchronous server, JavaScript, AI, file, shell, or network round trip. Phase 6 replaces the earlier minimal text-editing capability placeholder with a structured schema for manifest identity, behavior versioning, scope, key bindings, command declarations, routing policy, and editor rules.

## Responsibilities

- `src/protocol/mod.rs` defines the serializable wire/data model.
- `src/behavior/manifest.rs` validates manifest invariants before a manifest is trusted or installed.
- `src/server/behavior.rs` owns the active server manifest, publishes validated replacements with deterministic version increments, and performs constant-time behavior-version checks.
- `src/server/connection.rs` sends the active manifest during handshake and rejects edit/intent messages whose behavior version does not match the active server version before canonical document mutation.
- `src/client/behavior.rs` validates initial/replacement manifests, atomically swaps active client behavior, and routes key strokes to local built-in edits or server-intent declarations.
- `src/client/mod.rs` validates handshake manifests and processes replacement manifest messages from the server connection loop.
- `src/editor/surface.rs` consults the installed manifest to decide whether ordinary edits can emit client-first edit events and uses the client router for key-level behavior.
- `src/masonry_editor.rs` forwards character, Enter, and Tab text events through manifest routing without awaiting IPC.
- `src/protocol/codec.rs` serializes manifests through the same length-prefixed `rkyv` IPC boundary as other protocol messages.

## How It Works

`BehaviorManifest` contains:

1. `manifest_id`, `behavior_version`, and `scope` for stable identity and versioned routing decisions.
2. `keymaps` mapping key sequences and contexts to command IDs and routing policies.
3. `commands` declaring command IDs, display names, routing policy, and authority class.
4. `editor_rules` for text edit capabilities, Enter behavior, Tab behavior, bracket/quote pairs, comment continuation, and autocomplete triggers.

Routing is explicit through `RoutingPolicy` variants: client-first predictable, client-first requiring acknowledgement, server-first, server-first with a lock scope, UI-reactive priority, or background. Client-first commands may only use built-in edit authority; permission-bearing or side-effectful work is represented as server intents and cannot execute directly in the Rust client.

`validate_manifest` checks duplicate command IDs, unknown key binding commands, ambiguous key sequences, invalid tab/pair/autocomplete rules, and authority/routing mismatches. The schema has no field for arbitrary scripts or executable hooks.

On the server, `ActiveBehaviorManifest` wraps the current manifest. `Default` creates and validates the default text-editing manifest at behavior version `1`. Connection handshake sends `ServerMessage::BehaviorManifest` from this active state rather than constructing an ad hoc manifest in codec or connection code. Replacement publishing validates the candidate manifest, overwrites its version with `current + 1`, and only swaps the active manifest after validation succeeds; invalid candidates leave the previous manifest and version active.

Every incoming `ClientMessage::Edit` and `ClientMessage::EditorIntent` carries a `behavior_version`. `src/server/connection.rs` checks that value against `ActiveBehaviorManifest::version()` before taking the document mutex and before calling `DocumentState::apply_edit`. A mismatch returns `ServerMessage::EditRejected { reason: EditRejection::InvalidBehaviorVersion { behavior_version, server_behavior_version } }`, preserving the canonical rope and document version.

On the client, `ClientBehaviorState::new` validates the initial manifest before a connected session is returned. Later `ServerMessage::BehaviorManifest` values are replacement candidates. The connection task validates the candidate, swaps it into active state only on success, and emits either `BehaviorManifestInstalled` with the installed manifest or `BehaviorManifestRejected` with the rejected version. Invalid replacements do not leave partial state.

Hot-path key routing is deterministic and local. `EditorWidget` converts Masonry character, Enter, and Tab events into protocol `KeyStroke` values. `EditorSurface::route_key_with_event` asks the client router whether the key is a built-in client edit, a server-first command intent, or unhandled. Client-first edits reuse existing local mutation paths and enqueue edit events asynchronously; server-first routes return an intent declaration without mutating the local buffer. If the outbound IPC queue is full or absent, local manifest-declared editing still completes before any server or JavaScript work because enqueue uses bounded `try_send` rather than awaiting channel capacity.

The initial default text rules now execute as declarative manifest behavior. Enter inserts a newline plus the current line's leading spaces or tabs; when the trimmed current line starts with a declared comment prefix such as `//`, the inserted text also includes the declared continuation prefix. Tab inserts either the configured number of spaces or a literal tab according to `TabRule`. Pair rules intercept declared opening bracket/quote text, insert both sides at the caret with the caret left between them, or wrap the selected range. Autocomplete triggers are classified as inert `UiReactivePriority` declarations so future completion UI can observe the trigger without running extension code or mutating the document as part of trigger classification.

## Code Examples

```rust
use clay::behavior::manifest::validate_manifest;
use clay::protocol::BehaviorManifest;

let manifest = BehaviorManifest::minimal_text_editing(1);
validate_manifest(&manifest).unwrap();
```

## Invariants and Constraints

- Manifests are inert data, not JavaScript, WASM, shell commands, filesystem operations, network calls, workspace mutations, or AI tool invocations.
- Ordinary edit messages still carry deltas and metadata, including behavior version, instead of full documents.
- Protocol semantics remain outside `src/protocol/codec.rs`; the codec only frames and serializes messages.
- The client keeps using the server-issued behavior version when emitting edit transactions.
- Client manifest installation is atomic: a replacement validates before it becomes active, and invalid replacements keep the previous active behavior.
- Enter, Tab, pair insertion, comment continuation, and autocomplete trigger classification are driven by installed manifest data, not hardcoded side-effectful extension code.
- Server-first command routes do not mutate local text before server acknowledgement.
- The server, not the client, chooses and advances behavior versions; client-supplied stale or future behavior versions cannot bypass validation.
- Behavior-version validation happens before document mutation and does not inspect full document text.

## Tests

- `src/protocol/codec.rs`: round-trips `ServerMessage::BehaviorManifest` updates and `InvalidBehaviorVersion` rejections through the IPC codec, and rejects invalid or oversized manifest frames.
- `src/behavior/manifest.rs`: validates executable/side-effect authority rejection, duplicate command/key binding rejection, and all routing policy variants.
- `src/server/behavior.rs`: validates replacement publishing increments behavior versions, rejects invalid replacements without advancing state, and reports version mismatch metadata.
- `src/server/connection.rs`: validates handshake manifest publication and stale behavior-version edit rejection without canonical mutation.
- `src/client/behavior.rs`: validates atomic client replacement, previous-manifest retention on invalid replacement, client-first key routing, Tab routing, autocomplete trigger classification, and server-first intent routing.
- `src/client/mod.rs`: validates full outbound edit queues fail immediately via `try_send` without awaiting IPC capacity.
- `src/client/mod.rs`: validates runtime manifest replacement and rejection events from the connection loop.
- `src/editor/surface.rs`: validates client-first key routing mutates locally, ordinary typing completes locally without a server/JavaScript wait, server-first key routing does not mutate local text, Enter indentation, configured Tab insertion, pair insertion/wrapping, and comment continuation.
- Command: `cargo test --quiet`.

## Related

- [Protocol Codec](protocol-codec.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- [Client Edit Emission](../flows/client-edit-emission.md)
- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- `.agents/skills/project-patterns/references/behavior-manifests.md`
