# Phase 19 Persistent Runtime Hot Reload Primitive Review

## Source

- `src/server/js_runtime.rs`
- `src/server/mod.rs`
- `src/server/connection.rs`
- `src/server/parse_coordinator.rs`
- `src/server/workspace.rs`
- `runtime/js/packages.js`
- `runtime/js/modes.js`
- `runtime/js/parse.js`
- `packages/markdown/dist/load.js`
- `docs/wiki/modules/embedded-js-runtime.md`
- `docs/wiki/modules/package-loading.md`
- `docs/wiki/modules/parse-coordinator.md`
- `docs/wiki/modules/server-ipc-skeleton.md`
- `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md`

## Reviewed Primitives

| Primitive | Current capability | Phase 19 gap |
| --- | --- | --- |
| Persistent runtime service | `ClayJsRuntimeService` owns one worker-thread `JsRuntime`; evaluations preserve globals, module cache, loaded packages, mode activations, and parse-handler tokens. | Add explicit runtime generation ownership so hot reload builds a fresh service, validates config/package loads, then atomically swaps active generation. |
| Configuration/package loading | `load_configuration_from_root*` evaluates `~/.config/clay/init.js`; `loadPackage("@clay/markdown")` validates resolver-approved first-party packages and is idempotent in one runtime. | Treat reload as a fresh generation where `globalThis.__clayLoadedPackages` starts empty and default `init.js` reruns; do not add public `force` mutation semantics yet. |
| Module authority | `ClayModuleLoader` accepts curated `clay:*` facades, config-root relative `.js`, vendored `markdown-it`, and recorded package load-entry allowlist entries only. | Preserve the same allowlist rules per generation; failed next-generation loads must leave the previous generation active. |
| Mode registration/classification | `clay:modes` stores mode patterns and runtime-local activation metadata; selected-file open classifies paths generically and can lazy-load first-party packages. | After successful reload, reclassify/reactivate open documents through the same generic mode path; no Markdown-specific branch. |
| Runtime output application | `apply_runtime_outputs` applies behavior manifests, SDUI trees, and decoration pass-through from one evaluation. | Reuse this as generation-output application, but publish refresh messages to connected/open documents after the swap. |
| Parse handler bridge | `clay:parse.serverRegisterParseHandler` rejects executable callback payloads, stores JS functions in runtime-local token maps, and adapts registrations into `ParseCoordinator`. | Add generation metadata to handler registrations and parse tasks so old tokens are unregistered/ignored and stale parse results cannot publish after swap. |
| Parse scheduling | `ParseCoordinator` validates permission, ranges, windows, memory budgets, stale document versions, decoration payloads, and runs handlers in background tasks. | Add generic generation cancellation/replace APIs; keep parsing background and cancellable. |
| Workspace/open documents | `WorkspaceState` owns opened file documents, metadata lookup, handles, and `list_documents`; selected-file open is capability-gated. | Add or reuse safe open-document enumeration for reload refresh without granting broader filesystem authority or full-document IPC on ordinary edits. |
| Behavior manifests | `ActiveBehaviorManifest` publishes validated inert behavior and clients route key/input locally from installed manifests. | Swap to new manifests after activation refresh; clients keep hot-path routing inert and JavaScript-free. |
| Diagnostics | Runtime/configuration errors become sanitized `RuntimeDiagnostic` values. | Publish reload success/failure diagnostics without source snippets, absolute paths, tokens, secrets, or capability handles. |

## Existing Flow That Should Stay

```text
startup/init.js -> loadPackage("@clay/markdown") -> register modes/commands/parse handlers
selected-file open -> classify through clay:modes -> activate mode -> schedule bounded parse
ordinary edit -> server ack/client paint -> parse work later in background
```

Hot reload should be the same flow with a fresh runtime generation, not a second package/mode-specific implementation.

## Generic Gaps to Build

1. **RuntimeGeneration holder**: server-owned `{ generation_id, ClayJsRuntimeService, diagnostics }`, created off to the side and swapped only after configuration/package load succeeds.
2. **Generation-scoped package state**: `loadPackage` remains idempotent inside one generation; reload clears state by replacing the runtime, not by mutating globals.
3. **Generation-scoped parse registrations**: handlers and scheduled tasks carry `generation_id`; coordinator can replace handlers for a new generation and cancel old tasks.
4. **Late-result guard**: parse publication validates both document version and runtime generation before sending decorations.
5. **Open-document refresh primitive**: after swap, enumerate server-owned open documents, run generic classification/activation, publish behavior/decorations/diagnostics, and avoid full-document snapshots unless opening/reloading a file.
6. **Non-GUI trigger**: tests need a thin internal/server reload entrypoint that calls the shared primitive; no duplicate CLI/test reload logic.

## Rejected Alternatives

- **Mutate old runtime globals in place.** Module namespace objects, handler tokens, and closures can outlive cleared registries; stale handler invalidation is hard to prove.
- **Recreate per-open runtimes.** Reintroduces V8/disk churn and violates the persistent-runtime decision.
- **Markdown-specific reload branch.** Violates primitive-first mode planning; future modes need the same lifecycle.
- **Public `force` option on `loadPackage` now.** More API surface than needed; generation replacement gives cleaner semantics.
- **Process restart as reload.** Too heavy and drops open-document/lease state.

## Performance Boundary

Reload is explicit background/server-first work. No JavaScript should be added to keypress, paint, layout, scroll, Masonry text-event handling, local edit application, or edit acknowledgement hot paths. Parse refresh must remain bounded by existing `ParseWindowSnapshot`, `ParsePolicy`, payload-budget, timeout, and cancellation primitives.

## Security Boundary

Phase 19 does not expand package authority. Keep current resolver-validated `@clay/*` loading as an implementation limit, module loading through recorded package allowlist entries, package capability checks, executable callback rejection, server-held parse tokens, selected-file capability boundaries, workspace validation, and sanitized diagnostics. Reload must not grant new filesystem, network, shell, WASM, AI, raw-op, native-widget, client-JS, package-manager, package-control, or broader workspace authority beyond user-approved package capabilities.

## Minimal Model

```text
current generation G1 active
reload requested
  build G2 runtime service
  evaluate init.js / load configured first-party packages
  collect behavior/mode/parse registrations and diagnostics
  if valid: swap active generation to G2, cancel/ignore G1 parse work, refresh open docs
  if invalid: keep G1 active, publish sanitized diagnostics
```

This generation swap gives one simple invariant: active package, mode, behavior, and parse state belongs to exactly one runtime generation.
