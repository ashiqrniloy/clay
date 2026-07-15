# Language Intelligence and LSP 3.17 Bridge Contract

Phase 18.20 defines engine-neutral language-intelligence primitives and the deny-by-default `language-server` process boundary. This page is the canonical analyzer/LSP mapping contract: LSP 3.17 wire shapes stay in Phase 18.21 bridge packages; Clay core stores UTF-8 byte offsets, validated inert results, and opaque bounded sessions only.

Non-LSP analyzers and LSP adapters use the same Clay primitives. A fake Rust or package-JS analyzer that never speaks JSON-RPC is a first-class provider.

## Ownership Split

| Surface | Primitive | Role |
| --- | --- | --- |
| Hover / definition / code action / signature help | `LanguageIntelligenceRequest` / `LanguageIntelligenceResult` | Versioned, feature-tagged request/result envelope over UTF-8 byte offsets and known workspace locations. |
| Provider registration | `LanguageIntelligenceProvider` | Feature-tagged, cancellable, provenance-bound provider lane under `parse-document`. |
| Semantic highlighting | `DecorationSet` (`DecorationKind::Semantic`) | Additive two-axis vocabulary spans; refines syntax without erasing it. |
| Diagnostics | `DiagnosticSet` | Source-keyed range diagnostics with severity/code/message. |
| Completion | `CompletionResultSet` | Bounded items with plain/snippet text format, priority, exclusive claim. |
| Process transport | `LanguageServerSession` | Opaque host-owned byte/message conduit under explicit `language-server` grant. |

Clay core contains no LSP `Position`/`Range`/`Location`, `file://` URIs, JSON-RPC IDs, method names, `Content-Length` framing, or negotiated UTF-16/UTF-32 line/character fields. Position-encoding conversion happens at the package boundary against an exact document version.

## LSP 3.17 → Clay Mapping

| LSP 3.17 concept | Clay primitive / API | Conversion rules | Phase 18.21 responsibility |
| --- | --- | --- | --- |
| `textDocument/semanticTokens/*` (`SemanticTokens`, legend, delta) | `DecorationSet` with `DecorationKind::Semantic`, `TokenType`, `Modifiers` via `clay.decorations.serverPublishDecorations` | Decode legend/delta against the negotiated encoding and exact document version into UTF-8 byte ranges; map token types/modifiers onto Clay's closed vocabulary or open-string prefix escape; publish additive semantic spans that refine syntax. | Negotiate encoding/legend; apply deltas; convert positions; never erase syntax chunks. |
| `textDocument/publishDiagnostics` / `Diagnostic` | `DiagnosticSet` via `clay.diagnostics.serverPublishDiagnostics` | Map severity/code/message/source onto inert spans; convert ranges to bytes; replace by source key; empty array clears that source only. | Deduplicate tree-sitter recovery noise vs LSP diagnostics; keep viewport-bounded payloads. |
| `textDocument/completion` / `CompletionItem` | `CompletionResultSet` / `CompletionItem` via existing completion provider lane | Map `insertText`/`textEdit`, `insertTextFormat` → `PlainText`/`Snippet`, detail/documentation, priority/exclusive metadata; snippets remain inert text until client expansion. | Prefer richer LSP items over keyword bases without provider wars; honor user disable-native. |
| `textDocument/hover` / `Hover` | `LanguageIntelligencePayload::Hover` (`HoverResult`) | Convert optional range to bytes; flatten `MarkupContent`/`MarkedString` into bounded Markdown/plain text rendered as inert client text. | Choose MarkupKind; strip executable HTML/script; bound markdown length. |
| `textDocument/definition` / `Definition` / `DefinitionLink` | `LanguageIntelligencePayload::GoToDefinition` (`GoToDefinitionResult`) | Map `Location`/`LocationLink` to ordered `TextLocation` values: open `DocumentId` + byte range, or known `WorkspaceRootId` + normalized relative path + byte range. | Resolve URIs inside approved roots; order deterministic; reject external/out-of-root targets. |
| `textDocument/codeAction` / `CodeAction` / `Command` / `WorkspaceEdit` | `LanguageIntelligencePayload::CodeAction` (`CodeAction` + optional `EditPreview`) | Titles are inert strings; `Command` maps to a registered Clay command ID executed later through `CommandExecution`; `WorkspaceEdit`/`TextEdit` map to inert versioned `EditPreview` only in Phase 18.20. | Frame requests; convert edits; never auto-apply mutating WorkspaceEdit; later phases review rename/refactor authority. |
| `textDocument/signatureHelp` / `SignatureHelp` | `LanguageIntelligencePayload::SignatureHelp` (`SignatureHelpResult`) | Map signatures/parameters/documentation and active indexes; labels/docs are inert bounded text. | Negotiate trigger/retrigger; validate active indexes against signature/parameter counts. |
| JSON-RPC transport / initialize / sync / cancel | `LanguageServerSession` (`send`/`read`/`stop`) under `authorizeLanguageServer` + `startLanguageServerSession` | Opaque UTF-8 messages only. Framing, initialize, capabilities, `textDocument/did*`, `$/cancelRequest`, and Content-Modified handling stay package-side. | Implement LSP 3.17 adapters per `@clay/lsp-*` package; core remains analyzer-neutral. |

