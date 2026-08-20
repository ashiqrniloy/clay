# Low-Latency Incremental Syntax Decoration Primitive Review

## Source

- `plans/056-Low-Latency-Incremental-Syntax-Decoration.md`
- `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`
- `decision-logs/2026-07-19-1912-syntax-decoration-continuity-and-complete-authoritative-replacement.md`
- `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md`
- `plans/057-Syntax-Decoration-Continuity-and-Replacement-Correctness.md`
- `decision-logs/2026-07-19-2238-exact-range-provisional-decoration-replacement.md`
- `plans/058-Exact-Range-Provisional-Decoration-Replacement.md`
- `src/protocol/parse.rs`
- `src/protocol/decorations.rs`
- `src/server/document.rs`
- `src/server/connection/mod.rs`
- `src/server/parse_coordinator.rs`
- `src/server/syntax.rs`
- `src/server/decorations.rs`
- `src/editor/surface/mod.rs`
- `src/perf/metrics.rs`
- `benches/first_party_language_baselines.rs`
- `tests/performance_protocol.rs`
- `tests/primitives_docs.rs`

## Overview

This review originally mapped the approved low-latency syntax architecture onto Clay's existing generic parse and decoration primitives; it now records the completed implementation. Ownership, validation, cache-budget, vocabulary, theme, and background-task infrastructure remain reusable. The originally identified generic gaps—exact accepted-edit metadata, stable parse identity, one-parse capture extraction with bounded decoration fan-out, changed-range querying, affected-range replacement, and bounded provisional client interpolation—are implemented below.

Approved target flow:

```text
accepted edit -> one exact ParseInputEdit -> one stable-window parse
              -> changed ranges -> one capture pass -> bounded DecorationSet fan-out
optimistic edit -> interpolate validated inert spans
authoritative current-version output -> atomic affected-range replacement
```

No language-specific Rust path or parallel parser scheduler is needed.

## Current Flow Evidence

1. `refresh_native_syntax_after_edit` runs only after canonical edit acceptance. It obtains current document metadata/version and schedules native syntax asynchronously; local client paint has already happened.
2. Before scheduler coalescing, the refresh path divided a first-party 4 KiB parse window into contiguous 256-byte destinations.
3. Before scheduler coalescing, each destination became a separate `ParseCoordinator::schedule_parse_with_windows` call carrying a clone of the same `ParseWindowSnapshot`, producing up to 16 same-window native handler jobs.
4. Before coalescing, `TaskKey` contained `viewport_start`, deliberately allowing sibling chunk jobs to coexist. `TreeSitterSyntaxHandler::parse_sync` parsed once per job through the handler's shared `Arc<Mutex<Parser>>`, then queried only that job's 256-byte viewport. Implemented scheduling now keys native work by stable window identity and submits one bounded window request.
5. `CachedSyntaxTree` reuse requires the same document and identical `window_start`. `refresh_native_syntax_after_edit` derives `window_start` from `edit_start - max_window_bytes / 2`, so nearby typing moves the key and commonly forces a full parse.
6. Before exact reuse, the handler applied a whole-window `InputEdit` (`0..old_tree.end_byte()` replaced by all new window text). `ParseEditNotification` had invalidated ranges but no exact accepted edit coordinates or old/new points, and `Tree::changed_ranges` was not queried.
7. `SyntaxChunkCache` bounds retained chunk metadata and payload accounting. It does not own a reusable parsed-window capture result that can be split after one parse.
8. `EditorDecorationState::apply_edit` now shifts unaffected retained spans, resizes strict-interior syntax spans, lets generic broad token families inherit edge insertions, preserves surviving syntax through delete/replace, and invalidates intersecting non-syntax geometry.
9. Transformed chunks are provisional and carry edit-adjusted geometry/version keys. `EditorDecorationState::apply_set` replaces exact keys plus overlapping provisional keys for the same package/layer; set-level identity lets empty authoritative output clear an affected range.
10. Syntax and semantic spans still share inert normalization, but interpolation is authority-aware: syntax may be provisional, while intersecting semantic/diagnostic/search spans invalidate and unaffected ones only shift. Authoritative syntax replacement cannot erase another layer.

## Primitive and Gap Matrix

