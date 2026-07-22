# Phase 19 Hot Reload and Behavior Update Primitive Review

## Source

- `roadmap.md`
- `plans/033-Phase19-Persistent-Runtime-Hot-Reload-Semantics.md`
- `plans/036-Phase18.8-Bottom-Pane-Transient-Menu-and-Command-Execution-Foundation.md` through `plans/053-Phase18.21-First-Party-LSP-Bridge-Packages.md`
- `src/server/mod.rs`
- `src/server/js_runtime.rs`
- `src/server/connection.rs`
- `src/server/behavior.rs`
- `src/server/{parse_coordinator,completion,language_intelligence,document_analysis,language_server,syntax,ui}.rs`
- `src/packages/{service,modes,commands}.rs`
- `src/protocol/{mod,codec,parse,decorations,diagnostics,sdui,completion,language_intelligence}.rs`
- `src/client/{mod,behavior}.rs`
- `src/shell/package_ui.rs`
- `src/perf/budgets.rs`
- `tests/primitives_docs.rs`
- `tests/persistent_runtime_hot_reload.rs`

## Overview

This is the implementation-entry review for the current roadmap Phase 19. It succeeds the narrower [Phase 19 Persistent Runtime Hot Reload Primitive Review](phase19-persistent-runtime-hot-reload-primitive-review.md), which was completed by Plan 033 before command execution, package UI, tiered syntax, completion, range diagnostics, language intelligence, document-analysis workers, and first-party LSP bridges existed.

Plan 033 already supplies a fresh-runtime generation swap, generation-scoped parse cancellation, configuration reload, sanitized rollback diagnostics, open-document refresh, and a doc-hidden developer trigger. Current Phase 19 must complete the transaction and client-update semantics around that baseline; it must not build a second package loader, mode registry, parse scheduler, provider lane, renderer, or language-specific reload path.

## Entry Gate

Roadmap Phase 19 depends on Phase 18.7 and the Phase 18.8-18.14 package-capability sequence. Those gates are complete:

- Phase 18.7 persistent runtime/parse bridge: completed Plan 031 and approved decision `decision-logs/2026-06-26-1338-phase18-7-persistent-runtime-and-js-parsehandler-bridge.md`.
- Phase 18.8-18.14: completed Plans 036-042.
- Later contribution foundations used by this review: Plans 043-044 and 046-053 are complete.
- Plan 045 is not complete and is not an active gate. Its web-tree-sitter-only engine choice was explicitly superseded by approved decision `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md` and completed Plan 047, which preserves Plan 045's binding non-blocking open-parse and parse-error diagnostic work.
- Plan 033 is complete and remains the implemented partial Phase 19 baseline.

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.

## Existing Primitive Inventory

