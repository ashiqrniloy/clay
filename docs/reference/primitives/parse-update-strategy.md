# Incremental Parse and Background Parse Update Strategy

Phase 16 defines parsing as **server-side, cancellable background work**. Package parsers may analyze document text and return inert syntax/decorator data, but they never run in the Rust client and never block local typing or paint.

This document is architecture-only. It introduces no runtime code in Phase 16.

## Goals

- Let packages provide syntax trees, syntax spans, semantic spans, diagnostic spans, folding inputs, and Markdown mode decoration data.
- Preserve `.agents/skills/project-patterns/references/authority-boundaries.md`: the server owns canonical document versions and JavaScript execution; the client owns immediate editing and rendering.
- Preserve `.agents/skills/project-patterns/references/behavior-manifests.md`: parse work is `Background`, not `ClientFirstPredictable`.
- Preserve `.agents/skills/project-patterns/references/protocol-and-performance.md`: no full-document IPC for ordinary edits, no synchronous server/JavaScript round trip before local paint, bounded queues, cancellable server work, and viewport-bounded result delivery.

## Non-Blocking Hot-Path Contract

Incremental parsing must not participate in the ordinary keypress-to-local-paint path.

- Local predictable edits remain `ClientFirstPredictable` behavior-manifest work on the Rust client.
- Parse tasks are `Background` routing policy tasks and must not delay input handling, client shadow updates, or client render/input work.
- The typing hot path must stay within `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`; parse scheduling and result publication are asynchronous follow-up work.
- Parse notifications and results are bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` and parse-produced decoration payloads are additionally bounded by `DECORATION_PAYLOAD_BUDGET_BYTES` before client delivery.
- A slow or unavailable package parser degrades decoration freshness only; it never prevents the client from showing the locally edited text.

## Parse Unit Boundaries

Packages declare the coarsest unit they can update incrementally. The server scheduler may split or merge work, but publication remains viewport-bounded.

| Unit | Use Case | Server Input | Publication Rule | Tradeoff |
| --- | --- | --- | --- | --- |
| File-level | Small files, first parse after open, parsers without incremental state | Open/reload snapshot plus current `DocumentVersion` | Publish only spans intersecting the current viewport; cache the rest if within memory limits | Simple, but may be too costly for large files after every edit. |
| Region-level | Languages with block/section invalidation, Markdown fenced-code regions, diagnostics around changed block | Affected byte range plus parser-maintained invalidated regions | Prioritize dirty regions that overlap or are adjacent to the viewport | Good balance for Markdown and code-block/heading scopes. |
| Line-group-level | Line-oriented syntax highlighting and Markdown continuation rules | Changed line group, surrounding context window, and base version | Publish spans for visible line groups first, then nearby cached groups | Small updates and easy cancellation; less suitable for deep syntax dependencies. |

The baseline Phase 18 Markdown POC should prefer **line-group-level** for inline syntax decoration and **region-level** for fenced code blocks and heading/list sections. Full file-level parsing is allowed for first activation or resync, but not as the default response to every accepted edit in large documents.

## Edit Notification Shape

A future parse coordinator should enqueue compact edit notifications after the server accepts an edit and increments the canonical version. The notification is intentionally not a full-document snapshot.

```text
ParseEditNotification {
  document_id,
  document_version,
  behavior_version,
  package_prefix,
  mode_id,
  viewport,
  invalidated_ranges,
  accepted_edit: Option<ParseInputEdit>,
  parse_windows: [ParseWindowSnapshot],
  memory_budget,
}