| Primitive | Existing owner and behavior | Reuse without change | Generic gap before target architecture |
| --- | --- | --- | --- |
| `ParseCoordinator` | `src/server/parse_coordinator.rs`; registers permission-checked handlers, schedules background tasks, coalesces duplicate stable-window/version work, cancels older stream versions, rejects stale versions/generations, validates updates, and records task stats. | Keep as the only scheduler and lifecycle owner. Preserve generation/package cancellation, stale rejection, diagnostics, and non-blocking scheduling. | Implemented: decoration viewport is no longer logical native parse identity; one handler result can carry multiple independently bounded sets, all validated before publication. |
| `ParseEditNotification` | `src/protocol/parse.rs`; carries current document/version, behavior, package/mode, viewport, invalidated ranges, windows, and memory budget. | Keep package-neutral provenance, versions, invalidations, and optional bounded snapshots. Keep open/resync/viewport notifications valid without fabricated edits. | Add an optional exact accepted-edit descriptor with old/new byte endpoints and Tree-sitter points, validated against canonical versions and translated relative to a retained window. |
| `ParseWindowSnapshot` / `DocumentState` slicing | Versioned UTF-8-safe bounded text shape and canonical rope slicing helpers already exist. `ParsePolicy` enforces window and 30 MiB syntax-memory ceilings. | Keep server-canonical bounded text, UTF-8 checks, package/mode provenance, and `SYNTAX_CACHE_BUDGET_BYTES`. | Add stable bounded window identity/selection across adjacent edits and fallback when old/new coverage cannot support exact reuse. Route edit refresh through bounded rope slicing instead of first materializing `DocumentState::text()` as a full `String`. |
| `TreeSitterSyntaxHandler` | Generic grammar/query/vocabulary handler in `src/server/syntax.rs`; one parser mutex per registered grammar handler, per-document cached tree, query timeout, complete capture shaping, and decoration validation. | Keep one generic handler, first-party descriptor data, shared capture-to-`TokenType`/`Modifiers` mapper, parser/query budgets, and server-side execution. | Implemented: exact incremental reuse, changed/visible querying, and complete capture fan-out into stable 128-byte sets without a 32-span truncation cap. |
| `SyntaxChunkCache` | Server-side bounded LRU-style accounting keyed by document/version/package/layer/range; validates each `DecorationSet` before retention. | Keep payload/cache budgets, stale-version separation, and near-viewport eviction. | Implemented: chunks are transport/cache outputs only; syntax and semantic chunks at the same package/range retain separate keys. |
| `DecorationSet` / validation | `src/protocol/decorations.rs` and `src/server/decorations.rs`; inert versioned viewport spans with vocabulary, layer kind, priority, provenance, permission, range, and payload validation. | Keep normal `DecorationSet` transport and existing `render-decorations` validation. No AST/token transport or unbounded set is needed. | Represent affected-range/package ownership even for empty replacement output, then atomically replace overlapping syntax output while preserving unrelated syntax and semantic chunks. Choose the smallest compatible internal batch/stream representation during implementation. |
| Viewport requests | Client emits deduplicated metadata-only `DecorationViewportRequest`; server validates document/version/range and prepares a bounded parse window. | Keep scroll requests separate from edit transport, viewport priority, reserved client queue capacity, and no document text from client to server. | A viewport requests output coverage, not parser job identity. Reuse a current stable tree/capture result or schedule one missing-window parse, then publish visible output first. |
| `EditorDecorationState` | Client-local bounded inert chunk store. `apply_edit` performs generic provisional syntax interpolation over retained chunks; `apply_set` atomically replaces overlapping provisional package/layer ranges; resync/open clears state. | Keep client-local transformation, stale-version checks, near-viewport/cache bounds, and paint consumption of already-normalized inert spans. | Implemented without parser, IPC, package JavaScript, full-document state, or language/delimiter branches. |
| Semantic layering | Shared decoration normalization composes attributes and resolves priority with semantic-over-syntax tie-breaking; diagnostics use a separate source-keyed cache. | Keep additive syntax/semantic rendering and slower semantic refinement over syntax. | Implemented: only syntax overlaps interpolate; intersecting non-syntax geometry invalidates, unaffected layers shift, and syntax replacement remains layer-keyed. |
| First-party grammar budgets | Package/native descriptors supply `maxWindowBytes`; first-party syntax uses 4 KiB windows, stable 128-byte decoration output ranges, parse/query timeouts, 8 KiB decoration payloads, 4 KiB per-member parse-update envelopes, and 30 MiB retained syntax cache. | Preserve bounded large-file input/output and transport ceilings unless measurements justify a separate budget change. | Implemented: output chunk count is independent of native parse/query invocation count. |