| Category | Existing generic primitive and owner | What already works | Phase 19 gap |
| --- | --- | --- | --- |
| Runtime generation | `RuntimeGenerationStore`, `RuntimeGeneration`, `ClayJsRuntimeService` in `src/server/{mod,js_runtime}.rs` | One worker-owned `JsRuntime` per generation; fresh service reevaluates `init.js`; successful evaluation advances generation; syntax/timeout/heap failures preserve the old service and emit sanitized diagnostics. | The service swap is not yet one transaction with every shared registry and client-visible contribution. |
| Configuration/package loading | `loadPackage`, `PackageService`, `PackageLoadEntryAllowlist`, generation-local `globalThis.__clayLoadedPackages` | Bundled and installed user-authorized sources resolve through one package service; manifest, provenance, capabilities, graph relations, conflicts, package-root confinement, and load entries are validated; one-line loads rerun in a fresh runtime. | Candidate package records/grants/contributions need one commit/rollback boundary; old package workers and outputs need coordinated post-commit withdrawal. |
| Modes | `ModeRegistry`, `DocumentClassification`, `MajorModeActivation`, built-in `core.text`/`core.code` | Generic path/metadata classification, one active major mode, fallback editability, behavior selection, and reload-time open-document reclassification without language branches. | Reclassification results are returned from reload but not delivered as a coherent live-client generation. |
| Commands | `CommandRegistry`, `CommandExecutor`, `ControlCenter`, `TransientMenuSession` | Package and built-in command metadata, server-owned validation, shared SDUI/keybinding/menu execution routing, provenance, permissions, and bounded arguments. | No supported user-facing reload command or behavior/document/workspace reload-lock manager exists; developer trigger bypasses discoverable command UX. |
| Behavior | `ActiveBehaviorManifest`, `BehaviorManifest`, `ClientBehaviorState` | Server validates replacements and increments `behavior_version`; client validates then atomically replaces one manifest; edits/intents carry behavior version. | Server mutates behavior before generation swap; current server accepts only exact version; no bounded stale-version grace/ack protocol; `InvalidBehaviorVersion` does not currently request client resync. |
| Syntax and parsing | `SyntaxGrammarRegistry`, tiered `TreeSitterSyntaxHandler`, `ParseCoordinator` | Runtime/native/JS handlers share background scheduling, bounded windows, generation ownership, cancellation, stale document/generation rejection, diagnostics, and inert decorations. | Syntax selection plus decoration/diagnostic state lacks one client-visible runtime/render generation; coordinator output uses a shared single-consumer channel rather than reload fan-out. |
| Completion | `CompletionProviderRegistry`, `CompletionCoordinator`, snippet/exclusive/disable primitives | Bounded UI-reactive requests, provider generation, cancellation, stale result rejection, inert client-local snippet acceptance, dynamic document-analysis adapter. | Reload registration errors are recorded after active mutation and do not fail the candidate; live client has no generation-wide reset/install event. |
| Language intelligence | `LanguageIntelligenceCoordinator` and token-backed JS provider bridge | One bounded cancellable lane for hover/definition/actions/signatures with generation and document-version checks. | Same partial-registration and generation-wide install gap as completion. |
| Document analysis/LSP | `DocumentAnalysisCoordinator`, `LanguageServerProcessService` | Workers are keyed by package/contribution/root/runtime generation; exact grants, bounded mailboxes, canonical accepted deltas, stale rejection, and `cancel_generation` cleanup exist. | Reload currently registers candidate workers/providers into live coordinators before swap; failure in one registration does not roll back earlier mutations. Cleanup and client output clearing are not one commit lifecycle. |
| SDUI | `StaticSduiState`, `SduiSnapshot`, `SduiUpdate` | Runtime trees are validated and can replace per-document SDUI; clients render inert Clay-owned trees. | `apply_runtime_outputs` mutates shared SDUI before runtime swap, and reload refresh messages are not broadcast to connected clients. |
| Package UI | `PackageUiRegistrySnapshot::runtime_update`, `PackageUiRuntimeState` | Package panels/components/overlays/theme tokens/input routes/state-scope metadata are validated; shell runtime can atomically apply a bounded inert update. | `ClayRuntimeEvaluation.ui_contributions` is test-visible only. `IpcServer` owns no package-UI registry, and no package-UI snapshot crosses IPC during startup or reload. |
| Theme and typography | `active_theme`, `ActiveTypographyState`, `StyleRegistry`, `TypographyRegistry` | Complete inert theme/typography candidates validate; typography has a bounded broadcast channel with lag recovery; client install paths exist. | Theme has no live broadcast; typography broadcasts independently; both mutate before runtime swap and can be observed separately from behavior/UI/rendering. |
| Decorations and diagnostics | `DecorationSet`, `DiagnosticSet`, server validators/caches, editor chunk stores | Bounded viewport/source updates, document-version rejection, package provenance, cache limits, native paint from inert state. | Messages carry document versions but no runtime/render generation. Reload can therefore interleave old asynchronous output with new behavior/theme/UI state. |
| Protocol and client install | `ServerMessage`, `ClientMessage`, `Codec`, `ClientConnectionEvent` | `rkyv` frames are archive-validated and capped; behavior, theme, typography, SDUI, decoration, and diagnostic messages each have validated install paths. | No `RuntimeGenerationId`, complete reload snapshot, install acknowledgement, lag recovery snapshot, or one atomic client event spans all parts. |
| Workspace/document authority | `WorkspaceState`, `DocumentState`, leases, region locks, selected-file capabilities | Server owns canonical ropes, versions, roots, open-document snapshots, leases, range locks, and selected-file grants. | Protocol declares behavior/document/workspace lock scopes, but reload has no generic scoped lock manager; old-behavior edit recovery is undefined. |

## Current Reload Ordering and Atomicity Gaps

Current `IpcServer::reload_runtime_generation` builds and evaluates a fresh `ClayJsRuntimeService`, but `apply_runtime_evaluation` then mutates live state before `RuntimeGenerationStore::swap`:

1. `apply_runtime_outputs` publishes the behavior replacement and replaces shared SDUI.
2. Active theme is overwritten and typography is installed/broadcast.
3. Parse, completion, document-analysis, and language-intelligence registrations are inserted into live coordinators. Registration failures become diagnostics and do not abort the reload.
4. Old coordinator generations are cancelled.
5. Runtime store swaps to the new service/generation.
6. Open documents are refreshed and returned in `RuntimeReloadOutcome.refreshed_documents` to the developer caller.

This is atomic only for the runtime service pointer. A behavior/SDUI/theme/typography/provider mutation or partial registration can be visible while the store still reports the old generation. A validator/registration error can leave mixed active state and still return `reloaded: true`.