ParseInputEdit {
  base_document_version,
  document_version,
  start_byte, old_end_byte, new_end_byte,
  start_position, old_end_position, new_end_position,
}
```

Rules:

- `ParseInputEdit` is server-canonical metadata for one consecutive accepted version. Open, resync, and viewport-only notifications carry no fabricated edit.
- `ParseWindowSnapshot` is a UTF-8-safe bounded slice with stable `window_id`, absolute byte bounds, base point, and `incremental_edit`; it is the only parser text input.
- `document_id`, versions, provenance, and window metadata preserve ordering and stale-result rejection. `accepted_edit.relative_to_window` produces Tree-sitter-relative coordinates only when the edit fits the retained window.
- `viewport` and `invalidated_ranges` prioritize current output. Notifications and all returned members remain within `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` and the decoration budget; output chunking never creates additional parse notifications.

## Parse Task Lifecycle

Parse task lifecycle is server-side and cancellable:

1. `src/server/document.rs::DocumentState::apply_edit` accepts an edit, updates the canonical rope, and increments `DocumentVersion`.
2. The connection derives one canonical `ParseInputEdit`, retains a stable bounded window when possible, and submits one `ParseEditNotification` to `ParseCoordinator`.
3. The coordinator spawns one start-gated task for a new version/window, coalesces duplicate same-version/window requests, and supersedes older work for the same document/package/mode stream while preserving concurrency across unrelated documents.
4. The native handler reuses a matching cached tree with `Tree::edit`, parses once, unions `Tree::changed_ranges` with explicit invalidations, converts affected ranges into a shared 128-byte UTF-8-safe replacement-chunk grid via `replacement_ranges`, and queries the full envelope covering every touched chunk — so query coverage and replacement coverage are identical.
5. The handler maps complete captures into `IncrementalParseUpdate::decoration_updates`, splitting them into stable 128-byte sets in changed/visible-first order. Output chunk count does not multiply parse/query invocations.
6. If a newer document version arrives before completion, the coordinator cancels or marks the older task stale; matching-version active-task state is checked again before publication.
7. The server validates every decoration member atomically against document/version/provenance/range and payload budgets, then publishes ordinary inert `DecorationSet` messages outside paint/text-event handlers.

A task result is publishable only when its `document_version` matches the current server version or an explicitly accepted compatible version window. Older results are discarded before client delivery.

## Spawn, Cancel, Timeout, and Priority Model

A future `src/server/parse_coordinator.rs` is the preferred attachment point because it can keep parse policy separate from document mutation and JavaScript runtime concerns.

- **Spawn:** The coordinator observes accepted edits from `src/server/document.rs` and enqueues per-document work after the edit is committed.
- **Cancel:** Each document/mode/package parse stream keeps a cancellation token or generation counter. Newer edits supersede older tasks for overlapping regions.
- **Timeout:** Package parse handlers run with a bounded timeout. Timeout values are load-time validated package/configuration metadata, not per-keypress dynamic decisions.
- **Priority:** Visible viewport ranges are processed first, adjacent ranges second, and off-viewport/cache refresh work last.
- **Backpressure:** Queues are bounded per document and per package. When the queue is full, older stale work is dropped before newer viewport-relevant work.

`src/server/js_runtime/mod.rs` is the runtime boundary for executing package JavaScript through `deno_core`; it should remain the place where controlled server-side module execution, facade import allowlists, diagnostics, and raw-op restrictions are enforced. The parse coordinator should call into this runtime boundary instead of embedding parser JavaScript execution in `DocumentState`.

## Parse Result Shape

Package parse handlers return inert data. They do not return executable renderers, client callbacks, raw ops, or direct renderer instructions.

```text
ParseResult {
  document_id,
  document_version,
  behavior_version,
  package_prefix,
  parse_unit,
  invalidated_byte_ranges,
  syntax_tree_delta,       // optional compact tree/update summary
  decoration_updates,     // bounded DecorationSet members from one handler invocation
  folding_ranges,          // optional bounded range list
  diagnostics,             // optional bounded diagnostic spans
}
```

Publication rules:

- Serialized parse result metadata and incremental tree/update data must fit within `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` when no folding set is attached. An update carrying `folding_ranges` uses the derived `INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES` envelope; the folding set remains independently capped by `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES`.
- Decoration spans derived from the result must fit within `DECORATION_PAYLOAD_BUDGET_BYTES` after server validation and viewport filtering.
- Optional diagnostic side channels map to `IncrementalParseUpdate.diagnostic_update` / `DiagnosticSet` and must fit within `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES` after centralized validation; see [Range Diagnostics](diagnostics.md).
- `syntax_tree_delta` is server/cache metadata unless a later primitive explicitly exposes syntax trees. The Rust client receives only validated rendering/folding/diagnostic declarations it knows how to apply.
- Result delivery is viewport-prioritized: visible spans first, adjacent spans next, off-viewport data cached or discarded according to budget.

## Server Validation Before Client Delivery

Before publishing any parse-produced rendering update, the server validates:

- Package provenance: `package_prefix` matches the loaded package and active mode contribution.
- Permissions: parse handlers require the declared parse permission (for example `parse-document`) and cannot access filesystem, network, shell, AI, WASM, remote listeners, or raw `Deno.core.ops` unless a future decision explicitly grants and validates that authority.
- Version metadata: `document_id`, `document_version`, and `behavior_version` are current or safely compatible.
- Payload bounds: `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, the derived `INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES` when needed, `DECORATION_PAYLOAD_BUDGET_BYTES`, `DIAGNOSTIC_PAYLOAD_BUDGET_BYTES`, and `FOLDING_RANGE_PAYLOAD_BUDGET_BYTES` are enforced before allocation/publication.
- Ranges: byte ranges are valid for the server-canonical document version and intersect the delivered viewport unless deliberately cached server-side.
- Shape: decoration kinds, style tokens, priorities, folding kinds, and diagnostic severities are known inert schema values.
- Security: executable JavaScript, client-side callbacks, raw ops, arbitrary draw commands, native widget mutation, and unbounded strings are stripped or rejected.