## What Existing Primitives Already Achieve

- Canonical document versions and accepted edits remain server-owned.
- Ordinary text mutation and paint remain client-local and do not wait for parser, IPC, package JavaScript, semantic analysis, or filesystem work.
- `ParseCoordinator` already provides cancellation, runtime-generation replacement, stale-result rejection, timeout/error diagnostics, and package-neutral handler dispatch.
- `ParseWindowSnapshot` and `ParsePolicy` already bound parser-visible open-document text and retained syntax memory.
- `TreeSitterSyntaxHandler` already provides a single generic grammar/query/vocabulary path with no Rust/TypeScript/TSX/JavaScript/Markdown control-flow branch.
- `DecorationSet`, `SyntaxChunkCache`, `EditorDecorationState`, and `StyleRegistry` already provide inert bounded transport, cache retention, additive layers, and native paint.
- Existing `parse-document` and `render-decorations` permission/provenance checks remain sufficient; no new package API or permission is required for the optimization.

## Generic Gaps Identified Before Implementation

### 1. Exact accepted-edit descriptor

Add one optional parser-neutral descriptor sufficient to construct Tree-sitter `InputEdit`: old/new byte endpoints and old/new row/column points, tied to base/current document versions. Produce it at canonical server acceptance. Do not derive edit truth from client intent or diff two full snapshots.

### 2. Stable bounded parse window identity

Retain an aligned or otherwise stable bounded window across nearby edits. Exact edits are relative to that identity. If an edit crosses retained coverage, touches unavailable old text, or fails UTF-8/version validation, perform one bounded full-window fallback and record it.

### Accepted-edit and stable-window scheduling now implemented

`ParseInputEdit` and `ParsePoint` in `src/protocol/parse.rs` carry consecutive base/current versions, exact old/new byte endpoints, and zero-based row/byte-column endpoints. `DocumentState::apply_edit_with_parse_input` derives them from the validated canonical rope before mutation; rejected client intent never produces parser metadata. Open, resync, and viewport notifications keep `accepted_edit: None`.

`DocumentState::parse_window_after_edit` retains one bounded window per document/package/grammar stream. The first edit or an incompatible boundary-crossing edit selects a UTF-8-safe half-budget aligned window with headroom and marks it for full fallback. Adjacent edits wholly representable against the retained previous-version range preserve `window_id`/`byte_start`, transform the bounded end by the exact byte delta, and set `incremental_edit`. Hard storage replacement clears retained identities.

`refresh_native_syntax_after_edit` now slices this bounded canonical rope window directly instead of materializing `DocumentState::text()`. `ParseCoordinator` validates edit version/range ordering, stable identity/provenance, current text length and UTF-8 endpoints, implied old/new bounded lengths, and relative points before handler execution. `TreeSitterSyntaxHandler` keys cached trees by `window_id` and permits reuse only for consecutive versions with a matching implied old-window length; all other paths perform a safe bounded full parse.

Exact edit/window metadata is also serialized to constrained server-side JavaScript parse handlers; it adds no client parser or authority.

### 3. One native parse and changed-range capture pass

Logical native work identity is `(runtime generation, document, grammar, stable window, latest version)`. Decoration destinations are outputs, not task-key dimensions. This stage is implemented: the handler applies the exact relative `InputEdit` to the cached tree, incrementally parses with that edited tree, unions `old_tree.changed_ranges(&new_tree)` with the accepted-edit invalidation, clamps to the visible bounded window, expands each edge by at most one UTF-8 scalar, and deterministically sorts/merges ranges. `QueryCursor::set_byte_range` queries the affected envelope, preserving complete intersecting captures such as completed keywords, comments, and strings. Open, viewport-only, version-gap, window-mismatch, or malformed-reuse paths record one bounded full parse/query fallback instead of silently attempting stale reuse.

### 4. Bounded decoration fan-out now implemented

One query/capture result maps completely, then splits broad spans across stable 128-byte output ranges. Chunks intersecting explicit invalidations are ordered before adjacent output. `IncrementalParseUpdate::decoration_updates` is an internal batch: the coordinator validates every member's document/version/range/provenance and decoration/per-member update payload before publishing any member, while the connection streams unchanged `DecorationSet` messages. `DecorationSet` now carries set-level package/layer identity, so empty authoritative chunks clear their exact syntax range and `DecorationChunkKey` layer identity prevents same-package semantic state from being erased.

