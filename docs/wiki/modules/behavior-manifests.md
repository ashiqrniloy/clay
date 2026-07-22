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
- `src/server/ops/keybindings.rs` and `src/server/ops/behavior.rs` let server-side configuration JavaScript update/query manifests through validated Clay facades without adding JavaScript to the client hot path.
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

During embedded-runtime configuration evaluation, `bindKey` and `unbindKey` operate on a runtime-local `ActiveBehaviorManifest` in `ClayOpState`. They parse a single chord such as `Ctrl+S`, validate `editor`/`global` scope, reject unsupported `when` expressions and unregistered commands, and compile the result into the same `KeyBindingRule`/`CommandDeclaration` structures used by static manifests. If configuration changed behavior state, `ClayRuntimeEvaluation` returns the updated manifest so server startup can install it through normal manifest validation/versioning.

Every incoming `ClientMessage::Edit`, `ClientMessage::EditorIntent`, and server-first `ClientMessage::CommandIntent` carries a `behavior_version`. `src/server/connection.rs` checks edit/editor-intent versions against `ActiveBehaviorManifest::version()` before taking the document mutex and before calling `DocumentState::apply_edit`; command intents are rejected before command execution when the behavior version is stale. Edit mismatches return `ServerMessage::EditRejected { reason: EditRejection::InvalidBehaviorVersion { behavior_version, server_behavior_version } }`, preserving the canonical rope and document version.

On the client, `ClientBehaviorState::new` validates the initial manifest before a connected session is returned. Later `ServerMessage::BehaviorManifest` values are replacement candidates. The connection task validates the candidate, swaps it into active state only on success, and emits either `BehaviorManifestInstalled` with the installed manifest or `BehaviorManifestRejected` with the rejected version. Invalid replacements do not leave partial state.

Hot-path key routing is deterministic and local. `EditorWidget` converts Masonry character, Enter, and Tab events into protocol `KeyStroke` values. `EditorSurface::route_key_with_event` asks the client router whether the key is a built-in client edit, a completion request trigger, a client UI command, a server-first command intent, or unhandled. Manifest character binding lookup matches the stored lowercase chord case-insensitively when modifiers match exactly, so shifted chords such as `Ctrl+Shift+O` route even when the native keyboard event reports `"O"`; unbound shifted printable input still inserts the exact event text. Client-first edits reuse existing local mutation paths and enqueue edit events asynchronously; when an inserted character matches an inert autocomplete trigger, the editor builds a typed completion request from the post-edit caret/prefix metadata and the widget enqueues `ClientMessage::CompletionRequest` with bounded `try_send`. Manual `completion.trigger` key bindings use the same request path with `CompletionTrigger::Manual` and do not mutate text. Server-first routes enqueue bounded `ClientMessage::CommandIntent` values containing only the command ID, document ID, and behavior version, without mutating the local buffer. If the outbound IPC queue is full or absent, local manifest-declared editing still completes before any server or JavaScript work because enqueue uses bounded `try_send` rather than awaiting channel capacity.

The initial default text rules now execute as declarative manifest behavior. Enter inserts a newline plus the current line's leading spaces or tabs; when the trimmed current line starts with a declared comment prefix such as `//`, the inserted text also includes the declared continuation prefix. Tab inserts either the configured number of spaces or a literal tab according to `TabRule`. Pair rules intercept declared opening bracket/quote text, insert both sides at the caret with the caret left between them, or wrap the selected range. Autocomplete triggers are classified as inert `UiReactivePriority` declarations so future completion UI can observe the trigger without running extension code or mutating the document as part of trigger classification.

Phase 18.9 makes these generic key behaviors explicit and adds electric-character handling, all as `ClientFirstPredictable` manifest data rendered by Rust-known inert transform engines with no synchronous JavaScript, IPC, or server round trip before local paint. The `EditorBehaviorRules` manifest now carries an `electric_characters` array of `ElectricCharacterRule` entries, each naming a trigger character and a declarative `ElectricEffect` (currently `OutdentOneLevel`). When a closing bracket like `}`/`)`/`]` is typed as the first non-whitespace character on an over-indented line, the Rust engine sheds one indentation unit before inserting the trigger, so the closing bracket aligns with its opener. The effect is generic: any future language package can declare its own trigger/effect parameters, and the client executes only the Rust-known `ElectricEffect` variants it recognises, so packages contribute rule parameters only — no client-side JavaScript, raw ops, native widget handles, or callbacks. Electric-character rules that fail validation (e.g. an empty trigger) cause the whole manifest to be rejected as a malformed rule set. Package-declared electric characters flow through the same `electricCharacters` JSON field under `clay.modes` editor rules; only the `outdent-one-level` effect is accepted, unknown effects are dropped.

