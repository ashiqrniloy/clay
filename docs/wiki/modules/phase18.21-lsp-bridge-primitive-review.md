# Phase 18.21 LSP Bridge Primitive Review (Historical Baseline)

> This page records the pre-implementation primitive inventory used by Plan 053. Current process/session behavior is documented in [Language Server Process Service](language-server-process-service.md), current analysis/provider behavior in [Language Intelligence](language-intelligence.md), and current two-domain runtime behavior in [Embedded JavaScript Runtime](embedded-js-runtime.md). References below to one runtime, missing close lifecycle, central-router head-of-line blocking, or unbounded parse channels describe the historical gap, not current behavior.

## Source

- Plan: `plans/053-Phase18.21-First-Party-LSP-Bridge-Packages.md` (task 2).
- Roadmap: `roadmap.md` Phase 18.21.
- Process authority: `decision-logs/2026-07-14-2023-language-server-package-authority.md`.
- Patterns: `.agents/skills/project-patterns/references/mode-primitive-first.md`, `language-capability-sequencing.md`, `protocol-and-performance.md`, `extensions-and-ai.md`, and `authority-boundaries.md`.
- Primitive references: `docs/reference/primitives/language-intelligence.md`, `diagnostics.md`, `parse-update-strategy.md`, `package-security.md`, `registry.md`, and `backlog.md`.
- Process/session: `src/server/language_server.rs`, `src/server/ops/language_server.rs`, and `runtime/js/language-server.js`.
- Runtime/module boundary: `src/server/js_runtime/mod.rs`, `src/server/ops/mod.rs`, and `src/server/ops/packages.rs`.
- Analysis/provider paths: `src/server/parse_coordinator.rs`, `language_intelligence.rs`, `completion.rs`, `decorations.rs`, and `diagnostics.rs`.
- Protocol shapes: `src/protocol/parse.rs`, `language_intelligence.rs`, `completion.rs`, `decorations.rs`, and `diagnostics.rs`.
- Document/workspace/command flow: `src/server/connection/mod.rs`, `document.rs`, `workspace.rs`, and `command_execution.rs`.
- JavaScript facades: `runtime/js/{language,completion,parse,decorations,diagnostics,language-server}.js` with adjacent `.d.ts` declarations.
- Base packages: `packages/{rust,typescript,javascript,markdown}/{package.json,dist/load.js}`.
- Tests: `tests/language_server_authority.rs`, `language_intelligence.rs`, `completion_provider.rs`, `parse_coordinator.rs`, `decoration_transport.rs`, `range_diagnostics.rs`, `editor_performance_invariants.rs`, and `primitives_docs.rs`.

## Overview

Phase 18.21 needs package-owned LSP adapters, not a second language-intelligence subsystem. Existing Clay primitives already own fixed language-server process authority, analyzer-neutral request/result validation, semantic decoration transport, source-keyed diagnostics, completion result presentation, workspace navigation, commands, package provenance, and base-language fallback.

Four generic gaps block correct bridge packages:

1. exact byte transport for opaque child stdout/stdin;
2. an approved bounded document-analysis worker lifecycle carrying open/change/close state from server-canonical documents;
3. a resolver-validated dynamic package completion adapter into the existing completion coordinator;
4. a long-lived worker output path into existing semantic/diagnostic/result validators.

Only these gaps should add Rust capability. LSP framing, JSON-RPC, methods, capabilities, position/URI conversion, semantic-token delta decoding, and server-specific policy remain package JavaScript. Task 3 must approve the full-document and long-lived-worker authority before gaps 2–4 are implemented.

## Existing Primitive Inventory

### `LanguageServerSession` and exact grants

`LanguageServerProcessService` already provides one generic process boundary. `authorizeLanguageServer` records an exact pre-load grant for package, fixed contribution fingerprint, canonical executable, literal argv, declared inherited-environment names, and known directory workspace roots. `loadPackage` seals grant mutation before package code executes. `startLanguageServerSession` revalidates the current descriptor/grant and lets runtime code choose only an approved contribution and root.

