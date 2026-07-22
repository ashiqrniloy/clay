# Phase 18.20 Language Intelligence Primitive Review

## Source

- Plan: `plans/052-Phase18.20-Language-Intelligence-Primitives-and-LSP-Authority.md` (task 2).
- Roadmap: `roadmap.md` Phase 18.20.
- Decision direction: `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md`.
- Patterns: `.agents/skills/project-patterns/references/mode-primitive-first.md`, `authority-boundaries.md`, `extensions-and-ai.md`, `language-capability-sequencing.md`, and `protocol-and-performance.md`.
- Protocol primitives: `src/protocol/decorations.rs`, `diagnostics.rs`, `completion.rs`, and `mod.rs`.
- Server primitives: `src/server/decorations.rs`, `diagnostics.rs`, `completion.rs`, `parse_coordinator.rs`, `command_execution.rs`, `workspace.rs`, `js_runtime.rs`, `git.rs`, and `runtime_sandbox.rs`.
- Package authority: `src/packages/permissions.rs`, `authorization.rs`, `record.rs`, and `service.rs`.
- Client/UI primitives: `src/client/mod.rs`, `src/editor/surface.rs`, `src/masonry_editor.rs`, and `src/shell/transient_menu.rs`.
- Budgets: `src/perf/budgets.rs`; protocol framing: `src/protocol/codec.rs`.
- Tests: `tests/range_diagnostics.rs`, `completion_provider.rs`, `editor_performance_invariants.rs`, `performance_protocol.rs`, and `primitives_docs.rs`.

## Overview

Phase 18.20 needs four analyzer-neutral interaction primitives—hover, go-to-definition, code actions, and signature help—and a separately authorized process boundary for future LSP bridge packages. Most supporting machinery already exists. Clay already owns byte-offset document identity/versioning, semantic decorations, diagnostics, completion scheduling, command execution, transient menus, workspace-root validation, package provenance, and token-backed server-side JavaScript handlers.

The missing work is narrow: one generic language-intelligence request/result family, one feature-tagged provider lane, minimal projection onto existing UI/navigation primitives, and one deny-by-default language-server session primitive. LSP wire types, JSON-RPC, position-encoding conversion, initialization capabilities, and server-specific behavior belong in Phase 18.21 packages, not Clay core.

This review records architecture constraints only. It adds no runtime behavior and does not approve process authority. The separate Phase 18.20 decision task must define and receive explicit approval for the exact `language-server` grant and containment semantics before process code starts.

## Existing Primitive Inventory

### Text vocabulary and semantic decorations

`src/protocol/decorations.rs` owns `TokenType`, `Modifiers`, `DecorationKind { Syntax, Semantic, Diagnostic, SearchMatch }`, `DecorationSpan`, and `DecorationSet`. The LSP base semantic-token vocabulary plus Clay prose extensions already forms the canonical style vocabulary. `src/server/decorations.rs` validates document version, viewport byte ranges, package provenance, `render-decorations`, and `DECORATION_PAYLOAD_BUDGET_BYTES`. Client paint composes cached syntax and semantic runs additively through `StyleRegistry`; no new semantic renderer or theme vocabulary is needed.

### Range diagnostics

`src/protocol/diagnostics.rs` owns `DiagnosticSpan`, `DiagnosticSet`, and source-keyed replacement. `src/server/diagnostics.rs` validates current versions, byte ranges, provenance, field/count/payload limits, and `render-decorations`. `EditorDiagnosticState` retains near-viewport chunks and native paint draws theme-owned squiggles. Future LSP diagnostics map onto this primitive; they do not need another protocol, cache, scheduler, or renderer.

### Completion requests/results

`src/protocol/completion.rs` owns versioned `CompletionRequest`, `CompletionResultSet`, `CompletionItem`, provenance, request IDs, provider generations, status/rejection values, and payload validation. `src/server/completion.rs` owns `CompletionProviderRegistry` and the cancellable UI-reactive `CompletionCoordinator`. `src/shell/transient_menu.rs` projects results into the generic picker, while `EditorSurface` applies validated plain-text or bounded snippet replacements locally. LSP completion maps onto this existing framework under `completion-provider`.

### Command execution and transient menus