### 5. Bounded provisional interpolation now implemented

`EditorDecorationState::apply_edit` transforms only already-validated retained near-viewport chunks. Strict-interior insert/replace resizes syntax spans; generic comment/string/regexp/prose/code token families inherit edge insertions; deletion keeps surviving syntax geometry; arithmetic is overflow-checked and reversed ranges fail closed. Intersecting semantic/diagnostic/search spans invalidate instead of becoming provisional, while unaffected spans shift. Changed chunk geometry is marked provisional and edit acknowledgements advance chunk-key versions. Current-version server sets replace overlapping provisional package/layer ranges before installation, including empty authoritative clears. Resync/document replacement clears all provisional state.

### 6. Work-count and latency observability

This pre-refactor baseline is implemented through the existing `PerfRecorder` and `ParseCoordinatorStats`, not a second telemetry system:

- `refresh_native_syntax_after_edit` records one `syntax.parse.logical_work_items` event for each accepted native edit/document version.
- `TreeSitterSyntaxHandler` records actual `syntax.parse.invocations`, `syntax.parse.full`/`syntax.parse.incremental`, and `syntax.query.ranges`/`syntax.query.bytes` work.
- `ParseCoordinator` records validated `syntax.decoration.chunks`, exact-version `syntax.parse.cancelled_superseded`, and one `syntax.edit_to_publish` duration from accepted edit to first current-version native decoration publication.
- Enabled recorders retain at most 4096 snapshots. Disabled production recording remains a no-op; enabled events contain only numeric document/version/count/byte/duration metadata.
- `first_party_incremental_edit` remains the advisory Criterion fixture for Rust, TypeScript, TSX, JavaScript, and Markdown and now reports fixture-byte throughput for parse-through-ready-decoration work.

Deterministic tests lock logical-work/fan-out separation, exact incremental classification, changed-query bytes below the unchanged window for local edits, cancellation metadata, source/path omission, and bounded retention. They intentionally do not hard-fail machine-variable milliseconds.

## Clay JS API Audit

Plan 056 adds no caller-controlled Clay JS capability. `ParseInputEdit`, stable window identity, coordinator coalescing, Tree-sitter reuse/query ranges, decoration fan-out, set-level replacement identity, and `EditorDecorationState` interpolation are server/protocol/client implementation details. They do not receive facade exports, raw ops, custom properties, hidden configuration keys, or per-keystroke callbacks.

Plan 057 likewise adds no caller-controlled Clay JS capability. `replacement_ranges` (UTF-8-safe complete chunk grid), same-word narrow-syntax provisional inheritance (`is_completion_word_character` predicate, `same_word_suffix` flag in `interpolate_decoration_span`), and the shared 128-byte replacement chunk grid are compiled correctness internals. They do not receive facade exports, raw ops, custom properties, or configuration keys.

Plan 058 likewise adds no caller-controlled Clay JS capability. `subtract_half_open_range`, `subtract_provisional_chunk`, `coalesce_local_residual`, `coalesce_compatible_spans`, and `decoration_chunk_byte_size` are private editor-surface functions performing exact half-open authoritative viewport subtraction and local provisional residual coalescing. They do not receive facade exports, raw ops, custom properties, or configuration keys.

Existing public package surfaces remain sufficient: [`parse.serverRegisterParseHandler`](../../reference/clay-js-api/parse/server-register-parse-handler.md) declares a bounded server parser, [`syntax.serverRegisterSyntaxGrammar`](../../reference/clay-js-api/syntax/server-register-syntax-grammar.md) declares validated grammar metadata, and [`decorations.serverPublishDecorations`](../../reference/clay-js-api/decorations/server-publish-decorations.md) publishes inert bounded spans. The parse facade may supply exact accepted-edit metadata to its already registered server-runtime handler, but packages cannot control scheduling, changed-range queries, output chunking, interpolation, or authoritative replacement. Existing provenance, permissions, current-version, and payload validation remain the authority boundary.

`tests/rust_visibility_api_mapping.rs` requires public server Rust items to be mapped in the API inventory or explicitly marked non-JS infrastructure. `tests/clay_js_api_inventory.rs` verifies the existing facade/op/docs/inventory boundaries and rejects Plan 056 and Plan 057 internals as facade exports. No new generated registry entry is needed; the existing registry remains checked for freshness.

## Configuration Audit