Phase 18.18 first-party language packages reuse `runtime/js/behavior.js::buildCodeEditingManifest` rather than adding language branches. Rust installs 4-space indentation, common bracket/quote pairs, `//` continuation, electric `}`, and one-character `.`/`:` completion triggers. TypeScript and JavaScript install 2-space indentation, template-literal pairs, `//` continuation, electric `}`/`)`/`]`, and `.` completion. Markdown supplies the helper's generic `continueLineMarkers` Enter rule, 2-space indentation, prose delimiter pairs, and `#`/`[`/`` ` `` triggers, with no electric or line-comment continuation rules. `clay:modes` stores this inert activation metadata in the persistent runtime and deserializes it into the same protocol types when a classified document opens.

`buildCodeEditingManifest` drops duplicate or non-single-character electric/autocomplete triggers and caps autocomplete triggers at the protocol maximum of 32. This keeps helper output valid before `validate_manifest` performs the authoritative server check. Comment-toggle commands remain package-prefixed, server-first command declarations; status labels remain inert `statusItem` component contributions with package provenance.

Two built-in default rule sets ship with Clay. `EditorBehaviorRules::default_text()` (exposed via `BehaviorManifest::minimal_text_editing`) is the plain-text rule set used by the always-on `core.text` fallback and the base for package modes: generic indent-preserving Enter, Tab, common bracket/quote pairs, `//` comment continuation, and no electric characters. `EditorBehaviorRules::default_code()` (exposed via `BehaviorManifest::core_code_editing`) is the code-oriented rule set used by the always-on `core.code` fallback: identical indentation/pair/comment behavior plus electric-character reflow for `}`, `)`, and `]`, so generic code editing works with no language package loaded. `ModeRegistry::select_behavior_manifest_for_document` selects `core_code_editing` as the base manifest when the active major mode is the built-in `core.code` (and `minimal_text_editing` otherwise); built-in `core.*` modes ship their rule sets directly without requiring an enabled package record. Unmatched keybindings fall back to the built-in `core.*` manifest rules rather than blocking input, so a document that activates a built-in fallback mode is always editable.

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
- Server-first command routes do not mutate local text before server acknowledgement and reach server command execution through `ClientMessage::CommandIntent` rather than a UI-specific dispatcher.
- The server, not the client, chooses and advances behavior versions; client-supplied stale or future behavior versions cannot bypass validation.
- Runtime key binding registration is a manifest compilation step, not a client JavaScript handler installation step.
- Behavior-version validation happens before document mutation and does not inspect full document text.

## Tests

- `src/protocol/codec.rs`: round-trips `ServerMessage::BehaviorManifest` updates and `InvalidBehaviorVersion` rejections through the IPC codec, and rejects invalid or oversized manifest frames.
- `src/behavior/manifest.rs`: validates executable/side-effect authority rejection, duplicate command/key binding rejection, and all routing policy variants.
- `src/server/behavior.rs`: validates replacement publishing increments behavior versions, rejects invalid replacements without advancing state, and reports version mismatch metadata.
- `src/server/js_runtime.rs`: validates runtime `bindKey`/`unbindKey`, behavior query facades, unknown command rejection, manifest-based client key routing, each first-party package's activated indent/Enter/pair/comment/electric/completion rules and payload budget, package-prefixed server-first comment commands, status-item provenance, and absence of per-language Rust registration branches.
- `src/server/connection.rs`: validates handshake manifest publication and stale behavior-version edit rejection without canonical mutation.
- `src/client/behavior.rs`: validates atomic client replacement, previous-manifest retention on invalid replacement, client-first key routing, shifted character binding normalization, shifted printable fallback insertion, the configuration contract that a `Ctrl+Shift+O` folder binding routes on a Linux uppercase-`O` key event, Tab routing, autocomplete trigger classification, manual `completion.trigger` routing, and server-first intent routing.
- `src/client/mod.rs`: validates full outbound edit queues fail immediately via `try_send` without awaiting IPC capacity and that completion request events enqueue typed `ClientMessage::CompletionRequest` values without an edit mutation.
- `src/client/mod.rs`: validates runtime manifest replacement and rejection events from the connection loop.
- `src/editor/surface.rs`: validates client-first key routing mutates locally, autocomplete trigger requests are built after local insertion, manual completion requests do not mutate text, ordinary typing completes locally without a server/JavaScript wait, server-first key routing does not mutate local text, Enter indentation, configured Tab insertion, pair insertion/wrapping, and comment continuation.
- Command: `cargo test --quiet`.

## Related

- [Protocol Codec](protocol-codec.md)
- [Behavior Runtime Registration](behavior-runtime-registration.md)
- [Persistent Runtime Hot Reload](persistent-runtime-hot-reload.md) — Phase 19 `BehaviorGraceState` stale-edit grace and `InvalidBehaviorVersion` resync.
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- [Client Edit Emission](../flows/client-edit-emission.md)
- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- `.agents/skills/project-patterns/references/behavior-manifests.md`
