# First-Party LSP Bridge Packages

## Source

- `packages/lsp-shared/framing.js` — Content-Length frame encode/decode (1 MiB frame, 8 KiB header)
- `packages/lsp-shared/positions.js` — `VersionedDocument` with UTF-8/UTF-16/UTF-32 encoding, byte-to-position conversion
- `packages/lsp-shared/mapping.js` — LSP responses → Clay vocabulary (semantic tokens, diagnostics, completions, hover, definitions, code actions, signature help) with payload budgets
- `packages/lsp-shared/client.js` — `LspClient` session lifecycle, initialize handshake, document sync, server request allowlist
- `packages/lsp-shared/utf8.js` — Pure-JS UTF-8 codec (`encodeUtf8`/`decodeUtf8`/`utf8ByteLength`; no TextEncoder/TextDecoder)
- `packages/lsp-shared/typescript-language-server.js` — Shared canonical bridge factory for TypeScript/JavaScript
- `packages/lsp-rust/{package.json,dist/index.js,dist/load.js,dist/server.js}`
- `packages/lsp-typescript/{package.json,dist/index.js,dist/load.js,dist/server.js}`
- `packages/lsp-javascript/{package.json,dist/index.js,dist/load.js,dist/server.js}`
- `packages/lsp-markdown/{package.json,dist/index.js,dist/load.js,dist/server.js}`
- `scripts/update-first-party-lsp-shared.mjs` — deterministic copy distributor with freshness check
- `src/server/document_analysis.rs` — generic document-analysis worker lifecycle
- `src/server/ops/document_analysis.rs` — `register_document_analyzer` deno op
- `src/server/language_server.rs` — lossless `sendBytes`/`readBytes` ops
- `src/server/js_runtime.rs` — `op_clay_language_server_send_bytes`/`read_bytes`, worker invocation
- `src/packages/record.rs` — `validate_api_dependency_permissions` allowlist
- `src/perf/budgets.rs` — all Phase 18.21 typed budget constants
- `tests/fixtures/lsp/fake-server/{profiles,session,server,matrix.test,mjs}` — generic deterministic fake LSP harness
- `tests/lsp_bridge.rs` — shared adapter freshness, package manifests, fake-server matrix
- `tests/lsp_real_servers.rs` — environment-gated real-server smoke
- `tests/language_server_authority.rs` — session cap, revoke, byte-op limits, process lifecycle
- `tests/performance_protocol.rs` — typed budget lock assertions
- `tests/editor_performance_invariants.rs` — worker capacity constant guards
- Four package test files: `*-package.test.mjs` and `*-real-smoke.test.mjs`
- Decision log: `decision-logs/2026-07-15-1750-phase18.21-package-worker-authority.md`
- Authoritative bridge contract: [Language Intelligence and LSP 3.17](../../reference/primitives/language-intelligence.md)

## Overview

Phase 18.21 ships four first-party LSP bridge packages (`@clay/lsp-rust`, `@clay/lsp-typescript`, `@clay/lsp-javascript`, `@clay/lsp-markdown`) that layer LSP 3.17 framing, JSON-RPC, capability negotiation, document synchronization, and position/URI conversion entirely in package JavaScript over existing Clay primitives. Rust core remains LSP wire neutral: zero `Content-Length`, `jsonrpc`, `textDocument/*`, or `$/cancelRequest` string constants enter the server process, document analysis, connection wiring, completion, or language-intelligence code.

Each package declares an explicit fixed `language-server` capability contribution, registers a document analyzer through `serverRegisterDocumentAnalyzer`, and maps negotiated LSP responses onto Clay semantic tokens, diagnostics, completions, hover, definitions, code actions, and signature help through the shared `packages/lsp-shared/` adapter. Base language packages (`@clay/rust`, `@clay/typescript`, `@clay/javascript`, `@clay/markdown`) remain usable without a bridge; loading a bridge adds LSP enrichments without replacing Tier 1 syntax, base completion, or Markdown preview.

## Responsibilities