Plan 056 adds no `clay:configuration` surface. The stable-window cap, coalescing, fallback/query rules, payload/cache limits, 128-byte output splitting, and provisional interpolation are correctness and latency invariants, so users and packages cannot tune them through debounce, word-boundary, parse-window, chunk-size, interpolation, or client-parser keys. Existing [`syntax.setSyntaxEnginePreference`](../../reference/clay-js-api/syntax/set-syntax-engine-preference.md) remains the sole relevant user choice: its documented `target`/`tier` selects an already validated engine, not scheduling policy.

Plan 057 adds no `clay:configuration` surface. Complete authoritative replacement chunks (query coverage identical to replacement coverage, UTF-8-safe shared chunk grid), same-word narrow-syntax provisional inheritance (Unicode alphanumeric/underscore extends, whitespace/newline/punctuation stops), and unchanged broad-syntax edge behavior are compiled correctness invariants. No `syntaxSameWordBoundary`, `syntaxReplacementChunkGrid`, `syntaxWordInheritance`, `syntaxChunkQueryCoverage`, `syntaxCompleteReplacement`, or `syntaxUtf8ChunkGrid` key exists.

Plan 058 adds no `clay:configuration` surface. Exact-range authoritative viewport subtraction (split provisional chunks into left/right residuals, install authoritative spans, coalesce compatible adjacent residuals) and bounded residual coalescing are compiled correctness invariants. No `syntaxExactRangeReplacement`, `syntaxProvisionalSubtraction`, `syntaxResidualCoalescing`, `syntaxSubtractionCoalescing`, `syntaxExactRangeSubtraction`, `syntaxProvisionalResidual`, or `syntaxCoalescingStrategy` key exists.

Configuration remains outside keypress, text-edit, edit-acknowledgement, parse, publication, paint, layout, and scroll paths. The configuration reference records rejected hidden names and the API inventory test rejects them from configuration custom properties, facades, ops, inventory, and generated registry. No parser callback or external authority is introduced.

## Plan 057 Syntax-Decoration Continuity and Replacement Correctness

Plan 057 (`plans/057-Syntax-Decoration-Continuity-and-Replacement-Correctness.md`, superseding `decision-logs/2026-07-19-1912-syntax-decoration-continuity-and-complete-authoritative-replacement.md`) fixed two root-cause flickering defects discovered during manual testing after Plan 056 completed:

1. **Narrow-span inheritance gap**: Plan 056 `interpolate_decoration_span` only extended broad syntax (Comment, String, Heading1-6, etc.) at token edges; narrow syntax (Keyword, Function, Type, Variable, Number) never inherited inserted characters, so every appended identifier/keyword character painted base-white until authoritative server output arrived.

2. **Wider-than-queried authoritative replacement**: Plan 056 queried only the affected (changed+invalidated) envelope but published full 128-byte replacement chunks. Chunks containing syntax not covered by the query were published with empty spans, clearing all decoration in that range — especially visible after newline insertion.

### Fix 1: Same-Word Narrow-Syntax Provisional Inheritance

`interpolate_decoration_span` in `src/editor/surface/mod.rs` now receives the inserted text content (not just byte length). For narrow syntax spans (kind == Syntax, not broad token family), insertion at `span.byte_end` extends the span only when inserted text is non-empty and every character satisfies `is_completion_word_character` (Unicode `is_alphanumeric()` or `_`). Whitespace, newline, punctuation, brackets, and operators stop inheritance immediately.

Source: `src/editor/surface/mod.rs` — `edit_extent` returns `Option<(u64, u64, &str)>`, `interpolate_decoration_span` computes `same_word_suffix` flag, `is_completion_word_character` predicate.

Tests: `tests/syntax_grammar.rs` — `plan057_function_suffix_stays_decorated_through_local_ack_and_authoritative_states`; `tests/decoration_transport.rs` — `authoritative_syntax_corrects_inherited_suffix_without_clearing_unrelated_spans`; `src/editor/surface/mod.rs` — `optimistic_narrow_token_families_inherit_same_word_suffixes`, `optimistic_narrow_span_stops_at_non_word_boundaries`, `optimistic_narrow_span_inherits_unicode_word_suffix`, `optimistic_non_syntax_layers_do_not_inherit_same_word_suffixes`.

### Fix 2: Complete Authoritative Replacement Chunks

`replacement_ranges` in `src/server/syntax.rs` converts affected (changed+invalidated) ranges into a shared 128-byte UTF-8-safe replacement-chunk grid. The handler queries the full envelope covering every touched replacement chunk, clips captures at exact chunk boundaries, and constructs `DecorationSet` members from the same grid — so query coverage and replacement coverage are identical. Only chunks that intersect the query envelope are published; untouched adjacent chunks are never emitted.

