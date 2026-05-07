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

## Versioning

- Edits carry `behavior_version` and base document version.
- Manifest updates are atomic from the client's point of view.
- Hot reload publishes a new manifest version.
- Edits under stale behavior versions are accepted, corrected, rejected, or resynced according to the synchronization phase.