Validation failures produce runtime diagnostics or package errors. They must not panic the server/client and must not add work to the typing hot path.

## Viewport-Prioritized Result Delivery

The client viewport is the primary publication filter.

- The server tracks the latest `viewport_byte_start` and `viewport_byte_end` received from each client.
- Results overlapping the visible viewport are published first.
- Results adjacent to the viewport may be cached and published opportunistically to improve scroll smoothness.
- Off-viewport full-document results are not sent as ordinary edit follow-up IPC.
- When multiple clients watch the same document, each client receives viewport-specific filtered updates while the server cache may retain shared parse state.
- Scroll-triggered delivery is separate from edit-triggered delivery and still bounded by the same payload budgets.

## Fallback Behavior When Parsing Lags

Lagging package work must be visually safe and predictable:

- If a parse result is not ready within two edit cycles, the server sends a `no-decoration-update` acknowledgement for the newer document version so the client knows decorations are intentionally unchanged.
- The client retains the last validated decoration set for unaffected ranges.
- The client may clear stale decorations inside the edited range after a stale-version notice or `no-decoration-update` acknowledgement.
- Diagnostics and semantic decorations are allowed to become temporarily stale; they must be replaced or cleared once a current result arrives.
- Syntax highlighting may fall back to mode-level default styling or plain text for changed regions.
- No fallback path runs package JavaScript in the client.

## Attachment Points in Existing Architecture

No code is added in Phase 16, but later phases should attach parsing at these boundaries:

- `src/server/document.rs`: after accepted edits in `DocumentState::apply_edit`, expose compact accepted-edit metadata to the coordinator. Do not run JavaScript here.
- `src/server/js_runtime/mod.rs`: extend the controlled `deno_core` runtime/facade allowlist with future `clay:parse` APIs such as `parse.serverRegisterParseHandler`; preserve server-side execution and sanitized diagnostics.
- `src/server/parse_coordinator.rs`: new module recommended for per-document parse queues, cancellation tokens/generations, timeout policy, viewport priority, cache management, and result validation/publication.
- Future protocol modules: define `ParseEditNotification`, `ParseResult`, and `DecorationUpdate`/folding/diagnostic publication messages as bounded protocol shapes.