- Own all LSP wire protocol: Content-Length framing, JSON-RPC, methods, capabilities, document sync, positions, URIs, and cancellation in package JavaScript.
- Negotiate per-server capability differences (incremental vs full sync, push vs pull diagnostics, full-only vs full+delta semantic tokens, signature help presence/absence).
- Convert negotiated LSP responses to Clay vocabulary through bounded payload mapping.
- Respect Rust core budgets and worker lifecycle: message budget, session cap, worker count, document/text ceilings, input/output queue limits.
- Reject malformed framing, external URIs, mutating workspace edits, and oversize frames server-side before they enter the worker.
- Publish semantic tokens, diagnostics, and completions through existing `DecorationSet`, `DiagnosticSet`, and `CompletionResultSet` validators.
- Preserve base package behavior, syntax, and completion as independent fallback.

## Shared LSP 3.17 Adapter (`packages/lsp-shared/`)

One canonical adapter ships in `packages/lsp-shared/` and is deterministically copied into each language package's `dist/shared/` directory by `scripts/update-first-party-lsp-shared.mjs`. Packages import these copies; no package imports from another package's directory.

### `framing.js`

Bounded Content-Length frame encode/decode with hard limits:
- `MAX_FRAME_BYTES` = 1 MiB (aligned with `LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES`)
- `MAX_HEADER_BYTES` = 8 KiB
- `encodeFrame(message)` → `Uint8Array` with `Content-Length: N\r\n\r\n{json}\r\n`
- `FrameDecoder` class: `push(chunk: Uint8Array)` → parsed JSON messages, `finish()` detects truncation
- Validates ASCII-only headers, Content-Length presence/uniqueness/budget, Content-Type, and JSON-RPC object shape
- Rejects duplicate Content-Type headers and non-object message payloads
- Uses `utf8.js` codec (no `TextEncoder`/`TextDecoder` — Clay's deno_core runtime lacks the `deno_web` crate)

### `positions.js`

`VersionedDocument` with UTF-8/UTF-16/UTF-32 position encoding:
- `applyByteChange(byteOffset, oldEnd, newText)` → updates cached encoder output
- `byteOffsetToPosition(byteOffset)` → `{line, character}` in negotiated encoding
- `positionToByteRange(position)` → `{byteStart, byteEnd}` in UTF-8
- `normalizeRoot(path)` → rejects backslash, double-slash, dot/dotdot segments, query, hash, and percent-encoded slashes
- `isInRoot(uri, root)` → validates file URI containment within canonical root

### `mapping.js`

LSP response → Clay vocabulary conversion with typed payload budgets:
- `DECORATION_PAYLOAD_BYTES` = 8 KiB, `DIAGNOSTIC_PAYLOAD_BYTES` = 8 KiB, `RESULT_PAYLOAD_BYTES` = 16 KiB
- `MAX_SEMANTIC_TOKENS` = 128, `MAX_DIAGNOSTICS` = 128, `MAX_COMPLETIONS` = 256, `MAX_DEFINITIONS` = 64, `MAX_CODE_ACTIONS` = 64, `MAX_SIGNATURES` = 16, `MAX_PARAMETERS` = 32, `MAX_MARKDOWN_CHARS` = 4096
- `semanticTokensToClay(tokens, legend)` → `DecorationSet` with `TokenType`/`Modifiers` vocabulary via `TOKEN_TYPES`/`TOKEN_MODIFIERS` Maps
- `diagnosticsToClay(diagnostics, source)` → source-keyed `DiagnosticSet`
- `completionToClay(items)` → `CompletionResultSet`; mutating text edits (`.edit`) are filtered
- `hoverToClay(hover)` → `HoverResult`; strips `<script>` tags from Markdown
- `definitionToClay(definition)` → `GoToDefinitionResult`; filters to open documents only via `documentsByUri`
- `codeActionsToClay(actions)` → `CodeActionResult`; filters items with `.edit` (mutating `WorkspaceEdit`) automatically
- `signatureHelpToClay(help)` → `SignatureHelpResult`
- `parseCapabilities(caps)` → normalized capabilities with defaults; rejects array and non-object textDocumentSync
- Legend size limits: 256 tokenTypes, 32 tokenModifiers
- `boundedPayload()` enforces serialized payload ceilings via `utf8ByteLength`

### `client.js`

`LspClient` manages the server lifecycle:
- `initialize(serverCaps)` → sends `initialize` with Clay client capabilities (UTF-8 encoding, incremental sync, full+delta semantic tokens, pull diagnostics, completion snippets)
- `openDocument(uri, text, languageId, version)` → `textDocument/didOpen`
- `changeDocument(uri, version, changes)` → `textDocument/didChange` with versioned content changes
- `closeDocument(uri)` → `textDocument/didClose`
- `shutdown()` → sends shutdown request, waits for response
- `sendRequest(method, params)` / `sendNotification(method, params)` — validates method is non-empty string
- `receiveBytes(chunk)` → parses responses, notifications, and errors via `FrameDecoder`
- Error response validation: rejects malformed errors (missing code integer, missing message string)
- Server request handler allowlist map for `workspace/configuration`, `window/workDoneProgress/create`
- `finish()` on empty read after shutdown

### `utf8.js`

Pure-JS UTF-8 codec replacing `TextEncoder`/`TextDecoder` (unavailable in Clay's deno_core runtime):
- `encodeUtf8(str)` → `Uint8Array`
- `decodeUtf8(bytes)` → `string`; rejects malformed sequences
- `utf8ByteLength(str)` → byte count without allocating

## Document-Analysis Worker Infrastructure (`src/server/document_analysis.rs`)

The generic document-analysis coordinator is the Rust-side lifecycle for all four LSP bridge packages. It is analyzer-neutral: zero LSP types or language-specific branches.

### Worker lifecycle

- Workers are spawned lazily on first eligible `open_document` and stopped after last `close_document`.
- Max 4 workers globally (`DOCUMENT_ANALYSIS_MAX_WORKERS`), each with 32 synchronized documents and 8 MiB retained text.
- Worker identity: `WorkerKey { package_name, contribution, workspace_root_id, generation }`.
- Graceful shutdown: 2-second drain then kill within 5 seconds.
- Workers are dedicated `JsRuntime` instances sharing `ClayOpState` via `Arc<Mutex<PackageService>>` with `language_server_authority_sealed: true`.
- `cancel_package`, `cancel_root`, and `cancel_generation` lifecycle hooks remove worker routes and shut down matching workers.

### Input mailbox

- Bounded channel with `coalesce_reset`: pending resets for the same document replace earlier ones.
- Max input: 64 deltas or 2 MiB.
- Event kinds: `Open`, `Change`, `Reset`, `Close`, `Completion`, `LanguageIntelligence`, `Shutdown`.
- `change_document` returns `true` when the mailbox is full or the delta exceeds `DOCUMENT_ANALYSIS_MAX_DELTA_BYTES`, signaling the caller to send a `Reset` instead.

### Output channel

- Bounded channel: 64 events or 512 KiB.
- Outputs validated against active document version before delivery.
- Worker breaks on output queue saturation; stale outputs are dropped on send failure.

### Integration points

- `CompletionRequest` in `connection.rs` attempts dynamic provider resolution first (matching package prefix and trigger characters), falls back to static completion.
- `LanguageIntelligenceRequest` routes through `worker_for_document` to registered analyzer handlers.
- `EditAck` calls `document_changed` on both `CompletionCoordinator` and `LanguageIntelligenceCoordinator` to abort stale in-flight work.
- `IpcServer::reload_runtime_generation` opens refreshed documents through `document_analysis.open_document`.

## Four Package Policies

### `@clay/lsp-rust` (rust-analyzer)

- **Launch**: `rustup run stable rust-analyzer` (rustup proxy, not bare `rust-analyzer` — the binary is a rustup proxy that loses argv[0] dispatch).
- **Sync**: incremental (`textDocumentSyncKind.Incremental`).
- **Semantic tokens**: full + delta (`semanticTokens/full` + `semanticTokens/full/delta`).
- **Diagnostics**: pull (`textDocument/diagnostic`); polled after document changes.
- **Completion**: halving-retry truncation because real rust-analyzer lists can exceed 16 KiB result budget.
- **Signature help**: triggers on `(`, `,`, `<`.
- **Code actions**: bare boolean `true` (no resolve).

### `@clay/lsp-typescript` and `@clay/lsp-javascript` (typescript-language-server)

- **Shared factory**: `packages/lsp-shared/typescript-language-server.js` is imported by both packages; they differ only in `apiPrefix`, contribution ID, language IDs, and completion trigger characters.
- **Launch**: `typescript-language-server --stdio`; TypeScript 5.9.3 globally installed alongside (TypeScript 7.x removed `tsserver.js`).
- **Sync**: incremental.
- **Semantic tokens**: full-only (no delta).
- **Diagnostics**: push (`textDocument/publishDiagnostics`); not pull.
- **Server requests**: `workspace/configuration` handler returns array of nulls; `$/typescriptVersion` notification is ignored.
- **Completion triggers**: `['.', '"', "'", '/', '@', '<']`.
- **`member` token type**: mapped to `Method` (non-standard in TypeScript/JavaScript legend).

### `@clay/lsp-markdown` (Marksman)

- **Launch**: `marksman server` (the `server` subcommand is required).
- **Sync**: full document only (change:1, openClose:true). Marksman does not advertise incremental sync.
- **Semantic tokens**: full-only (no delta). Legend has 3 tokenTypes (`class`, `class`, `enumMember`).
- **Diagnostics**: push (`textDocument/publishDiagnostics`) for broken wiki links.
- **Signature help**: not advertised — bridge returns empty and does not declare `signatureHelp` in its language intelligence features.
- **Code actions**: only `Create a Table of Contents` (mutating `WorkspaceEdit`) which is automatically filtered by `codeActionsToClay` — no bridge-level filtering needed.
- **Project marker**: `.marksman.toml` file in the workspace root enables proper hover/definition/completion behavior.
- **Real-world behavior note**: Marksman's advertised capabilities greatly exceed its real behavior (semantic tokens return empty data without `.marksman.toml`, hover/definition/completion return null without a project marker). The bridge handles all of this gracefully with degradation to base Markdown decorations and completion.

### Package manifest contract

All four packages share the same manifest structure:
- `capabilities: ["language-server"]` (not `permissions` — language-server is a prohibited authority that cannot be requested by default).
- Fixed `executable`, literal `args`, and explicit `inheritEnvironment` (empty array for all four).
- `completion-provider` permission with `runtimeBridge: true`, `priority: 100`, `exclusive: false`, and `exportName: "provideCompletion"`.
- `languageIntelligenceProviders` with mode-scoped features matching what each server advertises.
- API prefix (`apiPrefix`) must match all contribution IDs (e.g., `lsp-rust` prefix → `lsp-rust.server`, `lsp-rust.bridge`).

## Security Boundary

- Fixed executable/argv/environment: package manifests declare exact launch descriptors; runtime code cannot select executables, change arguments, or inject environment.
- Grant-before-load: `authorizeLanguageServer` must be called during configuration evaluation and sealed before package code executes. No bundled/implicit grant.
- Byte bounds: send/read operations enforce `LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES` (1 MiB); oversize frames are rejected before reaching the child.
- Malformed framing: `FrameDecoder` rejects invalid Content-Length, non-ASCII headers, truncated frames, and non-JSON-RPC objects.
- External URI denial: file URIs outside the approved workspace root are rejected by `isInRoot`.
- Inert workspace edits: `codeActionsToClay` already filters items with `.edit`; no mutating operations reach the editor.
- Containment language: all four package docs repeat the trusted-subprocess model (same-user OS authority, not OS sandboxing).

## Loading and Fallback

Configuration must grant authority before loading each bridge package:

```js
import { authorizeLanguageServer } from "clay:language-server";
import { loadPackage } from "clay:packages";

await authorizeLanguageServer({
  package: "@clay/lsp-rust",
  contribution: "lsp-rust.server",
  workspaceRootIds: [1],
});
// ... repeat for lsp-typescript, lsp-javascript, lsp-markdown ...

await loadPackage("@clay/rust");
await loadPackage("@clay/lsp-rust");
// ... repeat for all four ...
```

Base packages remain usable without bridge grants. Removing a bridge `loadPackage` or revoking its grant leaves base Tier 1 syntax, keyword/snippet completion, and Markdown preview operational. Bridge semantic/diagnostic/completion outputs must not linger as authority after revocation — `revoke_language_server_grants` kills all owned sessions, and `cancel_package` removes worker routes.

## Tests

### Fake-server matrix (deterministic, runs on every `cargo test`)

- `tests/fixtures/lsp/fake-server/profiles.mjs` — 9 capability/response profiles (rust, typescript, javascript, markdown, minimal, hung, exit-early, malformed, oversize)
- `tests/fixtures/lsp/fake-server/session.mjs` — `FakeLspSession` in-process class
- `tests/fixtures/lsp/fake-server/server.mjs` — spawnable Node.js stdio fake server
- `tests/fixtures/lsp/fake-server/matrix.test.mjs` — drives all four bridges through fake sessions
- `tests/fixtures/lsp/fake-server/fake-server.test.mjs` — profile completeness, framing, spawnable round-trip
- `tests/lsp_bridge.rs` — runs Node tests, verifies adapter copy freshness, validates manifests

### Real-server smoke (opt-in with `CLAY_LSP_REAL_SMOKE=1`)

- `tests/lsp_real_servers.rs` — runs per-server Node smoke tests; skips with reason when binary unavailable
- Each bridge has a `*-real-smoke.test.mjs` with document fixtures, polling loops, and clean shutdown

### Package integration tests

- `*-package.test.mjs` — 3 tests per package covering manifest, feature mapping, error/identity rejection
- `tests/lsp_bridge.rs` — Rust-side manifest validation and fixture loading
- `src/server/js_runtime.rs` — grant-before-load, registration, and controlled-module bridge invocation
- `tests/completion_provider.rs` — priority 100 non-exclusive merge, `serverDisableCompletion` override
- `tests/range_diagnostics.rs` — diagnostic composition (Tree-sitter recovery suppression)
- `tests/language_server_authority.rs` — lossless bytes, session cap, revoke, process lifecycle
- `tests/editor_performance_invariants.rs` — worker capacity constants, no LSP wire types in hot paths
- `tests/performance_protocol.rs` — 20 typed budget lock assertions

## Primitive Coverage

| Primitive | Used for | Source |
|-----------|----------|--------|
| `LanguageServerSession` (lossless bytes) | Opaque child stdin/stdout | `src/server/language_server.rs`, `src/server/ops/language_server.rs` |
| `DocumentAnalysisCoordinator` | Worker lifecycle, input/output channels, document sync | `src/server/document_analysis.rs` |
| `DocumentAnalyzerRegistration` | Package-owned analyzer identity and handler | `src/server/ops/document_analysis.rs` |
| `CompletionCoordinator` (dynamic adapter) | LSP completion routed through analyzer worker | `src/server/completion.rs`, `src/server/connection.rs` |
| `LanguageIntelligenceCoordinator` | Hover, definition, code action, signature help | `src/server/language_intelligence.rs` |
| `DecorationSet` (Semantic) | Semantic token publication | `src/protocol/decorations.rs`, `src/server/decorations.rs` |
| `DiagnosticSet` (source-keyed) | LSP diagnostic publication | `src/protocol/diagnostics.rs`, `src/server/diagnostics.rs` |
| `CompletionResultSet` | LSP completion items | `src/protocol/completion.rs` |
| `LspClient` / shared adapter | All LSP wire protocol | `packages/lsp-shared/{framing,positions,mapping,client}.js` |

## Related

- [Phase 18.21 LSP Bridge Primitive Review](phase18.21-lsp-bridge-primitive-review.md)
- [Language Intelligence](language-intelligence.md)
- [Language Server Process Service](language-server-process-service.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Decoration Transport](decoration-transport.md)
- [Range Diagnostics](range-diagnostics.md)
- [Completion Snippet Expansion](completion-snippet-expansion.md)
- [First-Party Language Packages](first-party-language-packages.md)
- [LSP 3.17 Bridge Contract](../../reference/primitives/language-intelligence.md)
- [Bridge Package Authoring Guide](../../reference/packages/creating-packages.md#phase-1821-lsp-bridge-packages)