## Position and Location Conversion

```text
LSP Position (negotiated utf-8 | utf-16 | utf-32)
  -> bridge resolves against the exact Clay document version
  -> Clay UTF-8 byte offset / TextByteRange

LSP DocumentUri / Location / DefinitionLink target
  -> canonical known WorkspaceRootId + normalized relative path
  -> or open DocumentId
  -> reject absolute paths, `..` traversal, and URIs outside approved roots
```

Rules:

- Clay canonical positions are UTF-8 byte offsets tied to `DocumentId`/`DocumentVersion` or a known workspace root-relative path.
- The bridge owns encoding negotiation (`PositionEncodingKind`) and must re-resolve after edits using the version carried on the Clay request.
- `TextLocation::WorkspaceFile.relative_path` uses forward slashes, is non-empty, relative, and traversal-free.
- External schemes (`http(s):`, `git:`, non-file URIs) and out-of-root `file://` targets are denied.

## Request Lifecycle and Presentation

```text
local edit/paint completes
  -> client command captures document/version/cursor byte offset
  -> LanguageIntelligenceRequest (bounded, UiReactivePriority)
  -> LanguageIntelligenceProvider selection (mode/feature/priority/provenance)
  -> optional LanguageServerSession + package-side LSP conversion (Phase 18.21)
  -> LanguageIntelligenceResult validation + version/generation checks
  -> TransientMenuSession / status / navigation / CommandExecution projection
```

Presentation reuse:

| Feature | UI / action path |
| --- | --- |
| Hover / signature help | Bounded modeless transient text/status; empty/timeout/error statuses are typed. |
| Multiple definitions / code actions | `TransientMenuSession` with deterministic ordering. |
| Selected definition | Validated workspace/document navigation only. |
| Command-backed code action | Existing `CommandExecution` after registration/permission checks. |
| Direct edit preview | Inert `EditPreview` only; Phase 18.20 never auto-applies edits. |

Fallback/empty states use `LanguageIntelligenceStatus`: `Ok`, `Empty`, `Timeout`, `ProviderError`. Cancelled and stale work is dropped before client publication. Result ordering for definitions and code actions is deterministic provider order then input order within the validated bound.

## Markdown Handling

Hover, signature, and documentation fields may carry Markdown. Clay treats that text as inert: no HTML/script execution, no client-side JavaScript, no raw CSS, and no callback hooks. Oversize markdown/labels/docs reject under the field budgets below. Themes and typography do not change because of intelligence payloads.

## Budgets and Hot Paths

| Limit | Constant |
| --- | --- |
| Request payload | `LANGUAGE_INTELLIGENCE_REQUEST_PAYLOAD_BUDGET_BYTES` (512 B) |
| Result payload | `LANGUAGE_INTELLIGENCE_RESULT_PAYLOAD_BUDGET_BYTES` (16 KiB) |
| Definition locations | `LANGUAGE_INTELLIGENCE_MAX_DEFINITION_LOCATIONS` (64) |
| Code actions | `LANGUAGE_INTELLIGENCE_MAX_CODE_ACTIONS` (64) |
| Signatures / parameters | `LANGUAGE_INTELLIGENCE_MAX_SIGNATURES` (16) / `LANGUAGE_INTELLIGENCE_MAX_PARAMETERS` (32) |
| Edits per preview | `LANGUAGE_INTELLIGENCE_MAX_EDITS_PER_PREVIEW` (32) |
| Hover markdown | `LANGUAGE_INTELLIGENCE_MAX_HOVER_MARKDOWN_CHARS` (4096) |
| Outstanding requests | `LANGUAGE_INTELLIGENCE_MAX_OUTSTANDING_REQUESTS` (16) |
| Default / max timeout | `LANGUAGE_INTELLIGENCE_DEFAULT_TIMEOUT_MS` (500) / `LANGUAGE_INTELLIGENCE_MAX_TIMEOUT_MS` (5000) |
| Session message / stderr / count | `LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES`, `LANGUAGE_SERVER_STDERR_BUDGET_BYTES`, `LANGUAGE_SERVER_MAX_SESSIONS` |