Source: `src/server/syntax.rs` — `replacement_ranges` (shared grid), `decoration_sets_for_ranges` (takes `&[Range<usize>]` instead of single viewport), `parse_sync` computes `affected_ranges` → `replacement_ranges` → passes to `decorations_for_window`.

Tests: `tests/syntax_grammar.rs` — `plan057_newline_keeps_unrelated_short_file_syntax_through_every_state`, `plan057_empty_authoritative_chunk_clears_only_fully_queried_range`, `plan057_utf8_scalar_at_nominal_chunk_boundary_is_never_split`, `plan057_changed_broad_capture_completely_fills_touched_replacement_chunk`; `src/server/syntax.rs` — `replacement_ranges_move_shared_chunk_boundaries_past_utf8_scalars`, `decoration_member_count_does_not_multiply_parse_or_query_invocations`.

### Verification

- `plan057_first_party_languages_keep_continuity_across_edit_boundaries`: 25 composed transition cases over real Rust/TypeScript/TSX/JavaScript/Markdown grammar fixtures covering declaration/string growth, comment/prose newline, paragraph/code-span growth, punctuation, and deletion.
- `plan057_authoritative_queries_correct_inherited_code_keywords`: real grammar output corrects inherited keyword suffixes without collateral clearing.
- `rapid_local_versions_reject_stale_authority_without_losing_provisional_geometry`: stale version 2 authority cannot erase version 3 inherited geometry.
- `first_party_continuity_edits_keep_one_bounded_parse_and_query`: one parser call, one query range, one member per language for all five native descriptors.
- Manual X11 smoke: TypeScript `greet` + `x` remained function-colored, Enter after declaration left syntax decorated, no all-white newline regression.
- Criterion: no statistically significant performance change (Rust ~168 µs, TypeScript ~361 µs, TSX ~126 µs, JavaScript ~123 µs, Markdown ~200 µs).

## Plan 058 Exact-Range Provisional Decoration Replacement

Plan 058 (`plans/058-Exact-Range-Provisional-Decoration-Replacement.md`, superseding `decision-logs/2026-07-19-2238-exact-range-provisional-decoration-replacement.md`) fixed a per-letter downstream whiteness defect discovered after Plan 057 completed: the client optimistically shifts retained provisional chunk geometry by `inserted_len` on every edit, but the server re-anchors authoritative chunks to the fixed 128-byte replacement grid, creating an ever-growing gap between the authoritative chunk end and the next retained provisional chunk start — each typed byte exposed one additional undecorated byte in following code.

### Fix: Exact-Range Authoritative Viewport Subtraction

`apply_set` in `src/editor/surface/mod.rs` no longer deletes entire overlapping provisional chunks. Instead:

1. **Subtract**: `subtract_half_open_range` computes the left/right residual byte ranges outside the authoritative viewport and `subtract_provisional_chunk` splits a crossing provisional chunk into left and right `DecorationResidualSide` fragments, preserving spans whose byte ranges lie outside authority.
2. **Install**: Authoritative spans are inserted inside the authority viewport, replacing any prior overlapping decoration.
3. **Coalesce**: `coalesce_local_residual` merges fragmented residual chunks with adjacent compatible provisional chunks; `coalesce_compatible_spans` merges adjacent spans with identical kind/token_type/modifiers/scope/font_role/priority/provenance within each chunk.

Source: `src/editor/surface/mod.rs` — `DecorationResidualSide` enum, `subtract_half_open_range`, `subtract_provisional_chunk`, `coalesce_local_residual`, `coalesce_compatible_spans`, `decoration_chunk_byte_size`.