Live connections subscribe only to typography plus shared parse/analysis/provider result sources. They do not subscribe to `RuntimeReloadOutcome`; refreshed behavior/SDUI/decorations/diagnostics are not fanned out by reload. The connection source comment also records that one connection drains the shared parse channel, so this is not a multi-client broadcast primitive.

Client startup and live updates are separate messages. Startup receives Welcome, InitialDocument, BehaviorManifest, ActiveTheme, ActiveTypography, then SDUI/diagnostics. Live behavior, theme, typography, SDUI, decoration, and diagnostic events apply independently. No message proves that all installed parts belong to the same runtime generation.

## Existing Budgets and Hot-Path Policy

Typed limits already cover the work Phase 19 must compose rather than replace:

| Area | Existing limit |
| --- | --- |
| Local responsiveness | `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS = 16`, `EDIT_ACK_P95_BUDGET_MS = 40`, `SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS = 16` |
| Runtime | `JS_RUNTIME_EVALUATION_TIMEOUT_MS = 5000`, `JS_RUNTIME_HEAP_LIMIT_BYTES = 128 MiB`, advisory `RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS = 25` |
| IPC | `DEFAULT_MAX_FRAME_SIZE = 1 MiB`; client edit/event queues currently hold 256 messages |
| Behavior/UI | behavior manifest 4 KiB; SDUI snapshot 4 KiB; SDUI update 1 KiB; runtime SDUI tree 16 KiB/128 nodes/depth 16; typography 1 KiB |
| Package UI | 128 component nodes; 4 fixed panels; 16 overlays; 64 input routes |
| Parse/render | incremental parse update 4 KiB; first-party grammar windows 4 KiB; decoration set 8 KiB; syntax cache 30 MiB |
| Diagnostics | set 8 KiB/128 spans; client cache 8 MiB |
| Completion | request 512 B; result 16 KiB/256 items; per-provider timeout remains bounded by validated metadata |
| Language intelligence | request 512 B; result 16 KiB; document window 64 KiB; 16 outstanding; 500 ms default/5000 ms maximum |
| Analysis workers | 4 workers; 32 documents/worker; 256 KiB/document; 8 MiB mirror/worker; 64-event/2 MiB input; 64-event/512 KiB output; 8 pending requests; 5 s handler; 64 MiB worker heap |
| Language-server process | 16 sessions; 1 MiB message; 64 KiB stderr; 30 s low-level read; analysis shutdown 2 s graceful/5 s total |

Reload evaluation, package resolution/authorization, registry validation, reclassification, parsing, provider execution, worker/process I/O, and client snapshot validation must remain server-first/background work. Ordinary local text application, caret/selection, key routing, paint, layout, scroll, pointer handling, and edit acknowledgement must not wait on reload JavaScript, locks held during candidate evaluation, full-document IPC, or provider cleanup.

Phase 19 still needs a typed aggregate reload-snapshot/grace/lock budget. It should derive that ceiling from representative complete snapshots and the 1 MiB codec cap rather than adding unbounded frames or a configuration knob.

## Security and Authority Boundary

Reload re-evaluates authority; it does not grant authority.

- Module loading remains deny-by-default: curated `clay:*` facades, canonical configuration-root relative modules, and resolver-recorded package-root-confined modules only.
- Package source provenance and user/admin capability grants remain mandatory for bundled and installed packages. Source kind alone grants nothing.
- `init.js` cannot self-grant package/process/workspace authority. Exact `language-server` grants are configuration-only and seal before package code loads.
- Package UI, behavior, syntax, completion, intelligence, diagnostics, and decorations remain inert declarations/results validated for schema, prefix, provenance, permissions, conflicts, ranges, versions, and payloads.
- Executable callback fields, raw ops, native handles/widgets, raw CSS/render callbacks, and client-side package JavaScript remain rejected.
- Selected-file tokens, known workspace roots, canonical document versions, leases, and range locks remain authoritative during reload and stale-edit handling.
- LSP children retain trusted same-user subprocess authority; root/cwd identity is not an OS filesystem/network/process sandbox.
- Candidate failure must retain old grants, services, workers, sessions, registries, and client state. Successful commit must make old-generation grants unusable and perform bounded cleanup without leaking source text, absolute document paths, environment values, tokens, secrets, or raw child output.

## Generic Gaps Required Before Implementation