This split keeps `DocumentState` focused on canonical document mutation, `ClayJsRuntimeService` focused on constrained JavaScript execution, and a new coordinator focused on scheduling and parse-result policy.

## Phase 18.16/Plan 056 Tiered Syntax Grammar Parse/Highlight Path

`SyntaxGrammarContribution` reuses this background parse strategy across three engine tiers. Tier 1 uses compiled first-party `tree-sitter-*` grammar data registered as static descriptors. Tier 2 uses the shared host-side web-tree-sitter adapter for resolver-validated package-root-confined `tree-sitter-wasm` and `.scm` assets. Tier 3 retains package-JS parse handlers for grammar-less languages, Markdown-specific behavior, or an explicit `javascript` preference. Every tier produces capture records for the same capture-to-Phase 18.15 `TokenType` + `Modifiers` mapper and bounded `DecorationSet` output.

For consecutive accepted versions, `ParseCoordinator` carries one exact `ParseInputEdit` into one stable bounded window. A matching Tree-sitter tree receives `Tree::edit`, the parser runs once, and `old_tree.changed_ranges(&new_tree)` is unioned with explicit invalidations. Affected ranges are converted into a shared 128-byte UTF-8-safe replacement-chunk grid via `replacement_ranges`, and the full envelope covering every touched chunk is queried once with `QueryCursor::set_byte_range` — so every published chunk's query coverage equals its replacement coverage; intersecting captures retain their complete grammar-owned boundaries and are clipped at exact chunk boundaries. Open/full/viewport fallback has no accepted edit and queries the bounded visible range explicitly.

One parse/capture result becomes `IncrementalParseUpdate::decoration_updates`: complete captures are split into stable 128-byte output sets, with sets intersecting explicit invalidations published before adjacent sets. The coordinator validates all members atomically, including per-member decoration and incremental-update payload budgets. Empty syntax sets are authoritative replacements. Output fan-out and ordinary `DecorationSet` transport/cache work never create sibling parser jobs or multiply parse/query invocation metrics.

Package load validates grammar metadata, paths, style maps, permissions, provenance, and budgets. At document open/reload/reclassification/package-load time, Clay selects syntax independently of major mode. `setSyntaxEnginePreference(target, tier)` is the only user override and accepts `native`, `wasm`, or `javascript`/`js`; package load order cannot silently replace a native descriptor. A document can remain editable as `core.code` or `core.text` while syntax is selected, and no grammar/fallback selection leaves major-mode editability unchanged.