Child I/O runs on the dedicated language-server router, outside the Deno worker and client hot paths. The router enforces one session per package/contribution/root identity, a global 16-session cap, 256 KiB message cap, 64 KiB stderr cap, timeout, direct spawn, `env_clear`, piped stdio, `kill_on_drop`, identity checks on every operation, explicit stop, and service-shutdown cleanup. A tested `revoke_for_package` hook exists, but the public runtime package-disable facade remains planned, so later worker/session lifecycle work must wire withdrawal rather than assume it. The service exposes no child, PID, stdio, shell, cwd override, environment values, or runtime-selected argv.

What it can already do: safely start and stop each selected fixed server and move bounded opaque data.

What it cannot safely do: `SessionCommand::Read` and `LanguageServerProcessService::read` return `String`; `handle_read` applies `String::from_utf8_lossy` to each arbitrary stdout chunk. LSP `Content-Length` counts bytes, and a child read may split one UTF-8 code point or contain partial/multiple frames. The current `send(string)` path also makes text the public low-level contract. Correct framing therefore requires generic exact `Uint8Array`-style `sendBytes`/`readBytes`; Rust must still not parse LSP headers.

### Persistent runtime and module allowlist

`ClayJsRuntimeService` owns one persistent V8 isolate on one `clay-js-runtime` thread per generation. `RuntimeCommand::{Evaluate, Parse, LanguageIntelligence}` are consumed sequentially. Parse and language-intelligence callbacks use resolver-validated module exports stored behind runtime-issued tokens; Rust never receives JavaScript function values.

`ClayModuleLoader` permits curated `clay:*` facades, configuration-root-confined relative `.js` modules, resolver-recorded package `loadEntry` modules and package-root-confined relative imports, plus the existing vendored `markdown-it` exception. Unknown URLs, external package imports, traversal, and arbitrary filesystem modules fail closed.

This is enough for short bounded request handlers. It is not a long-lived bridge host: an endless child-read promise would keep one runtime command active, serialize unrelated parse/intelligence work behind it, and eventually hit timeout/worker-poison handling. `std::sync::mpsc::channel` also gives the runtime command ingress no per-document bridge backpressure contract. A bridge therefore needs an explicitly approved bounded worker lifecycle rather than an evaluation that never completes.

### Parse windows and document analysis

`ParseCoordinator` already supplies server-canonical, UTF-8-safe, versioned, provenance-bound `ParseWindowSnapshot` values to token-backed package handlers. It validates window lengths, package/mode/document/version identity, policy and memory limits, cancellation generations, stale results, decoration/diagnostic side channels, and the 4 KiB incremental-update envelope. Open and viewport scheduling return before parse completion.

This does not provide LSP synchronization:

- `ParseEditNotification` has viewport/invalidated ranges and optional windows, but no open/change/close event kind, base version, exact accepted edit operation, or workspace-root lifecycle.
- First-party grammar policies cap each supplied window at 4 KiB; the fallback opening policy is at most 64 KiB. `schedule_parse_window` truncates the requested document range to `max_window_bytes`.
- The ordinary `ClientMessage::Edit` / `EditorIntent` path applies the canonical edit and replies, but does not emit an ordered package document event carrying that accepted operation.
- Protocol and workspace code has open/reload messages but no generic document-close message/event for package analysis state.
- Parse output and runtime-diagnostic receivers currently use unbounded channels; they are not an approved queue contract for long-lived subprocess traffic.

Parse windows remain correct for syntax work and bounded intelligence context. They must not be stretched or looped into a fake `didOpen`: LSP requires one coherent initial text snapshot, and servers must see accepted unsaved changes in version order. Task 3 must approve a separate generic bounded document-analysis lifecycle and an oversize fallback ceiling.

### Document open/edit/reload flow

`DocumentState` owns the canonical rope, version checks, lease checks, and accepted edit ordering. `src/server/connection/mod.rs` handles open paths through `WorkspaceState`, sends `DocumentOpened`, classifies/activates the mode, and schedules background syntax. Edits call `DocumentState::apply_edit` before `EditAck`; client local paint remains optimistic and does not wait for package JavaScript or a child process. Reload returns a new canonical snapshot and reruns generic open follow-up activation.

