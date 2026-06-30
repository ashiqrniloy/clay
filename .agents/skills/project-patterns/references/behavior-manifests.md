# Behavior Manifest Pattern

## Core Rule

The server owns behavior definitions; the client executes only inert, versioned behavior manifests for latency-sensitive hot-path behavior.

## Use a Behavior Manifest When

- A keypress should be client-first and predictable.
- Editor mode behavior must be immediate, such as Enter indentation, Tab handling, bracket/quote pairing, Markdown list continuation, or comment continuation.
- The client needs to route keybindings without asking the server synchronously.
- UI-reactive behavior needs triggers, priorities, or cancellation policy.

## Do Not Use a Behavior Manifest For

- Arbitrary JavaScript execution in the client.
- File/workspace/shell/network/AI side effects.
- Unknown extension commands.
- Long-running computation.
- Security-sensitive permission decisions.

## Routing Policies

Plans should classify commands/key behavior as one of:

- `ClientFirstPredictable` — apply locally, send async transaction.
- `ClientFirstRequiresAck` — apply locally but expect confirmation/correction.
- `ServerFirst` — send intent; client waits for server result.
- `ServerFirstWithLock` — server locks range/document/behavior/workspace before mutation.
- `UiReactivePriority` — trigger async cancellable UI work such as completion/diagnostics.
- `Background` — non-urgent work that must not delay edits or UI-reactive work.

## Generic Key-Behavior Manifest Kinds (Phase 18.9)

- The generic `TextTransform` kinds (pair insertion `PairRule`, comment continuation `CommentContinuationRule`, Tab `TabRule`, Enter `EnterRule`, and electric characters `ElectricCharacterRule` + `ElectricEffect`) are reusable across all future modes, not Markdown-only or Python-only. Ship two default rule sets: `EditorBehaviorRules::default_text()` / `BehaviorManifest::minimal_text_editing` (no electric) and `EditorBehaviorRules::default_code()` / `BehaviorManifest::core_code_editing` (electric outdent for `}`/`)`/`]`).
- Electric characters are the only new manifest *kind*; they extend the `EnterRule`/`PairRule` family, not a new primitive *category*. Only Rust-known engines execute them (`insert_electric_with_event`, `dedent_leading_one_level` in `src/editor/surface.rs`); package JSON parsing accepts only the `outdent-one-level` effect and drops unknown effects. Future electric effects each need a Rust-known engine.
- Built-in `core.*` modes ship their own default behavior manifests without an owning package; `select_behavior_manifest_for_document` detects the `core.` prefix and bypasses package-record lookup.
- Decision log source: `decision-logs/2026-07-01-0350-phase18-9-generic-text-code-fallback-modes-and-key-behavior.md`.

## Versioning

- Edits carry `behavior_version` and base document version.
- Manifest updates are atomic from the client's point of view.
- Hot reload publishes a new manifest version.
- Edits under stale behavior versions are accepted, corrected, rejected, or resynced according to the synchronization phase.