`src/server/command_execution.rs` is the single server-owned validation path for registered/built-in command intents. It rechecks routing, provenance, permissions, target, and bounded arguments. `TransientMenuSession` is bounded inert query/selection/status state with command or completion actions; it already supports modal/modeless bottom overlays and accessibility labels. Definitions and code-action choices can reuse this session. Command-backed code actions can reuse `CommandExecution`; direct code-action edits remain inert previews in Phase 18.20.

### Workspace roots and navigation

`src/server/workspace.rs::WorkspaceState` owns canonical workspace roots, single-file grants, document/path identity, root-relative display paths, traversal rejection, and validated file opening. `canonical_file_state` re-canonicalizes and rejects paths outside the selected root. Go-to-definition locations can therefore use an open `DocumentId` or `WorkspaceRootId` plus normalized relative path and byte range. Raw absolute paths and external `file://` URIs are not canonical Clay locations.

### Document identity/version state

`src/protocol/mod.rs` defines `DocumentId`, `DocumentVersion`, `BehaviorVersion`, `ClientId`, `WorkspaceRootId`, and `DocumentMetadata`. Existing decoration, diagnostic, completion, parse, and edit flows stale-drop mismatched versions. New language-intelligence requests must carry request/client/document/version/behavior/provider-generation metadata and a UTF-8 byte offset or range. Clay canonical positions are byte offsets, never LSP line/character values.

### Package permissions and authorization

Task-2 baseline found 19 permissions and no `language-server`. Task 4 now adds deny-by-default `language-server`, fixed `LanguageServerContributionDescriptor`, and exact `LanguageServerGrant` metadata after approval of `decision-logs/2026-07-14-2023-language-server-package-authority.md`. `PackageAuthorizationRecord` still binds package-wide capabilities to identity/source/version/API prefix/runtime profile/approver; the scoped grant additionally binds contribution fingerprint, canonical executable, inherited-environment declaration, and known directory-root IDs. `PackageService` fails closed when either layer is missing/stale. Relevant permissions are:

- `parse-document` for bounded Clay-provided open-document analysis and token-backed handlers;
- `render-decorations` for semantic decorations and diagnostics;
- `completion-provider` for completion providers;
- `command-registration` plus command-specific checks for command-backed code actions.

A generic language-intelligence provider that receives only bounded Clay-provided open-document data should reuse `parse-document`; it does not inherit process, filesystem, network, shell, workspace-mutation, or command authority. A process-backed provider must separately hold the future approved `language-server` grant. Output-specific permissions remain required and are not bypassed by that grant.

### Persistent runtime handler tokens

`src/server/parse_coordinator.rs::JsParseHandlerRegistration` stores a resolver-validated package, metadata, runtime-issued token, parse unit, and timeout—not a raw JavaScript callback. `ParseCoordinator` owns cancellation, generations, current versions, stale-result rejection, and result/diagnostic channels. `src/server/js_runtime.rs` invokes tokens on the persistent server runtime with timeout, heap, import, and sanitized-diagnostic boundaries. New package language-intelligence handlers should reuse this token/module boundary and lifecycle rather than carrying function values or creating another JavaScript runtime.

### Existing process precedents

Two internal services provide reusable implementation patterns but no package process authority:

- `RuntimeSandboxSupervisor` in `src/server/runtime_sandbox.rs`: persistent `tokio::process::Command`, piped stdin/stdout, bounded line-framed JSON, handshake, timeout, kill-on-timeout, and `kill_on_drop`.
- `GitDiscoveryService` in `src/server/git.rs`: closed command/argv table, known-workspace cwd, capped async stdout/stderr readers, timeout, `kill_on_drop`, and sanitized diagnostics.

Neither is a generic process API. `PackageManagerBackend` uses `std::process::Command` only for the host-owned pnpm boundary and is not reusable package authority. Tokio process support already resolves at version 1.52.2, so Phase 18.20 needs no new process dependency.

## What Existing Primitives Already Achieve

Without new LSP types or language-specific Rust code, Clay can already:

- accept additive semantic tokens as `DecorationSet { kind: Semantic }`, map them through `TokenType + Modifiers`, and paint them with existing syntax;
- publish analyzer diagnostics as source-keyed `DiagnosticSet` chunks;
- schedule, cancel, stale-drop, display, and accept completion results;
- display bounded result lists and status text through `TransientMenuSession`;
- execute command-backed actions through one permission/provenance boundary;
- navigate/open files only through canonical known workspace roots or explicit single-file grants;
- invoke resolver-validated package modules through runtime-issued handler tokens;
- bind package capabilities to identity/source/version and fail closed on missing grants;
- run internal bounded child processes asynchronously without shell strings.

