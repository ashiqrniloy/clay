# Extensions and AI Pattern

## JavaScript Extensions

- JavaScript runs on the server through `deno_core`, not in the Rust client.
- Extensions register commands, modes, UI declarations, permissions, and behavior definitions.
- The server compiles extension registrations into behavior manifests and SDUI updates.
- Ordinary typing must not synchronously wait on JavaScript execution.

## Hot Reload

- Prepare and validate a fresh runtime-generation candidate while the current generation remains active; do not mutate live state during evaluation.
- Serialize reload attempts, but acquire `LockScope::Behavior` only for the final compare-and-swap commit. Ordinary typing and background parsing must not wait for JavaScript evaluation.
- Commit all generation-owned server contributions once, then broadcast one bounded complete snapshot per affected connection. Clients validate and atomically install the whole snapshot before acknowledging its runtime generation.
- The commit is the rollback boundary: pre-commit failure preserves the old generation; post-commit fan-out/cleanup failure recovers from latest state and must not restore revoked authority.
- Revoke old executable authority logically at commit and terminate old workers/sessions afterward under their existing bounded cleanup rules.
- Use the explicit built-in `clay.runtime.reloadConfiguration` command through normal command execution, with no default keybinding. Add no watcher, reload-specific IPC, or diff protocol until measured need justifies it.
- Decision log source: `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`.

## AI Mutation

- AI reads/proposals may be unlocked.
- AI edits should carry document version, behavior version, range, and permission scope.
- AI mutation should lock only the required scope: range, document, behavior, or workspace.
- Behavior-changing AI sessions should lock affected behavior/document scope until the new manifest is installed.
- Server emits transactions or UI updates; clients do not grant AI direct local mutation authority.

## Package Distribution

- Installable Clay packages should use the npm-compatible package distribution direction in `package-distribution.md` unless a later approved decision supersedes it.
- Package installation and package execution remain separate: package managers download and resolve dependencies; Clay validates package metadata, permissions, documentation coverage, behavior contributions, and runtime/load-time boundaries before server-side execution.
- Package-provided Clay JS APIs must use the package name or registered package prefix so users and AI agents can identify provenance.

## Future WASM Modules

WASM may eventually support sandboxed hot-path behavior modules, but plans should treat this as future architecture unless explicitly in scope.

If introduced, require:

- Stable ABI.
- Capability/permission model.
- Fuel/time limits.
- Memory limits.
- Deterministic host APIs.
- Versioned module manifests.
- Documentation registry entries.