1. **Staged generation candidate:** validate all `ClayRuntimeEvaluation` parts and provider registrations without mutating active state; commit once or drop the candidate.
2. **One generation identity:** bind runtime, package records, behavior, theme, typography, SDUI/package UI, modes/commands, syntax, providers, workers, and render output to one monotonic server generation.
3. **Live fan-out:** broadcast committed state to every affected connection; lagged clients receive the latest complete snapshot rather than consuming shared intermediate updates.
4. **Atomic client install:** validate one bounded inert snapshot, install behavior/rendering/UI state in one event, then acknowledge that generation.
5. **Stale-version policy:** decide bounded previous-generation edit grace, hard expiry, rejection/correction/resync, and stale asynchronous render/provider output handling. Current exact rejection plus no resync for `InvalidBehaviorVersion` is insufficient.
6. **Scoped lock manager and command trigger:** reuse `LockScope` and `CommandExecution`; evaluate outside the lock and hold only the approved behavior commit lock. Keep the developer helper thin.
7. **Package-UI protocol projection:** serialize only bounded inert package-UI state; never send registry internals, callbacks, grants, or native handles.
8. **Post-commit cleanup:** reuse each coordinator's `cancel_generation`/`cancel_package`, package allowlist revocation, analysis-worker shutdown, and language-server session teardown only after commit succeeds.
9. **Aggregate budgets and instrumentation:** add typed snapshot/grace/queue ceilings and Phase 14/15 observations proving no mixed visual generation and no ordinary typing/ack dependency.

These are reusable lifecycle/protocol primitives. No gap justifies `if rust`, `if markdown`, package-specific reload code, a second parser/provider/renderer, or per-language process handling.

## Approved Phase 19 Semantics

`decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md` resolves gaps 1-6 and 8-9 before implementation. Plan 054 now implements staged server preparation/validation, immediate one-at-a-time reload admission, explicit command routing, generic behavior-scoped compare-and-swap locking, post-commit generation-owned contribution replacement/cleanup, and live `RuntimeStateSnapshot` fan-out with capacity-16 broadcast / latest-snapshot lag recovery / install-ack validation; atomic client installation and stale-edit grace remain later tasks:

- Prepare G2 in a fresh runtime while G1 remains active; serialize attempts, validate all affected snapshots, and acquire `LockScope::Behavior` only for final compare-and-swap commit.
- Broadcast capacity-16, connection-scoped complete snapshots under the 1 MiB frame ceiling. Clients validate/stage snapshots and send `RuntimeGenerationInstalled(G2)`; invalid snapshots receive no acknowledgement. Full atomic client install and fail-closed reconnect remain the next task.
- Retain only G1's inert manifest/metadata for normally valid already-rendered `Edit`/`EditorIntent` operations until that connection acknowledges G2 or the global two-second/256-accepted-transaction ceiling closes. Reject old commands, stale-drop old provider/render work, and recover expired edits with latest runtime state plus canonical document resync.
- Route explicit reload through built-in `clay.runtime.reloadConfiguration` with no default binding, no watcher, and no dedicated reload IPC.
- Treat server commit as the rollback boundary: logically revoke old executable authority at commit, then apply existing bounded worker/session cleanup. Consider diffs/chunking only at 768 KiB payload p95 or 16 ms client-install p95 through a separate reviewed decision.

## Rejected Implementation Shapes

- Mutating V8 globals or module caches in place.
- Treating the existing runtime-service pointer swap as a complete transaction.
- Independent live behavior/theme/typography/SDUI/render messages without generation identity.
- Per-language or per-package Rust reload branches.
- A reload-specific IPC dispatcher that bypasses `CommandExecution`.
- Holding a global/document/behavior lock while evaluating JavaScript or reparsing.
- File watching, polling, debounce state, or a new dependency before an explicit reload command proves insufficient.
- Client-side package JavaScript, callbacks, raw CSS, native widget construction, or direct renderer hooks.
- Unbounded snapshots, queues, stale-generation retention, worker restart loops, or subprocess shutdown waits.
- Claiming language-server children are sandboxed by workspace-root/cwd checks.

## Tests

cargo test --test protocol primitives_docs::
- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- Existing focused baseline: `cargo test --test runtime persistent_runtime_hot_reload::`.

## Related

- [Persistent Runtime Hot Reload](persistent-runtime-hot-reload.md)
- [Earlier Phase 19 Primitive Review](phase19-persistent-runtime-hot-reload-primitive-review.md)
- [Primitive Architecture](primitive-architecture.md)
- [Behavior Manifests](behavior-manifests.md)
- [Package Loading](package-loading.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Decoration Transport](decoration-transport.md)
- [Language Intelligence](language-intelligence.md)
- [Language Server Process Service](language-server-process-service.md)
- `plans/054-Phase19-Hot-Reload-and-Behavior-Update-Semantics.md`
- `decision-logs/2026-07-16-1825-phase19-hot-reload-transaction-and-stale-edit-semantics.md`