Future bridge events must attach only after these authoritative transitions: initial snapshot after successful open/activation; delta after canonical acceptance/version increment; close/reload/root removal through explicit lifecycle events. They must never consume unvalidated client intent or run before local paint.

### Semantic decorations

`DecorationSet` already carries versioned viewport-bounded `DecorationKind::Semantic` spans using `TokenType + Modifiers`. Server validation checks `render-decorations`, provenance, version, range, vocabulary, payload, and cache limits. Client paint composes semantic spans additively over syntax through cached inert state and `StyleRegistry`.

No new semantic-token protocol, cache, theme mapper, renderer, or paint branch is needed. Package code must decode the negotiated LSP legend/delta and convert positions, then publish existing spans.

Current publication is not a live worker channel. `ClayOpState::published_decoration_set` is one `Option`, reset by `begin_evaluation`, and harvested only when an evaluation command completes. It cannot carry an arbitrary stream of server notifications while a bridge worker remains active.

### Range diagnostics

`DiagnosticSet` already carries inert severity/code/message/source data with source-keyed replacement. Validation enforces `render-decorations`, package provenance, exact version, viewport ranges, field/count/payload limits, and an 8 MiB client cache. Empty source chunks clear only that source; client paint uses theme-owned squiggles without erasing syntax or semantics.

No second LSP diagnostic transport or renderer is needed. Tree-sitter recovery nodes remain non-authoritative; later composition may suppress only overlapping recovery noise.

Current runtime publication is likewise not a notification stream. `published_diagnostic_set` is one evaluation result slot, not a bounded multi-event worker output queue. Long-lived bridges need server-stamped, versioned outputs routed through the existing diagnostic validator/cache/publication path.

### Completion

`src/protocol/completion.rs`, `CompletionProviderRegistry`, and `CompletionCoordinator` already support bounded versioned requests/results, provider priority, optional exclusive claim, disabled-provider filtering, cancellation, timeout, generations, stale rejection, inert snippets, and `TransientMenuSession` presentation. The generic Rust registry already accepts executable `CompletionProvider` implementations.

The package-facing path is still static-only. `serverRegisterCompletionProvider` rejects `module`, `handler`, and callback fields; `completion_provider_metas` stores manifest-declared `items`; `static_package_completion_result` prefix-filters those items directly in `connection/mod.rs`. `JsCompletionProviderRegistration` exists only as an unused shape. LSP completion therefore needs one narrow resolver-validated module/export token adapter into the existing coordinator, not another coordinator, result type, menu, snippet parser, or per-language provider path.

### Language-intelligence providers

`LanguageIntelligenceRequestAndResult` and `LanguageIntelligenceCoordinator` already cover hover, definition, code action, and signature help with UTF-8 byte offsets, exact versions, feature/mode/priority selection, package provenance, bounded document windows, cancellation, timeout, generation checks, inert edit previews, and stale rejection. Package providers already register through resolver-validated module/export tokens.

No new feature scheduler is needed. Bridge workers should answer these existing request envelopes after package-side LSP conversion. The current per-request runtime call receives at most a 64 KiB context window and cannot itself establish synchronized full-document server state or drain unsolicited child notifications.

### Workspace roots and navigation

`WorkspaceState` owns canonical directory roots, selected-file grants, root-relative paths, open-document identity, traversal rejection, and validated opening/reloading. Definition locations already normalize to an open `DocumentId` or known `WorkspaceRootId` plus safe relative path and byte range. `CommandExecutor` routes workspace opens through that same authority.

No generic URI or filesystem primitive is missing. Package adapters must convert `file://` locations to these existing identities and reject external schemes, unknown roots, absolute paths, and traversal. Child cwd remains launch/audit metadata, not OS confinement.

### Commands and code actions

`CommandRegistry`, `CommandExecutor`, and `TransientMenuSession` already provide bounded, provenance-checked, permission-checked server-first command execution and result selection. Code actions already support inert titles, registered command IDs, and versioned `EditPreview`; direct workspace edits are not applied.