Tests:
- `tests/syntax_grammar.rs` — `plan058_repeated_comment_edits_do_not_grow_a_shifted_chunk_boundary_gap` (3 repeated insertions before byte 128, zero undecorated boundary bytes throughout), `plan058_first_party_languages_preserve_shifted_boundary_continuity` (5 first-party languages, 3 repeated insertions each).
- `tests/decoration_transport.rs` — `plan058_empty_authority_after_insertion_preserves_shifted_right_residual`, `plan058_empty_authority_after_deletion_preserves_shifted_right_residual`, `plan058_repeated_insert_delete_authority_cycles_preserve_boundary_geometry` (128 insert/delete pairs).
- `src/editor/surface/mod.rs` — `half_open_subtraction_returns_zero_one_or_two_fragments`, `current_authority_replaces_only_its_viewport_and_coalesces_right_residual`, `authoritative_viewport_splits_crossing_provisional_span`, `repeated_authority_keeps_local_residual_cache_bounded` (512 cycles, exactly 2 chunks/2 spans, retained bytes ≤ `SYNTAX_CACHE_BUDGET_BYTES`), `authoritative_syntax_preserves_other_package_and_semantic_provisional_chunks`.
- `tests/editor_performance_invariants.rs` — `exact_range_decoration_replacement_stays_off_edit_and_paint_hot_paths` (subtraction/coalescing absent from `apply_edit` and `paint` bodies).

### Verification

- Parser/query/member metrics unchanged from Plan 057: one parser call, one query range, one member per language; queried bytes remain Rust 20, TypeScript 26, TSX 26, JavaScript 26, Markdown 17.
- `first_party_authoritative_replacement` Criterion benchmark: one exact authority apply plus local residual coalescing measured 1.8150 µs median (95% interval 1.6250–1.9959 µs, 20 samples).
- Five-language incremental estimates: Rust 152.39 µs, TypeScript 344.39 µs, TSX 125.50 µs, JavaScript 123.55 µs, Markdown 199.49 µs. No statistically significant regression.
- Manual X11 Linux smoke: 150-byte Rust line comment before decorated code, 8 per-letter insertions inside the comment, then Backspace and Enter — all downstream Rust decoration retained with no per-letter white peeling.

## Rejected Alternatives

- **Patch viewport priority only:** leaves up to 16 repeated same-window parses and base-color flashes.
- **Parallel parser scheduler:** duplicates `ParseCoordinator`, cancellation, generation, provenance, stale-version, timeout, and diagnostics logic.
- **One unbounded decoration payload:** violates transport and cache budgets.
- **Whitespace/idle-only parsing:** punctuation, delimiters, operators, EOF, and token/capture transitions are also syntax boundaries.
- **Client Tree-sitter:** duplicates grammar/tree state and broadens provenance/memory complexity before optimized server latency is measured.
- **Language-specific interpolation or invalidation:** grammar queries and generic `TokenType`/`Modifiers` already describe the required boundaries.
- **Package JavaScript in edit/paint paths:** breaks immediate local paint and client authority boundaries.

## Hot-Path and Performance Constraints

- Baseline amplification was explicit: a 4 KiB first-party parse window divided into 256-byte output viewports produced up to 16 native handler jobs, each parsing the same snapshot through a shared per-handler parser mutex.
- Implemented scheduler behavior is one native task per accepted document version/grammar/stable window. Same-version/window requests coalesce; newer versions cancel older stream work; unrelated documents remain independent.
- Implemented output fan-out retains one parser/query invocation while producing complete, independently bounded `DecorationSet` members; changed-visible members drain before adjacent members.
- Ordinary typing and paint remain wait-free relative to parse/IPC/package work.
- Parser input remains server-canonical, UTF-8-safe, versioned, and bounded. Ordinary edit refresh must not copy or transmit a full large document.
- Output remains viewport/near-viewport bounded by `DECORATION_PAYLOAD_BUDGET_BYTES`, `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`, and `SYNTAX_CACHE_BUDGET_BYTES`.
- Cancellation/coalescing is per document/grammar stream, not global across documents or grammars.

## Security and Authority Boundary

- Parsing stays server-side over already-open document text. Canonical versions and exact accepted-edit metadata come from server document state.
- Syntax handlers continue to require `parse-document`; publication continues to require or preserve validated `render-decorations` provenance.
- Tier 1 native grammars remain compiled, resolver-associated first-party descriptors. This work adds no arbitrary third-party native grammar or library loading.
- Client interpolation transforms validated inert spans only. It cannot execute grammar/package code or make provisional output authoritative.
- No filesystem, network, shell, AI, raw-op, native-widget, package-manager, workspace-mutation, client-JavaScript, new WASM artifact, or renderer-callback authority is introduced.
- Metrics contain IDs, versions, counts, byte counts, durations, and fallback categories only—never document text, query text, captures, clipboard contents, absolute paths, or secrets.

## Tests

