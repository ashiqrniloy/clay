# Extensions and AI Pattern

## JavaScript Extensions

- JavaScript runs on the server through `deno_core`, not in the Rust client.
- Extensions register commands, modes, UI declarations, permissions, and behavior definitions.
- The server compiles extension registrations into behavior manifests and SDUI updates.
- Ordinary typing must not synchronously wait on JavaScript execution.

## Hot Reload

Hot reload flow:

1. Server detects or receives reload request.
2. Affected behavior scope may become temporarily locked/read-only.
3. Server re-evaluates JavaScript.
4. Server builds a new behavior manifest version.
5. Server sends manifest diff/snapshot to clients.
6. Clients atomically install it.
7. Editing resumes under the new behavior version.

## AI Mutation

- AI reads/proposals may be unlocked.
- AI edits should carry document version, behavior version, range, and permission scope.
- AI mutation should lock only the required scope: range, document, behavior, or workspace.
- Behavior-changing AI sessions should lock affected behavior/document scope until the new manifest is installed.
- Server emits transactions or UI updates; clients do not grant AI direct local mutation authority.

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