No LSP command dispatcher or edit authority is needed. Mutating rename/refactor/format/import-management remains deferred.

### Package loading, provenance, and revocation

`loadPackage` resolves installed packages, validates manifests and permissions, records package-root-confined load entries, seals language-server authority, and evaluates one-line package load entries in the persistent runtime. Runtime generations and provider/parse generations already provide stale-work boundaries. `PackageService` has internal disable/revoke accounting, while the public runtime disable facade remains planned; later worker implementation must connect approved package withdrawal/reload/root-removal/shutdown events to worker/session/output cleanup rather than inventing package-specific cleanup.

The four base packages are already complete no-LSP fallbacks:

| Package | Existing behavior | LSP bridge implication |
| --- | --- | --- |
| `@clay/rust` | Rust mode, Tier 1 syntax, behavior manifest, command, keyword/snippet completion, status | Separate `@clay/lsp-rust`; base package stays usable without grant/executable. |
| `@clay/typescript` | TS/TSX/MTS/CTS mode, Tier 1 syntax, behavior, command, keyword/snippet completion, status | Separate `@clay/lsp-typescript`; do not replace static/base providers. |
| `@clay/javascript` | JS/JSX/MJS/CJS mode, Tier 1 syntax, behavior, command, keyword completion, status | Separate `@clay/lsp-javascript`; share package adapter source, not identity. |
| `@clay/markdown` | Markdown mode, Tier 1 syntax, Tier 3 parser fallback, behavior, commands, completion, status, independent preview | Separate `@clay/lsp-markdown`; Marksman must not own preview or syntax fallback. |

All four currently request only mode, command, completion, parse, and decoration permissions. None has `language-server`; bridge packages must carry that authority separately.

## What Existing Primitives Already Achieve

Without new LSP-shaped Rust code, Clay can already:

- authorize and launch one fixed server contribution for an approved root;
- register and invoke dynamic hover/definition/code-action/signature providers;
- validate/publish semantic decorations and source-keyed diagnostics;
- validate, merge, display, and accept completion/snippet results;
- navigate only through open documents and known workspace-root-relative paths;
- execute registered command-backed actions and retain direct edits as inert previews;
- bind every registration/result/session to package and generation provenance;
- keep Tier 1 syntax, base completion, behavior, and Markdown preview operational with no bridge.

Bridge implementation must reuse these paths.

## Locked Generic Gaps (All Resolved)

All four gaps identified by this review have been resolved by tasks 4–6 of Plan 053.

### 1. Lossless bounded session bytes — Resolved (Task 4)

Extend the existing `LanguageServerSession` with exact bounded byte send/read. Preserve every existing grant/session identity check, direct process policy, timeout, count/message/stderr cap, and cleanup rule. Package code owns buffering and `Content-Length` framing. Do not add an LSP parser or second process service in Rust.

**Implementation**: `SessionCommand::Read` now returns `Result<Vec<u8>>`, two deno ops (`op_clay_language_server_send_bytes` with `#[buffer]` input, `op_clay_language_server_read_bytes` returning `#[buffer] Vec<u8>`) expose `sendBytes`/`readBytes` as `Uint8Array`, message budget raised to 1 MiB. Hot-path code is forbidden from using byte ops. Lossless split-UTF-8 round-trip, fragmented LSP frame reassembly, and oversize rejection confirmed by `tests/language_server_authority.rs`.

### 2. Bounded document-analysis worker lifecycle — Resolved (Task 3 Decision + Tasks 4–5)

After task 3 approval, add one analyzer-neutral package worker contract bound to package, contribution, workspace root, runtime generation, and open document identity. It must receive a coherent bounded initial server-canonical snapshot, ordered accepted deltas, reload/resync, close, root removal, revocation, runtime replacement, and shutdown. Input/output queues, document-size ceiling, worker count/heap/time, pending requests, and cleanup require explicit approved limits.

This is new full-document/package-worker authority and cannot be inferred from `parse-document` windows. Oversize documents must never enter partial child state; they retain base mode/syntax/completion and receive one bounded sanitized status.