Semantic tokens, diagnostics, and completion are therefore mappings onto existing primitives. Phase 18.20 must not replace or fork them.

## Generic Phase 18.20 Gaps

### `LanguageIntelligenceRequestAndResult`

Add one engine-neutral protocol family for four feature kinds:

- `Hover`: optional byte range plus bounded Markdown/plain-text content;
- `GoToDefinition`: bounded ordered `TextLocation` values;
- `CodeAction`: source byte range, title, optional registered command ID, and optional inert versioned edit preview;
- `SignatureHelp`: bounded signatures/parameters and validated active indexes.

Every request/result carries request ID, client/document/version/behavior/provider generation, feature kind, package provenance, and status. `TextLocation` is either an open document or a known workspace root + normalized relative path, plus a UTF-8 byte range. Empty, timeout, cancelled, stale, and provider-error outcomes are typed and deterministic.

No LSP `Position`, `Range`, `Location`, URI, JSON-RPC ID, method name, or negotiated encoding enters this canonical model. An LSP bridge resolves line/character positions against the exact document version and negotiated UTF-8/UTF-16/UTF-32 encoding before constructing Clay byte offsets.

### `LanguageIntelligenceProvider`

Add one feature-tagged provider registry/coordinator, not one scheduler per feature. Provider metadata records package provenance, package-owned provider ID, modes, supported feature kinds, priority, timeout, result caps, and runtime generation. Resolver-validated package modules register by module/export metadata and runtime-issued token; no callback object crosses the op boundary.

The lane reuses completion/parse lifecycle rules: deterministic selection, bounded queues, cancellation, request/provider generation, current document/behavior version checks, timeout, sanitized errors, and stale-result discard. A newer edit, cursor request, package withdrawal/reload, or provider generation supersedes older work. Built-in/non-LSP fake analyzers and future process-backed LSP adapters use the same registry.

### Minimal request and presentation composition

Add built-in discoverable commands for hover, definition, code actions, and signature help. Client intent captures current document/version/cursor after local state is available and enqueues work without blocking paint. Existing UI primitives cover presentation:

- hover/signature help: bounded modeless transient text/status projection;
- multiple definitions/code actions: `TransientMenuSession`;
- selected definition: validated workspace/document navigation;
- command-backed code action: existing `CommandExecution`;
- direct edit: preview only in Phase 18.20, never applied automatically.

No language-intelligence-specific Masonry widget is required.

### `LanguageServerSession`

Add one host-owned, opaque async session service after the now-approved authority decision and Task 4's exact configuration grant boundary. It must start a package-declared and user-approved fixed server contribution for an approved workspace root; package runtime input must not choose executable, argv, cwd, shell, or unrestricted environment. Session operations are bounded start/read/write/stop calls tied to package/grant/runtime generation and cleaned up on timeout, exit, revocation, package withdrawal/reload, runtime replacement, and server shutdown.

This primitive is an opaque byte/message process conduit. LSP `Content-Length` framing, initialization, capabilities, cancellation messages, document synchronization, and server-specific policy stay in Phase 18.21 bridge packages.

The authority decision must state containment truthfully. Setting `current_dir` to a workspace root constrains Clay's launch contract but does not OS-confine a same-user child process. This review does not label that child sandboxed or filesystem-confined.

## Lifecycle and Data Flow

```text
local edit/paint completes
  -> client command captures document/version/cursor byte offset
  -> LanguageIntelligenceRequest (bounded, UI-reactive)
  -> generic provider selection by mode/feature/priority/provenance
  -> resolver-validated package handler token
       -> optional separately-authorized LanguageServerSession
       -> package-side LSP conversion (Phase 18.21)
  -> LanguageIntelligenceResult validation
  -> stale/generation/version check
  -> existing transient menu/status/navigation/command projection
```

Semantic, diagnostic, and completion data branch onto existing paths instead:

```text
LSP SemanticTokens -> DecorationSet(kind = Semantic)
LSP Diagnostic     -> DiagnosticSet
LSP CompletionItem -> CompletionResultSet
```

No provider/process/JavaScript/IPC wait occurs before local text paint.

## Budgets and Hot-Path Policy

Existing hard limits remain authoritative:

- `DECORATION_PAYLOAD_BUDGET_BYTES` = 8 KiB;
- `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES` = 8 KiB and `DIAGNOSTIC_MAX_SPANS_PER_SET` = 128;
- `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` = 4 KiB;
- `COMPLETION_REQUEST_PAYLOAD_BUDGET_BYTES` = 512 B;
- `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES` = 16 KiB and `COMPLETION_RESULT_MAX_ITEMS` = 256;
- `TRANSIENT_MENU_MAX_ITEMS` = 256;
- `DEFAULT_MAX_FRAME_SIZE` = 1 MiB.

Phase 18.20 must add typed constants for language-intelligence request/result payloads, per-feature item/string/nesting limits, outstanding requests, provider timeout, process/session count, process message size, and stderr diagnostics. Representative `rkyv` payload tests must sit below the IPC frame ceiling.

| Work | Lane |
| --- | --- |
| Local edit, cursor, selection, paint | Client-first/local; never waits on provider, process, JavaScript, or IPC |
| Hover/definition/action/signature request | Cancellable `UiReactivePriority` work |
| Semantic/diagnostic refresh | Cancellable viewport-prioritized `Background` work |
| Completion | Existing cancellable `UiReactivePriority` lane |
| Provider/process registration and authorization | Configuration/package-load time |
| Process start/read/write/stop | Async server work outside client hot paths |
| Result paint | Cached validated inert state only |

## Security and Authority Boundary

Provider authority and process authority are separate:

1. Provider registration under `parse-document` may receive only bounded Clay-provided open-document data and return inert validated results.
2. `render-decorations`, `completion-provider`, and command-specific permissions remain required for their existing output/action paths.
3. `language-server` is a deny-by-default process capability requiring package declaration, explicit pre-load `init.js` grant, exact package/server contribution/workspace scope, and approved decision `2026-07-14-2023`; bundled trust/load never auto-grants it.
4. `language-server` must not imply generic shell, arbitrary executable/argv, network API, raw filesystem API, workspace mutation, package control, raw ops, native UI, or client runtime.

Reject generic shell strings, arbitrary runtime argv, external URIs, raw absolute filesystem paths, path traversal, raw ops, client JavaScript, callbacks, native handles, raw CSS/HTML, source/secret leakage, unvalidated command targets, and direct/unvalidated action edits. Process stderr/errors are bounded and sanitized. Every result retains package provenance and exact document/version metadata.

## Rejected Implementation Shapes

- **LSP-shaped Rust core:** no `lsp-types`, JSON-RPC methods, LSP URIs, server capabilities, or initialization state in canonical Clay protocol types.
- **Per-language branches:** no Rust/TypeScript/JavaScript/Markdown-specific provider, navigation, process, or renderer branch.
- **Second diagnostics/completion renderer:** semantic, diagnostic, and completion LSP responses reuse `DecorationSet`, `DiagnosticSet`, and `CompletionResultSet`.
- **New menu widget:** hover/signature/definition/action projection reuses existing transient overlay/menu/component state.
- **Raw line/UTF-16 positions:** package adapters convert negotiated LSP positions to Clay UTF-8 byte offsets against an exact version.
- **Generic process or shell facade:** language-server sessions start only approved fixed contributions.
- **Automatic code-action edits:** Phase 18.20 carries inert previews; mutation needs existing command/edit authority and explicit user action.
- **False sandbox claims:** cwd/workspace validation is not OS filesystem confinement.

## Tests

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- Final implementation and verification are documented in [Language Intelligence](language-intelligence.md) and [Language Server Process Service](language-server-process-service.md), including protocol round trips, provider cancellation/staleness, UI projection, workspace navigation, deny-by-default process grants, fixed launch descriptors, bounded process I/O, cleanup, and semantic/diagnostic/completion reuse tests.

Run this review gate with:

```bash
cargo test --test protocol primitives_docs::
```

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Phase 18.19 Completion Extensions Primitive Review](phase18.19-completion-extensions-primitive-review.md)
- [Range Diagnostics](range-diagnostics.md)
- [Completion Snippet Expansion](completion-snippet-expansion.md)
- [Decoration Transport](decoration-transport.md)
- [Command Registry](command-registry.md)
- [Transient Menu Session](transient-menu-session.md)
- [Workspace Discovery and File Browser](workspace-file-browser.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Git Discovery Service](git-discovery-service.md)
- [Language Intelligence](language-intelligence.md)
- [Language Server Process Service](language-server-process-service.md)
- [Package Primitive Security](../../reference/primitives/package-security.md)
