# Language Intelligence

## Source

- `src/protocol/language_intelligence.rs`
- `src/server/language_intelligence.rs`
- `src/server/ops/language_intelligence.rs`
- `src/server/js_runtime.rs`
- `runtime/js/language.ts`
- `src/server/connection.rs`
- `src/client/behavior.rs`, `src/client/mod.rs`
- `src/editor/surface.rs`, `src/masonry_editor.rs`
- `src/shell/transient_menu.rs`
- `tests/language_intelligence.rs`
- `tests/editor_performance_invariants.rs`
- Authoritative bridge contract: [Language Intelligence and LSP 3.17](../../reference/primitives/language-intelligence.md)
- Public API: [`serverRegisterLanguageIntelligenceProvider`](../../reference/clay-js-api/language/server-register-language-intelligence-provider.md)

## Overview

Phase 18.20 implements one analyzer-neutral path for hover, go-to-definition, code actions, and signature help. Clay owns UTF-8 byte-offset request/result envelopes, provider selection and cancellation, result validation, protocol delivery, and projection onto existing menus/navigation/commands. Providers may be Rust implementations or resolver-validated package JavaScript. LSP JSON-RPC, `Content-Length`, URI and position-encoding conversion, initialization, and server-specific policy remain package responsibilities for Phase 18.21.

Semantic tokens, diagnostics, and completion do not use this four-feature result family. They reuse `DecorationSet`, `DiagnosticSet`, and `CompletionResultSet` respectively.

## Responsibilities

- Define versioned, provenance-bearing protocol data for four intelligence features.
- Register feature/mode/priority-scoped providers under `parse-document`.
- Schedule bounded `UiReactivePriority` work without blocking edits or paint.
- Cancel superseded work and stale-drop old document/provider generations.
- Validate locations, ranges, strings, nesting, payload size, and edit previews before publication.
- Present inert results through `TransientMenuSession`, validated workspace navigation, and `CommandExecution`.
- Keep process authority separate; a process-backed provider must independently use an approved `LanguageServerSession`.

## How It Works

### Registration and package bridge

A package declares `clay.contributions.languageIntelligenceProviders` and calls `serverRegisterLanguageIntelligenceProvider`. `op_clay_language_register_intelligence_provider` verifies package identity, `parse-document`, package-prefixed provider ID, supported modes/features, timeout, safe module path/export, and prohibited executable fields. A JS module export is stored in `globalThis.__clayLanguageIntelligenceHandlers` behind a runtime-issued token; no callback or process handle crosses into Rust metadata.

`ClayRuntimeEvaluation` returns provider metadata and JS registrations. `IpcServer::apply_runtime_evaluation` adapts JS registrations to `JsLanguageIntelligenceProvider` and registers them with the single `LanguageIntelligenceCoordinator`. Runtime generation replacement cancels old work.

### Request and coordination flow

```text
clay.language.hover | goToDefinition | codeActions | signatureHelp
  -> client captures document/version/behavior/cursor byte offset
  -> nonblocking ClientMessage::LanguageIntelligenceRequest
  -> connection builds a UTF-8-safe <=64 KiB document window
  -> LanguageIntelligenceCoordinator::schedule
  -> deterministic provider selection by feature, mode, priority, ID
  -> per-request oneshot result delivery
  -> connection-local channel -> ServerMessage::LanguageIntelligenceResult
  -> client staleness checks -> TransientMenuSession/navigation/action
```

The coordinator owns one in-flight task key per client/document/feature. A newer request aborts the old task. It caps global outstanding work at `LANGUAGE_INTELLIGENCE_MAX_OUTSTANDING_REQUESTS`, applies each provider timeout up to `LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS`, and returns immediately. Completion sends through a per-request oneshot, avoiding cross-client result theft from a shared receiver.

`finish_task` checks provider generation and document version, validates the result, and overwrites result provenance with registered provider provenance. Timeout/provider failures become sanitized status envelopes; cancelled/stale work is dropped without flashing UI.

### Protocol and validation

`LanguageIntelligenceRequest` carries request/client/document/version/behavior/provider-generation metadata, cursor byte offset, and `LanguageIntelligenceFeature`. `LanguageIntelligenceResult` adds provider provenance, `LanguageIntelligenceStatus`, and a feature-matched payload:

- `HoverResult`: optional range and bounded Markdown.
- `GoToDefinitionResult`: bounded `TextLocation` list.
- `CodeActionResult`: bounded inert titles, registered command IDs, and versioned `EditPreview` values.
- `SignatureHelpResult`: bounded signatures, parameters, documentation, and active indexes.

`validate_result` rejects unordered ranges, invalid active indexes, control characters, excessive fields/nesting/payloads, unsafe relative paths, external locations, and malformed previews. `TextLocation` is either an open `DocumentId` or known `WorkspaceRootId` plus normalized relative path; Clay core has no LSP URI or line/character type.

### Client presentation