- Documentation structure and discoverability use generic `tests/primitives_docs.rs` inventory/wiki validators; executable tests remain authoritative for behavior instead of phase-specific prose needles.
- `src/server/syntax.rs::tests::{native_parse_records_source_safe_work_classification_and_query_counts,incremental_parse_queries_less_than_unchanged_window,query_ranges_merge_and_expand_utf8_safe_empty_invalidations}`: verifies exact incremental/full classification, reduced query bytes, and deterministic UTF-8-safe normalization.
- `tests/syntax_grammar.rs::{incremental_keyword_completion_requeries_whole_capture_not_distant_syntax,incremental_comment_opener_requeries_containing_capture,incremental_string_opener_requeries_complete_string_capture,incremental_string_closer_requeries_complete_string_capture,incremental_newline_shortens_line_comment_capture,cached_tree_version_gap_uses_one_bounded_full_parse}`: covers capture boundaries, distant unchanged syntax, and fallback.
- `tests/syntax_grammar.rs::{dense_4k_capture_pass_fans_out_complete_bounded_decoration_sets,changed_decoration_chunk_is_ordered_before_adjacent_chunks}`: verifies complete dense fan-out, per-member budgets, atomic coordinator validation, and visible/changed-first order.
- `tests/syntax_grammar.rs::{first_party_package_queries_keep_authoritative_token_boundaries,first_party_package_queries_keep_broad_captures_continuous}`: runs exact incremental edits through each first-party package query. Keyword/prose-heading, declaration/identifier, punctuation, comments, multiline strings, Markdown prose, code spans, and code blocks retain complete current captures; removal clears only the removed keyword. These tests lock parser/capture boundaries rather than whitespace or idle debounce.
- `tests/language_intelligence.rs::{semantic_span_refines_syntax_while_syntax_chunk_remains_theme_resolved,semantic_publication_rejects_stale_invalid_forged_and_oversize_payloads}`: locks additive slower semantic refinement over retained syntax and rejects stale semantic publication.
- `tests/parse_coordinator.rs::invalid_decoration_batch_member_rejects_whole_update`: proves malformed batch members publish no partial state.
- `tests/decoration_transport.rs::{syntax_chunk_replacement_preserves_semantic_layer,optimistic_comment_style_extends_until_authoritative_replacement}`: locks layer-aware identity and end-to-end inert provisional continuity/correction.
- `src/editor/surface/mod.rs::tests`: covers UTF-8 interior insertion, broad/narrow edges, delete/replace geometry, non-syntax lifecycle, overlapping chunk replacement, reversed edits, and snapshot clearing.
- `src/server/parse_coordinator.rs::tests`: verifies one logical accepted-edit item, chunk fan-out metrics, one first-publication latency sample, and cancellation metadata.
- `tests/performance_protocol.rs::syntax_pipeline_metrics_are_source_safe_and_retention_bounded`: locks metric names, numeric-only metadata, and recorder capacity.
- `src/protocol/parse.rs::tests::exact_edit_and_stable_window_round_trip`: verifies exact edit and stable window metadata survive `rkyv` serialization.
- `src/server/document.rs::tests::accepted_edits_record_exact_utf8_and_newline_coordinates`: covers insert/delete/replace coordinates across multibyte UTF-8 and newlines.
- `src/server/document.rs::tests::adjacent_edits_retain_window_identity_and_crossing_edit_falls_back`: verifies stable adjacent identity, bounded snapshots, and explicit full fallback.
- `tests/parse_coordinator.rs::{exact_accepted_edit_and_window_relative_points_reach_handler,malformed_or_out_of_window_incremental_edit_is_rejected}`: verifies validated handler delivery and fail-closed malformed metadata.
- `tests/editor_performance_invariants.rs::parse_window_snapshot_primitive_uses_bounded_rope_slicing`: rejects full-document materialization in accepted-edit syntax refresh.
- Remaining implementation follow-ups belong in `tests/syntax_grammar.rs`, `tests/decoration_transport.rs`, and `tests/performance_protocol.rs`.

Run current documentation gate:

```bash
cargo test --test protocol primitives_docs::
```

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Parse Coordinator](parse-coordinator.md)
- [Parse Task Lifecycle](parse-task-lifecycle.md)
- [Decoration Transport](decoration-transport.md)
- [Syntax Grammar Registry](syntax-grammar-registry.md)
- [Masonry Editor](masonry-editor.md)
- [Parse Update Strategy](../../reference/primitives/parse-update-strategy.md)
- [Rendering Strategy](../../reference/primitives/rendering-strategy.md)
- [Package Security](../../reference/primitives/package-security.md)