**Implementation**: `DocumentAnalysisCoordinator` in `src/server/document_analysis.rs` with max 4 workers, 32 documents/worker, 8 MiB text/worker, bounded input mailbox (64 deltas/2 MiB) with `coalesce_reset`, bounded output channel (64 events/512 KiB), lazy spawn on first open, stop after last close with 2s graceful + 5s kill. Decision contract approved as `decision-logs/2026-07-15-1750-phase18.21-package-worker-authority.md`. 6 unit tests covering lifecycle, revocation, oversize, and cancellation.

### 3. Dynamic package completion adapter — Resolved (Task 5)

Complete the unused generic bridge from a resolver-validated package module/export token to the existing `CompletionProviderRegistry`/`CompletionCoordinator`. Preserve current request/result types, priority/exclusive/disable semantics, cancellation, timeout, item/payload limits, server-stamped provenance, and client-local snippet expansion. Do not add a parallel LSP completion coordinator or static polling workaround.

**Implementation**: `register_completion_provider` op accepts `runtimeBridge: true` with `exportName` (default `"provideCompletion"`); creates `JsCompletionProviderRegistration` stored in `ClayOpState`; JS handlers stored in `globalThis.__clayCompletionHandlers` keyed by token. `CompletionRequest` in `connection/mod.rs` attempts dynamic provider resolution first (package prefix + trigger characters), spawns async with timeout, falls back to `static_package_completion_result`. `document_changed` called on every `EditAck` to abort stale work. LSP bridges register at priority 100 non-exclusive with halving-retry truncation for oversize results.

### 4. Long-lived validated worker outputs — Resolved (Task 5)

Allow the approved worker to emit bounded asynchronous semantic and diagnostic replacements and answer existing completion/intelligence requests without waiting for a configuration evaluation to finish. Route outputs through current decoration, diagnostic, completion, and language-intelligence validators/caches/publication paths. Stamp package/root/generation/document/version provenance server-side; stale, revoked, oversize, malformed, or out-of-root output fails closed.

**Implementation**: `spawn_worker` creates a tokio task consuming mailbox events, `publish_invocation_outputs` routes outputs through `validate_decoration_publication` and `validate_diagnostic_publication` against active document version. `begin_evaluation` now clears both `published_decoration_set` and `published_diagnostic_set`. Output channel saturation sets `output_failed` flag; worker breaks and sends error reply. Stale live output dropped after newer reset.

These four gaps are generic enough for future non-LSP analyzers. No other Rust primitive is justified by this review.

## Authority and Data Flow

```text
server-canonical open/edit/reload/close
  -> approved bounded package/root/generation analysis worker
  -> package-owned LSP framing, JSON-RPC, sync, capability, position/URI policy
  -> exact authorized LanguageServerSession bytes
  -> fixed trusted same-user child

child response/notification
  -> package-owned LSP conversion
  -> existing Clay semantic / diagnostic / completion / intelligence shapes
  -> existing validators, caches, stale checks, protocol, and native UI
```

Authority remains split:

- Rust server owns canonical documents, versions, accepted-edit ordering, worker/session lifecycle, grants, provenance stamping, budgets, validation, publication, and cleanup.
- Package JavaScript owns LSP framing, JSON-RPC state, capabilities, method policy, synchronization messages, cancellation messages, position/URI conversion, and server-specific initialization.
- Child owns language analysis under same-user OS authority. It is trusted subprocess authority, not workspace/filesystem/network/process confinement.
- Rust client owns local input, optimistic shadow text, rendering, viewport, selection, transient UI state, and inert result projection. It executes no package JavaScript.

## Budgets and Hot-Path Policy

Existing hard limits remain:

| Surface | Current limit |
| --- | --- |
| Language-server message / stderr / sessions / read | 256 KiB / 64 KiB / 16 / 30 s |
| Language-intelligence request / result / context | 512 B / 16 KiB / 64 KiB |
| Intelligence outstanding / default / maximum timeout | 16 / 500 ms / 5 s |
| Completion request / result / items | 512 B / 16 KiB / 256 |
| Decoration / diagnostic set | 8 KiB / 8 KiB |
| Diagnostic spans / retained cache | 128 / 8 MiB |
| Incremental parse update / syntax cache | 4 KiB / 30 MiB |
| First-party parse window | 4 KiB |
| JavaScript heap / evaluation timeout | 128 MiB / 5 s |
| IPC frame | 1 MiB |