`language_intelligence_result_to_menu_session` reuses existing shell state:

- Hover and signature help use modeless sessions with Markdown converted to inert plain text.
- Definitions and code actions use modal selectable sessions.
- Same-document definitions call `navigate_to_byte_offset`.
- Workspace definitions reuse `clay.workspace.openFile`, then apply a pending byte-offset jump after `DocumentOpened`.
- Command-backed actions use existing SDUI/`CommandExecution` validation.
- Edit previews display status only; Phase 18.20 never applies them.

`EditorWidget` tracks the active request ID and rejects mismatched results. Text edits, Escape, and menu activation clear the active request. Menu action arguments survive SDUI conversion, so workspace-root/path/offset metadata remains available without hidden widget callbacks.

### Semantic, diagnostic, and completion reuse

LSP-compatible outputs map onto existing primitives:

- semantic tokens -> `DecorationSpan::from_vocabulary` with `DecorationKind::Semantic`, `TokenType`, and `Modifiers`;
- diagnostics -> source-keyed `DiagnosticSet` publication;
- completion -> existing completion provider/results, including inert snippet text format, priority, exclusive claim, and disable behavior.

`language-server` grants none of these output permissions. Packages still need `render-decorations` or `completion-provider` where applicable.

## Code Example

```js
import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
import * as provider from "./provider.js";

serverRegisterLanguageIntelligenceProvider({
  packageManifest,
  id: "example.intelligence",
  modes: ["example"],
  features: ["hover", "definition", "codeAction", "signatureHelp"],
  module: provider,
  exportName: "provideLanguageIntelligence",
  timeoutMs: 500,
});
```

See the authoritative API page for complete options and errors.

## Primitive Coverage

- **Primitive/category:** `LanguageIntelligenceRequestAndResult` and `LanguageIntelligenceProvider`.
- **Owners:** `src/protocol/language_intelligence.rs` and `src/server/language_intelligence.rs`.
- **Facade/op:** `runtime/js/language.ts::serverRegisterLanguageIntelligenceProvider` and `src/server/ops/language_intelligence.rs`.
- **Permission:** `parse-document` for bounded open-document analysis; no implicit process/filesystem/network/shell authority.
- **Budgets:** 512 B request, 16 KiB result, 64 KiB document window, 16 outstanding requests, 500 ms default/5000 ms maximum timeout, plus feature-specific count/string limits in `src/perf/budgets.rs`.
- **Hot-path policy:** provider/JS/process work is cancellable server-side UI-reactive work; local edits and paint consume local or validated inert state only.
- **Reuse rule:** future analyzers and LSP bridges register through this generic provider lane; never add feature-specific schedulers or language-specific Rust branches.

## Invariants and Constraints

- Canonical positions are UTF-8 byte offsets tied to exact document versions.
- Registered provider provenance, not package-returned JSON, owns published identity.
- One coordinator serves all four features.
- Result payload feature must match request feature.
- External URIs, absolute/traversing paths, callbacks, raw ops, client JavaScript, and executable fields are rejected.
- Code-action edits are inert previews only.
- No provider, JavaScript, process, or IPC wait occurs before local text paint.

## Phase 18.21 Publish Handoff

`@clay/lsp-*` packages should layer LSP framing and conversion over this primitive and [Language Server Process Service](language-server-process-service.md). Required handoff:

1. Negotiate position encoding and convert against the exact Clay document version.
2. Normalize locations to open documents or approved workspace-root-relative paths.
3. Map hover/definition/code-action/signature responses into this result family.
4. Publish semantic tokens, diagnostics, and completions through their existing Clay primitives.
5. Preserve cancellation, generation checks, budgets, inert code-action previews, and additive semantic composition.
6. Keep all LSP types and language-specific policy out of Clay core.

## Tests

- `tests/language_intelligence.rs`: protocol round trips, validation, provider ordering, cancellation, timeout, provenance, semantic composition, and authority separation.
- `src/server/js_runtime.rs`: JS facade registration/rejection and token-backed provider invocation.
- `src/client/mod.rs`, `src/client/behavior.rs`, `src/masonry_editor.rs`, `src/shell/transient_menu.rs`: nonblocking request routing, stale result rejection, menu projection, navigation, and preview non-mutation.
- `tests/editor_performance_invariants.rs`: no provider/process/JS work in editor hot paths.
- `tests/performance_protocol.rs`: representative result payload ceiling.

```bash
cargo test --test language_intelligence
cargo test --test editor_performance_invariants
cargo test --test performance_protocol
```

## Related

- [Phase 18.20 Primitive Review](phase18.20-language-intelligence-primitive-review.md)
- [Language Server Process Service](language-server-process-service.md)
- [Transient Menu Session](transient-menu-session.md)
- [Command Registry](command-registry.md)
- [Decoration Transport](decoration-transport.md)
- [Range Diagnostics](range-diagnostics.md)
- [Completion Snippet Expansion](completion-snippet-expansion.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