Performance contract:

- No LSP, provider, process, JavaScript, or IPC wait occurs before local text paint.
- Hover/definition/code-action/signature requests are cancellable `UiReactivePriority` work with version and provider-generation checks.
- Semantic and diagnostic publication remains viewport-prioritized `Background` work under existing decoration/diagnostic budgets.
- Completion remains on its existing cancellable UI-reactive lane.
- Package-side LSP conversion is bounded; oversize or slow conversion fails closed rather than blocking paint.
- Process start/read/write/stop is async server work outside client hot paths.

## Permissions and Process Authority

| Capability | Permission | Notes |
| --- | --- | --- |
| Register intelligence provider | `parse-document` | Receives only bounded Clay-provided open-document data; returns inert results. |
| Publish semantic/diagnostic spans | `render-decorations` | `language-server` does not bypass this. |
| Publish completion items | `completion-provider` | `language-server` does not bypass this. |
| Command-backed code actions | command registration + command-specific checks | Executed later through `CommandExecution`. |
| Spawn language-server child | `language-server` | Deny-by-default; explicit `authorizeLanguageServer` grant before `loadPackage`; sealed on first load. |

Security contract:

- Explicit grants bind package provenance, contribution fingerprint, canonical executable, inherited-environment names, and known directory-root IDs.
- No implicit server launch from `loadPackage`, bundled `NativeTrust`, or provider registration.
- Sessions use fixed validated descriptors, direct `tokio::process::Command` (never a shell string), `env_clear` + declared inherits, approved-root cwd, piped stdio, `kill_on_drop`, and hard message/session/stderr/timeout caps.
- External URI denial and root-relative normalization apply to every location.
- Code-action edit previews stay inert and bounded; mutating WorkspaceEdit/rename/refactor need later authority review.
- Process diagnostics are sanitized and must not echo document source, inherited secrets, or unbounded child output.
- Approved containment semantics: cwd/root identity is launch metadata and audit identity, **not** an OS filesystem/network/process sandbox. A same-user child can read other paths, use the network, or spawn processes. Call this trusted subprocess authority. See `decision-logs/2026-07-14-2023-language-server-package-authority.md`.

## Phase 18.21 Extension Checklist

Bridge packages (`@clay/lsp-rust`, `@clay/lsp-typescript`, `@clay/lsp-javascript`, `@clay/lsp-markdown`) must:

1. Declare `language-server` plus fixed `clay.contributions.languageServers` metadata and require `authorizeLanguageServer` before load.
2. Layer LSP `Content-Length` framing, initialize/capability negotiation, document sync, and `$/cancelRequest` on `LanguageServerSession`.
3. Convert negotiated positions and URIs using this contract; reject out-of-root and external targets.
4. Map the seven LSP feature families in the table above onto Clay primitives without adding LSP types to core.
5. Keep requests cancellable, versioned, and off the typing/rendering hot path.
6. Deduplicate diagnostics against tree-sitter recovery where both sources publish.
7. Preserve additive semantic+syntax composition and existing completion exclusive/disable behavior.
8. Leave mutating rename/refactor/format/import-management behind a future authority review.

## Coverage

Deterministic coverage: `tests/language_intelligence.rs`, `tests/language_server_authority.rs`, `tests/range_diagnostics.rs`, `tests/completion_provider.rs`, `tests/editor_performance_invariants.rs`, `tests/performance_protocol.rs`, `tests/primitives_docs.rs`, and `tests/package_loading_docs.rs`. Manual smoke markers: `docs/development/launch-and-gui-smoke.md` Phase 18.20/18.21 sections. Architecture review: `docs/wiki/modules/phase18.20-language-intelligence-primitive-review.md`.