Open scheduling is enqueue-only: text and the initial mode state return before parse completion. Handler errors, timeouts, invalid updates, and budget failures are sanitized into `RuntimeDiagnostic` values such as `parse.open_failed` through `ParseCoordinator::finish_task`; they do not block open or leak paths/source text. The client may interpolate already-validated inert syntax spans through optimistic edits, then subtract the exact authoritative half-open viewport from overlapping provisional chunks of the same package/layer via `apply_set`, preserving left/right span fragments outside authority and locally coalescing compatible residual chunks/spans. Tree-sitter and package parse/highlight work remains `Background`, cancellable, stale-version rejecting, viewport-prioritized, and bounded by `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, `DECORATION_PAYLOAD_BUDGET_BYTES`, and `SYNTAX_CACHE_BUDGET_BYTES`; no grammar package JavaScript, parser/query compilation, native artifact loading, filesystem/network/shell/AI/raw-op authority, or full-document IPC runs in editor hot paths. Tier 2 runtime assets are local/resolver-validated; no runtime download, shell/package-manager build, or native-library load occurs.

## Security Rules

- Background parse tasks run server-side through the constrained `deno_core` runtime, not in the Rust client.
- Parse results are validated and stripped of arbitrary code before becoming `DecorationUpdate`, folding, diagnostic, or cache payloads.
- Package parsers cannot access filesystem outside already-open document content, network, shell, AI mutation, WASM execution, remote listeners, raw `Deno.core.ops`, native widget mutation, or client-side JavaScript by default.
- Future exceptions require explicit permission declaration, server validation, documentation, and a decision log before implementation.
- Client delivery contains inert declarations only; the Rust client renders known declarations locally.

## Phase 18.5 Large-File Parse-Window Primitives

The Phase 18.5 [large-file Markdown primitive review](../../wiki/modules/phase18-large-file-markdown-primitive-review.md) identified bounded parse input as a reusable primitive gap. Clay now defines generic parse-window and memory-budget shapes in `src/protocol/parse.rs` and validates them through `src/server/parse_coordinator.rs`; the names and validation rules are intentionally mode-neutral.

Implemented reusable primitives:

- `ParseWindowSnapshot`: a server-canonical, versioned, UTF-8-boundary-validated text slice with `document_id`, `document_version`, package/mode provenance, `byte_start`, `byte_end`, `base_line`, and bounded `text`.
- `ParseWindowRequest` / `ParsePolicy`: a bounded request derived from viewport and invalidated ranges with generic guard bytes, timeout, package/mode provenance, `max_window_bytes`, and `memory_budget_bytes`.
- `SyntaxMemoryBudget` (the implemented `SyntaxCacheBudget` primitive) and `SYNTAX_CACHE_BUDGET_BYTES`: retained syntax/cache memory accounting with a 30 MiB large-file budget separate from total RSS, runtime baseline, canonical document storage, and temporary parser allocations.
- `DocumentState::parse_window_snapshot` / `parse_window_snapshots`: server-canonical rope slicing helpers that copy only validated byte windows, align guard ranges to UTF-8 boundaries, and reject oversized or over-budget windows.
- `ParseCoordinator::schedule_parse_with_windows`: schedules background parse work with prevalidated snapshots, aborts superseded tasks for the same document/package/mode, and delivers the current windows to the package handler only after `parse-document` handler registration.

Security and performance rules:

- Range snapshots require the existing `parse-document` permission path because only registered parse handlers can receive them; install/enable alone grants no parser text access.
- Snapshots expose only already-open document text inside validated requested windows and include document/version/provenance metadata for stale-result rejection and package ownership checks.
- Large-file ordinary edits must not pass the full document string to package JavaScript; small-file full snapshots remain a policy decision only when they fit documented budgets.
- Native grammars may set a larger data-only context ceiling when grammar state cannot be reconstructed from an arbitrary viewport fragment. Markdown uses `MAX_OPENABLE_FILE_BYTES` (768 KiB) so every openable file has complete block context; its query/decor viewport remains independently capped at `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES` (4 KiB), and same-version scroll requests reuse the cached native tree rather than reparsing. Code grammars retain 4 KiB context windows.
- The parse coordinator preserves stale-version rejection, cancellation/generation semantics, timeout bounds, payload budgets, package provenance, and no client-side package JavaScript.
- Rust primitive names and branches remain language-neutral; rejected examples include `MarkdownParser`, `MarkdownItToken`, `MarkdownHeading`, `MarkdownFence`, `heading_open`, `list_item_open`, and `if mode == "markdown"` parser paths.

## Phase 17/18 Follow-Up

- Add the `clay:parse` facade and `parse.serverRegisterParseHandler` planned API stub.
- Implement `src/server/parse_coordinator.rs` with bounded queues, cancellation, timeout, viewport priority, and stale-result discard tests.
- Add protocol structs and payload-bound tests for parse notifications/results and decoration publication.
- Implement Markdown mode parser behavior with line-group-level incremental updates and region-level fenced-code invalidation.
- Add tests proving parse delays do not block `ClientFirstPredictable` typing or violate `KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS`.