Task 3 must choose, document, and approve any new snapshot, delta, event queue, output queue, worker, or synchronized-document limits. This review deliberately does not invent values.

Hot-path classification is fixed:

- local edit application, caret/selection, paint, layout, scroll, and native text events never wait for process, JavaScript, worker, or IPC;
- accepted document events enqueue only after canonical commit and edit acknowledgement remains independent of worker progress;
- hover/definition/code-action/signature and completion remain cancellable UI-reactive work;
- semantic and diagnostic refresh remains background/viewport-bounded publication;
- process start/read/write/stop and worker execution stay server-side;
- paint consumes only cached validated inert state.

## Security Boundary

The worker and bridge must not gain raw filesystem reads, network APIs, shell, arbitrary process launch, runtime-selected executable/argv/cwd/environment, package-manager installation, raw ops, client JavaScript, native widgets, raw CSS/HTML execution, AI authority, workspace mutation, or direct edit application.

Document content may cross only after task 3 approves exact full-snapshot/delta scope under package provenance and required permissions. Process grants remain exact and pre-load. Output permissions remain separate: `language-server` does not bypass `parse-document`, `render-decorations`, `completion-provider`, command checks, workspace-root validation, or result budgets.

Runtime module resolution remains package-root confined. Adapters must reject malformed frames, unsupported server requests, external/out-of-root URIs, stale versions, forged provenance, mutating `WorkspaceEdit`, and unsupported capabilities. Diagnostics must not leak document text, environment values, absolute paths, tokens, or unbounded child stderr.

## Rejected Implementation Shapes

- LSP types, method strings, JSON-RPC IDs, `Content-Length`, capabilities, positions, or URIs in Rust core.
- Per-language Rust branches or one process/provider/renderer path per bridge package.
- Four independent protocol adapters instead of one package-owned shared adapter.
- A second semantic renderer, diagnostic cache, completion coordinator, language-intelligence scheduler, navigation path, command dispatcher, or process service.
- Lossy string framing, base64/JSON byte amplification when exact typed bytes are available, parse-window concatenation pretending to be full `didOpen`, or child filesystem rereads pretending to synchronize unsaved buffers.
- An endless child-read loop inside configuration evaluation or the existing sequential runtime command worker.
- Unbounded worker/event/output queues or reuse of current unbounded parse channels as the approved bridge queue contract.
- Direct package calls to raw ops, direct filesystem reads, process handles, or publication that bypasses existing validators.
- Automatic language-server install, implicit first-party grant, silent package default, fake unsupported results, direct workspace edits, rename/refactor/format/import authority, or sandbox claims.

## Test Strategy

Task 4 should add byte-exact split-UTF-8, fragmented/multiple frame transport, cap, timeout, exit, identity, and cleanup tests to `tests/language_server_authority.rs`.

After task 3 approval, the worker task should add one generic fake adapter test path covering initial snapshot, ordered accepted deltas, reload/close/revoke/root-removal/runtime-replacement/shutdown, bounded queues, oversize fallback, dynamic completion, live semantic/diagnostic publication, stale/generation rejection, and no hot-path wait. Existing focused suites remain the regression owners for output shape and UI behavior.

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.20 Language Intelligence Primitive Review](phase18.20-language-intelligence-primitive-review.md)
- [Language Intelligence](language-intelligence.md)
- [Language Server Process Service](language-server-process-service.md)
- [Parse Coordinator](parse-coordinator.md)
- [Completion Snippet Expansion](completion-snippet-expansion.md)
- [Decoration Transport](decoration-transport.md)
- [Range Diagnostics](range-diagnostics.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [First-Party Language Packages](first-party-language-packages.md)
- [LSP 3.17 Bridge Contract](../../reference/primitives/language-intelligence.md)
