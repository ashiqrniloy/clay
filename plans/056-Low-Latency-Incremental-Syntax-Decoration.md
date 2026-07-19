# Low-Latency Incremental Syntax Decoration

## Objectives

- Remove repeated same-window Tree-sitter parsing and make accepted edits produce one cancellable/coalesced incremental parse per document grammar stream and version.
- Use stable bounded parse windows, exact edit metadata, changed-range queries, viewport priority, and bounded decoration fan-out without coupling transport chunks to parser jobs.
- Preserve visually continuous token/capture decoration during optimistic local edits, especially inside comments, strings, prose, and code blocks.
- Keep syntax parsing server-side/background, ordinary text paint client-local, semantic decoration additive, and package grammar behavior generic and provenance-validated.

## Expected Outcome

- Typing no longer makes affected tokens flash to the base text color while current syntax is pending in predictable existing captures.
- Each first-party grammar performs at most one parse for a document version/window regardless of how many bounded decoration chunks are published.
- Tree-sitter receives exact edits against reusable stable-window trees; highlight queries run only for explicit invalidations plus Tree-sitter changed ranges.
- Authoritative decoration changes arrive as atomic token/range replacements, while rapid edits cancel/coalesce superseded parse work and stale versions never paint.
- No client parser, language-specific Rust branch, package JavaScript hot-path work, new dependency, hidden configuration key, or new package/platform authority is introduced.

## Tasks

- [x] Review existing parse/decoration primitives and record generic gaps before implementation
  - Acceptance Criteria:
    - Functional: Inventory `ParseCoordinator`, `ParseEditNotification`, `ParseWindowSnapshot`, `TreeSitterSyntaxHandler`, `SyntaxChunkCache`, `DecorationSet`, `EditorDecorationState`, viewport requests, first-party grammar budgets, and semantic layering; map the approved architecture to existing primitives and isolate only generic gaps.
    - Performance: Record the current 4 KiB window / 256-byte viewport amplification (up to 16 same-window native handler jobs), shared parser serialization, moving edit-centered `window_start` cache misses, and current intersecting-span deletion behavior; preserve non-blocking local paint and bounded large-file input.
    - Code Quality: Produce a primitive/gap matrix that separates parse execution, changed-range capture extraction, bounded decoration fan-out, and client interpolation; reject per-language branches and a parallel parser scheduler.
    - Security: Confirm parsing remains server-side over already-open, versioned, bounded document text with existing `parse-document` / `render-decorations` provenance checks; no filesystem, network, shell, AI, raw-op, native-widget, client-JavaScript, or third-party native grammar authority is added.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/{index,registry,parse-update-strategy,rendering-strategy,package-security}.md`
      - `docs/wiki/modules/{primitive-architecture,parse-coordinator,parse-task-lifecycle,decoration-transport,syntax-grammar-registry,masonry-editor}.md`
      - `.agents/skills/project-patterns/references/{mode-primitive-first,protocol-and-performance,authority-boundaries,planning-checklist}.md`
      - `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`
    - Options Considered:
      - Patch only viewport priority: smaller diff, but leaves repeated parsing and base-color flashes.
      - Add a second parser/scheduler: rejected duplication.
      - Extend existing generic parse/decor primitives and client inert-state interpolation. Chosen.
    - Chosen Approach:
      - Create an indexed primitive-review page with current-flow evidence, reusable inventory, generic gaps, ownership, hot-path/security constraints, and rejected alternatives before implementation.
    - API Notes and Examples:
      ```text
      accepted edit -> one ParseInputEdit -> one stable-window parse
                    -> changed ranges -> bounded DecorationSet fan-out
      optimistic edit -> interpolate inert spans -> authoritative range replacement
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/low-latency-incremental-syntax-decoration-primitive-review.md`: primitive inventory and approved gap map.
      - `docs/wiki/index.md`: link the primitive review.
      - `tests/primitives_docs.rs`: deterministic primitive-review coverage.
    - References:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
      - `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md`
  - Test Cases to Write:
    - `low_latency_incremental_syntax_decoration_primitive_review`: requires amplification evidence, one-parse architecture, provisional interpolation, package-neutrality, and security/hot-path boundaries.
    - `cargo test --test primitives_docs`
    - `git diff --check`
  - Completion Evidence:
    - Added the indexed primitive/gap matrix at `docs/wiki/modules/low-latency-incremental-syntax-decoration-primitive-review.md` and deterministic coverage in `tests/primitives_docs.rs`.
    - Verified `cargo fmt --check`, `cargo test --test primitives_docs` (124 passed), and `git diff --check`.

- [x] Add deterministic parse-work and decoration-latency observability before refactoring
  - Acceptance Criteria:
    - Functional: Measure native parse invocations, incremental/full parse classification, queried bytes/ranges, emitted decoration chunks, superseded cancellations, and accepted-edit-to-current-decoration publication latency by document/version without recording document text or paths.
    - Performance: Instrumentation is constant-time/bounded metadata work outside paint; add a repeatable benchmark/fixture for Rust, TypeScript, TSX, JavaScript, and Markdown edits without hard-failing machine-variable wall-clock timing before a stable CI baseline exists.
    - Code Quality: Reuse the existing performance recorder and coordinator stats instead of introducing another telemetry system; counters distinguish parser work from transport fan-out.
    - Security: Metrics contain IDs, versions, counts, durations, and byte counts only—no source, captures, clipboard data, absolute paths, package code, or secrets.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/performance.md`
      - `src/perf/{budgets,metrics}.rs`
      - `benches/first_party_language_baselines.rs`
      - `tests/performance_protocol.rs`
    - Options Considered:
      - Visual-only manual timing: insufficient regression evidence.
      - Hard CI milliseconds immediately: flaky across runners.
      - Deterministic work-count assertions plus recorded benchmark latency. Chosen.
    - Chosen Approach:
      - Add source-safe counters/timers and fixture reporting; lock the structural target of one parse per version/window in tests, then record latency distributions for later threshold promotion.
    - API Notes and Examples:
      ```text
      syntax.parse.invocations{document_id, version}=1
      syntax.query.bytes=<changed/visible bytes>
      syntax.decoration.chunks=<bounded fan-out count>
      syntax.edit_to_publish_ms=<duration>
      ```
    - Files to Create/Edit:
      - `src/perf/metrics.rs`: syntax pipeline metadata/timing helpers if existing recorder methods are insufficient.
      - `src/server/{parse_coordinator,syntax}.rs`: record parse/query/publication work.
      - `src/server/connection.rs`: mark server-accepted native edit/version timing origins.
      - `benches/first_party_language_baselines.rs`: incremental syntax latency/work benchmark.
      - `tests/performance_protocol.rs`: deterministic work-count/budget assertions.
      - `docs/development/performance.md`: benchmark command and advisory interpretation.
      - `docs/wiki/modules/low-latency-incremental-syntax-decoration-primitive-review.md`: implemented observability flow, metrics, limits, and tests.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - One accepted edit records one logical native parse work item and one first-publication latency sample, independent of output chunk count.
    - Native parser/query events distinguish invocations, full/incremental classification, queried ranges, and queried bytes.
    - Metrics omit source/path fields and remain bounded to 4096 retained snapshots.
    - `cargo bench --bench first_party_language_baselines --no-run`
  - Completion Evidence:
    - Added metadata-only syntax work/latency events through the existing `PerfRecorder`, exact-version cancellation metadata through coordinator task state, and a 4096-snapshot recorder ceiling; default-disabled recording remains a no-op outside paint/text-event paths.
    - Extended the five-language `first_party_incremental_edit` Criterion fixture with byte throughput and parse classification output; wall-clock results remain advisory.
    - Verified `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, focused syntax/coordinator unit tests, `cargo test --test performance_protocol`, `cargo test --test parse_coordinator`, `cargo test --test primitives_docs`, `cargo test --test rust_visibility_api_mapping`, `cargo bench --bench first_party_language_baselines --no-run`, and `git diff --check`.

- [x] Carry exact accepted-edit metadata and stable bounded window identity through parse scheduling
  - Acceptance Criteria:
    - Functional: Define a generic versioned parse-edit descriptor sufficient to construct exact Tree-sitter `InputEdit` coordinates; derive it from the accepted server operation; identify stable bounded windows across adjacent edits; force safe full-window fallback when an edit cannot be represented against the cached window.
    - Performance: Ordinary edits do not serialize or copy full documents; stable window selection reuses the same cache identity across nearby typing; large-file windows remain within package policy and syntax memory budgets.
    - Code Quality: Protocol types remain parser/language-neutral, UTF-8 checked, and explicit about old/new byte and point ranges; open/resync/viewport paths remain valid without fake edit metadata.
    - Security: Only server-canonical accepted edit metadata and already-authorized bounded snapshots reach handlers; all ranges, versions, provenance, and budgets are validated before execution.
  - Approach:
    - Documentation Reviewed:
      - Tree-sitter Rust 0.25.10 local rustdoc/source for `InputEdit`, `Tree::edit`, and `Parser::parse(..., old_tree)`, generated with `CARGO_TARGET_DIR=/tmp/clay-tree-sitter-doc cargo doc -p tree-sitter --no-deps`.
      - Context7 `/tree-sitter/tree-sitter` Rust editing and zero-based row/byte-column point documentation.
      - [Tree-sitter incremental editing](https://github.com/tree-sitter/tree-sitter/blob/f45a488dea5c98a93721566a2098a658dea73ecd/docs/src/using-parsers/3-advanced-parsing.md#L3-L22)
      - `src/{protocol/parse,server/document,server/connection}.rs`
    - Options Considered:
      - Whole-window replacement edit: simple but defeats precise reuse.
      - Edit-centered windows: current behavior; unstable cache key.
      - Stable aligned/retained window identity plus exact relative edit, with bounded fallback. Chosen.
    - Chosen Approach:
      - Extend existing parse notification/window primitives minimally; produce edit metadata at canonical acceptance, translate it relative to the selected retained window, and reject/fallback when old/new coverage is insufficient.
    - API Notes and Examples:
      ```rust
      tree.edit(&InputEdit {
          start_byte,
          old_end_byte,
          new_end_byte,
          start_position,
          old_end_position,
          new_end_position,
      });
      let new_tree = parser.parse(new_text, Some(&tree));
      ```
    - Files to Create/Edit:
      - `src/protocol/parse.rs`: generic parse-edit/window identity shapes and serialization test.
      - `src/server/document.rs`: canonical accepted-edit derivation, retained bounded windows, UTF-8/point helpers, and unit tests.
      - `src/server/connection.rs`: capture accepted edit metadata, slice bounded canonical windows, and schedule it.
      - `src/server/parse_coordinator.rs`: validate/forward metadata.
      - `src/server/syntax.rs`: key cache eligibility by stable window identity and force bounded full fallback when exact reuse is unsafe.
      - `src/server/js_runtime.rs`: forward generic edit/window metadata to authorized server-side JS handlers.
      - `tests/{parse_coordinator,performance_protocol,syntax_grammar,editor_performance_invariants}.rs`: round-trip, bounds, fallback, compatibility, and no-full-document checks.
      - `benches/{first_party_language_baselines,markdown_baselines}.rs`, `src/server/mod.rs`, and `tests/markdown_mode.rs`: compile-compatible protocol fixture updates.
      - `docs/wiki/modules/low-latency-incremental-syntax-decoration-primitive-review.md`: implemented flow, invariants, fallback, and tests.
    - References:
      - `.agents/skills/project-patterns/references/{protocol-and-performance,authority-boundaries}.md`
  - Test Cases to Write:
    - Insert/delete/replace with ASCII, multibyte UTF-8, newline insertion/deletion, and window-relative points.
    - Exact edit and stable-window metadata survive `rkyv` serialization and handler delivery.
    - Adjacent keystrokes retain stable window identity and remain within policy bounds.
    - Boundary-crossing edits safely fall back without stale-tree reuse.
    - Oversized, mismatched-version, out-of-window, malformed point, and mismatched window identity metadata is rejected.
    - Accepted-edit syntax refresh slices the canonical rope without materializing a full document string.
  - Completion Evidence:
    - Added parser-neutral `ParsePoint`/`ParseInputEdit` metadata derived before canonical rope mutation, with consecutive versions, exact byte endpoints, and zero-based row/byte-column endpoints; open/resync/viewport work carries no fabricated edit.
    - Added retained per-document/package/grammar windows with stable aligned identity, bounded headroom, exact delta end transformation, UTF-8-safe slicing, and explicit full fallback for first edit, boundary crossing, stale retention, or incompatible cached length.
    - Coordinator validation now rejects malformed exact edits, relative point mismatches, invalid stable identities, provenance/version mismatch, and over-budget/implied old-window ranges before execution; ordinary edit refresh no longer copies full document text.
    - Tree-sitter cache reuse is restricted to consecutive versions with matching stable identity and implied old-window length. Exact `Tree::edit` application and changed-range queries remain in the later Tree-sitter reuse task.
    - Verified version-exact Tree-sitter 0.25.10 local docs/source plus Context7 `/tree-sitter/tree-sitter`, focused protocol/document/syntax/connection/JS-runtime tests, and the `parse_coordinator`, `performance_protocol`, `syntax_grammar`, `editor_performance_invariants`, and `primitives_docs` suites.

- [x] Parse once per document version and coalesce superseded native syntax work
  - Acceptance Criteria:
    - Functional: Native scheduling creates one parse task for each `(generation, document, grammar, version, stable window)` rather than one task per decoration viewport; rapid newer edits cancel/coalesce older work while the latest version always runs; viewport requests reuse current trees or schedule one missing-window parse.
    - Performance: The parser mutex sees at most one parse for a version/window; no global serialization is added across documents or grammars; edit acknowledgement and local paint remain wait-free.
    - Code Quality: Reuse `ParseCoordinator` task lifecycle and stats, changing task identity/priority rather than adding a native-only scheduler; JS fallback handlers preserve their current generic lifecycle.
    - Security: Generation, package provenance, permissions, versions, timeout, and memory/payload validation remain fail-closed; cancellation cannot publish old-generation or stale results.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/{parse-coordinator,parse-task-lifecycle}.md`
      - `src/server/parse_coordinator.rs`
      - [Zed foreground/background reparse lifecycle](https://github.com/zed-industries/zed/blob/edeaf598c7495bd7b9e9a05d68e61f08ad275d16/crates/language/src/buffer.rs#L1832-L1922)
    - Options Considered:
      - Keep viewport start in parser task key: permits sibling duplicates.
      - Global single parse queue: blocks unrelated documents.
      - Per-document/grammar/version/window task identity with latest-version supersession. Chosen.
    - Chosen Approach:
      - Separate logical parse identity from decoration destination ranges; preserve per-document concurrency and existing generation cancellation.
    - API Notes and Examples:
      ```text
      ParseTaskKey = generation + document + grammar + window
      current_version[ParseTaskKey] = latest accepted version
      decoration chunks are outputs, never ParseTaskKey dimensions
      ```
    - Files to Create/Edit:
      - `src/server/parse_coordinator.rs`: task identity, cancellation/coalescing, stats.
      - `src/server/connection.rs`: one schedule call per edit/window.
      - `tests/parse_coordinator.rs`: one-parse, supersession, viewport reuse, multi-document concurrency.
      - `tests/primitives_docs.rs`: deterministic coverage for implemented scheduler/coalescing documentation.
      - `docs/wiki/modules/{parse-coordinator,parse-task-lifecycle,syntax-grammar-registry,low-latency-incremental-syntax-decoration-primitive-review}.md`: implemented scheduler identity, lifecycle, and pending output fan-out boundary.
    - References:
      - `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`
  - Test Cases to Write:
    - A 4 KiB first-party window schedules one parse, not 16.
    - Ten rapid versions publish only the latest current result and bounded cancellation stats.
    - Two documents parse independently.
    - Runtime-generation replacement cancels old work.
  - Completion Evidence:
    - Native open/edit/viewport scheduling now submits one full stable bounded window. Coordinator task identity uses generation/document/package/mode/window, coalesces duplicate same-version/window requests, rejects older scheduling, and cancels older stream versions even when window identity changes.
    - Added a start gate plus active-version completion check so a fast or aborted older task cannot remove or publish over the latest task. Existing generation/package cancellation and JS fallback lifecycle remain shared and fail closed.
    - Added deterministic coverage for one handler invocation across sibling viewport requests, ten-version latest-only publication with nine bounded cancellations, independent two-document execution, first-party one-window scheduling, and existing runtime-generation replacement.
    - Verified `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --test parse_coordinator` (28 passed), `cargo test --test primitives_docs` (124 passed), `cargo test --test syntax_grammar` (49 passed), `cargo test --test performance_protocol` (19 passed), focused first-party/open non-blocking library tests, and `git diff --check`.

- [x] Reuse Tree-sitter trees and query only changed/visible ranges
  - Acceptance Criteria:
    - Functional: `TreeSitterSyntaxHandler` applies exact relative edits to a matching cached tree, incrementally reparses once, unions explicit invalidations with `old_tree.changed_ranges(new_tree)`, expands only enough to recover intersecting query captures, and queries visible/changed ranges.
    - Performance: Unchanged subtrees are reused; query work scales with changed/visible ranges rather than the complete 4 KiB window for each output chunk; parser/query timeout and memory ceilings remain enforced.
    - Code Quality: Changed-range normalization/merge is generic, deterministic, UTF-8 safe, and tested independently; full parse is an explicit fallback with instrumentation, not silent default behavior.
    - Security: Package queries and style maps remain prevalidated; capture output stays bounded, inert, provenance-stamped, and source-free outside authorized spans.
  - Approach:
    - Documentation Reviewed:
      - [Tree-sitter query range API](https://github.com/tree-sitter/tree-sitter/blob/f45a488dea5c98a93721566a2098a658dea73ecd/docs/src/using-parsers/queries/4-api.md#L65-L84)
      - [Zed changed-range union and expansion](https://github.com/zed-industries/zed/blob/edeaf598c7495bd7b9e9a05d68e61f08ad275d16/crates/language/src/syntax_map.rs#L664-L829)
      - `src/server/syntax.rs`
    - Options Considered:
      - Query complete window after one parse: acceptable fallback, but unnecessary work.
      - Query raw edit bytes only: misses captures whose parent syntax changed.
      - Union Tree-sitter changed ranges with explicit invalidations and intersect viewport. Chosen.
    - Chosen Approach:
      - Preserve the edited old tree through current parse, derive/merge changed ranges, and use `QueryCursor::set_byte_range` over their smallest affected envelope with one-scalar UTF-8 edge expansion. One contiguous envelope keeps current single-`DecorationSet` replacement correct; the next task can query/publish disjoint ranges separately during bounded fan-out.
    - API Notes and Examples:
      ```rust
      let changed = old_tree.changed_ranges(&new_tree);
      cursor.set_byte_range(start..end); // intersecting matches included
      ```
    - Files to Create/Edit:
      - `src/server/syntax.rs`: exact incremental parse, changed-range normalization/query, cache state.
      - `src/server/connection.rs`: accepted-edit invalidation range instead of full-window edit invalidation.
      - `tests/syntax_grammar.rs`: reuse, changed-range, delimiter/comment/string, stale/fallback coverage.
      - `tests/editor_performance_invariants.rs`: no parser/query work enters client hot paths and current cancellation guard.
      - `docs/wiki/modules/{syntax-grammar-registry,low-latency-incremental-syntax-decoration-primitive-review}.md`: exact tree reuse, query-range algorithm, fallback, and tests.
    - References:
      - Local version-exact Tree-sitter rustdoc/source generated/inspected per `AGENTS.md`.
  - Test Cases to Write:
    - Keyword completion changes the whole capture.
    - String/comment opener and closer invalidate the containing capture.
    - Newline changes line-comment extent.
    - Unchanged distant captures are not requeried or republished.
    - Cached-tree mismatch uses one bounded full parse and records fallback.
  - Completion Evidence:
    - `TreeSitterSyntaxHandler` now reuses only a consecutive matching stable-window tree, converts exact server-relative coordinates to Tree-sitter 0.25.10 `InputEdit`, applies `Tree::edit`, and passes the edited tree to `Parser::parse`; version/window/length mismatch remains one instrumented bounded full fallback.
    - Incremental query input now unions `old_tree.changed_ranges(&new_tree)` with the accepted-edit invalidation, clamps to the visible bounded window, expands edges by at most one UTF-8 scalar, sorts/merges deterministically, and queries only the resulting affected envelope. Full/open/viewport work explicitly queries the bounded visible window.
    - Changed output retains complete intersecting captures and publishes matching affected-range update/decor metadata; package style/provenance, capture/payload/cache limits, stale validation, and parser/query timeouts remain unchanged. Multi-set decoration fan-out stays isolated to the next task.
    - Added deterministic coverage for whole keyword completion without distant syntax republication, comment opener, string opener/closer, newline-shortened line comment, UTF-8-safe range merge/expansion, reduced incremental query bytes, metadata validation, and version-gap full fallback.
    - Reviewed Context7 `/tree-sitter/tree-sitter` plus version-exact local Tree-sitter 0.25.10 rustdoc/source (`Tree::edit`, `Tree::changed_ranges`, `QueryCursor::set_byte_range`). Verified `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `syntax_grammar` (55 passed), `parse_coordinator` (28 passed), `editor_performance_invariants` (22 passed), `performance_protocol` (19 passed), `primitives_docs` (124 passed), and focused native-window tests.

- [x] Separate one parse/capture pass from bounded decoration chunk publication
  - Acceptance Criteria:
    - Functional: One native parse/capture result can fan out into all required current-version `DecorationSet` chunks for changed and visible ranges; chunks replace affected syntax regions without deleting unrelated syntax or semantic chunks.
    - Performance: Each IPC/update remains within existing decoration/incremental payload ceilings and client syntax-cache budget; chunk count does not increase parser/query invocation count; visible changed output is emitted before adjacent output.
    - Code Quality: Preserve generic `DecorationSet` validation and cache keys; choose the smallest compatible batch/stream shape after primitive review, avoiding duplicate native and JS publication paths.
    - Security: Every emitted chunk is independently validated for document/version/viewport/provenance/style/payload; malformed batch members fail atomically or are rejected before any partial unsafe state is published.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/decoration-transport.md`
      - `src/{protocol/decorations,server/decorations,server/parse_coordinator,server/syntax}.rs`
    - Options Considered:
      - Increase one set without bounds: rejected.
      - Reparse once per chunk: current bug.
      - Parse/query once, then split validated capture records into bounded output chunks. Chosen.
    - Chosen Approach:
      - Keep normal `DecorationSet` transport/cache chunks as output units. `IncrementalParseUpdate::decoration_updates` is the minimal internal batch: one handler result carries stable 128-byte sets in changed-visible-first order, coordinator validation is all-or-nothing, and the connection streams ordinary set messages.
    - API Notes and Examples:
      ```text
      ParsedWindow { tree, captures }
        -> split_by_bounded_viewport_and_payload(captures)
        -> DecorationSet[]
        -> validate each -> visible-first publication
      ```
    - Files to Create/Edit:
      - `src/protocol/parse.rs`: internal multi-set handler result.
      - `src/protocol/decorations.rs`: set-level package/layer identity for empty authoritative replacements and layer-aware keys.
      - `src/server/{syntax,parse_coordinator,decorations,connection}.rs`: complete capture fan-out, atomic member validation, and visible-first normal-set drain.
      - `src/editor/surface.rs`: exact package/layer/range replacement, including empty syntax sets without erasing semantic chunks.
      - `tests/{syntax_grammar,parse_coordinator,decoration_transport,performance_protocol}.rs`: dense fan-out, ordering, atomic rejection, layering, and payload/work bounds.
      - `docs/wiki/modules/{syntax-grammar-registry,parse-coordinator,decoration-transport,low-latency-incremental-syntax-decoration-primitive-review}.md`: implemented batch, fan-out, replacement, limits, and tests.
      - Protocol fixture/benchmark initializers: compile-compatible set-level identity and multi-set handling.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Test Cases to Write:
    - Dense 4 KiB fixture publishes all syntax spans through bounded sets after one parse.
    - Visible changed chunk publishes before adjacent chunks.
    - One invalid/oversized chunk cannot install malformed or mixed-version state.
    - Semantic spans survive syntax-range replacement.
  - Completion Evidence:
    - Removed the 32-capture truncation path. One Tree-sitter parse/query/capture pass now maps complete output, splits broad captures across stable 128-byte ranges, emits empty authoritative syntax replacements, and orders chunks intersecting explicit invalidations before adjacent output.
    - `IncrementalParseUpdate::decoration_updates` carries one internal batch. `ParseCoordinator` validates every member's document/version/enclosing range/package provenance, normal decoration budget, and per-member incremental-update budget before publishing any member; one malformed member rejects the whole result. The connection drains validated members through unchanged `ServerMessage::DecorationSet` transport.
    - `DecorationSet` now carries set-level package/layer identity, and `DecorationChunkKey` includes layer kind. Empty syntax chunks clear the exact key while same-package semantic chunks at the same range survive syntax replacement.
    - Added deterministic dense-4-KiB completeness/payload coverage, changed-first ordering, atomic invalid-member rejection, empty replacement, syntax/semantic layer survival, and per-language multi-chunk window coverage. Chunk metrics now count emitted members while parse/query invocation metrics remain independent.
    - Verified `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, `syntax_grammar` (56 passed), `parse_coordinator` (29 passed), `decoration_transport` (14 passed), `performance_protocol` (19 passed), `primitives_docs` (124 passed), `editor_performance_invariants` (22 passed), plus focused native-window, open-background, JavaScript-handler, semantic-layer, and typography tests.

- [x] Interpolate client decoration spans through optimistic edits
  - Acceptance Criteria:
    - Functional: Insertions/deletions/replacements shift unaffected spans; insertion strictly inside a syntax capture extends it provisionally; deletion shrinks/splits or invalidates only unsafe affected geometry; broad comment/string/prose/code captures may inherit edge insertions; authoritative current-version sets replace provisional affected ranges atomically.
    - Performance: Interpolation is bounded by retained near-viewport chunks/spans, allocates no full-document state, executes no parser/IPC/package JavaScript, and keeps ordinary text paint immediate.
    - Code Quality: Rules operate on generic `DecorationKind`, `TokenType`, modifiers, byte ranges, and edit operations; no language/delimiter-specific Rust conditions; semantic, diagnostic, and search layer lifecycle remains explicit.
    - Security: Client only transforms already-validated inert spans; it does not infer package authority, execute grammar code, inspect filesystem, or make provisional spans authoritative across resync/version mismatch.
  - Approach:
    - Documentation Reviewed:
      - `src/editor/surface.rs::EditorDecorationState`
      - `docs/wiki/modules/{decoration-transport,masonry-editor}.md`
      - [VS Code inserted-character tokenization](https://github.com/microsoft/vscode/blob/56d6f639fb09e6610c9eb8f56439496b9536e283/src/vs/editor/common/model/textModelTokens.ts#L75-L101)
      - [Zed syntax interpolation](https://github.com/zed-industries/zed/blob/edeaf598c7495bd7b9e9a05d68e61f08ad275d16/crates/language/src/syntax_map.rs#L329-L449)
    - Options Considered:
      - Drop every intersecting span: current flash.
      - Preserve all stale spans blindly: wrong geometry.
      - Edit-aware provisional interpolation with bounded fallback invalidation. Chosen.
    - Chosen Approach:
      - Extend `EditorDecorationState::apply_edit` into one deterministic retained-chunk transform. Strict-interior syntax edits resize; generic broad `TokenType` families inherit edge insertions; deletion/replacement keeps surviving syntax; intersecting non-syntax spans invalidate while unaffected layers shift. Mark transformed chunks provisional, advance their key versions on acknowledgement, and let normal current server sets replace exact plus overlapping provisional package/layer ranges.
    - API Notes and Examples:
      ```text
      [comment span: 10..30] + insert at 20 len 3 -> provisional 10..33
      [keyword span: 10..12] + insert at 12      -> await authoritative token result
      ```
    - Files to Create/Edit:
      - `src/editor/surface.rs`: interpolation, provisional chunk tracking, versioned key advancement, and atomic affected-range replacement; existing style-revision hooks were sufficient, so no layout/widget code changed.
      - `tests/decoration_transport.rs`: end-to-end inert comment continuity and authoritative empty correction.
      - `docs/wiki/modules/{decoration-transport,masonry-editor,low-latency-incremental-syntax-decoration-primitive-review}.md`: implemented geometry, layer lifecycle, authority, limits, and tests.
    - References:
      - `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`
  - Test Cases to Write:
    - Interior comment/string/prose/code insert keeps continuous style.
    - Interior keyword/identifier insertion keeps geometry until authoritative whole-token replacement.
    - Edge insert, newline, delete, replace, UTF-8, overlapping chunks, and reversed edit cases stay bounded and valid.
    - Resync/document switch clears provisional state.
    - Syntax interpolation does not mutate semantic/diagnostic/search ownership incorrectly.
  - Completion Evidence:
    - `EditorDecorationState::apply_edit` now uses overflow-checked byte geometry for insert/delete/replace. It shifts unaffected spans, extends every syntax token on strict-interior insertion, preserves surviving syntax through removals, and applies edge inheritance only to generic comment/string/regexp/prose/code `TokenType` families. Reversed ranges fail closed.
    - Intersecting semantic/diagnostic/search spans are removed rather than made provisional; unaffected instances shift normally. Work remains bounded to retained near-viewport chunks/spans and calls no parser, IPC, package JavaScript, filesystem, or full-document path.
    - Changed chunks are marked provisional, chunk-key geometry follows edits, and `EditAck` advances each retained key version. A current `DecorationSet` removes its exact key plus overlapping provisional keys for the same package/layer before installation, so empty authoritative output corrects syntax atomically without clearing semantic ownership. Snapshot/resync replacement still clears all state.
    - Added unit coverage for UTF-8 interior insertion, broad comment/string/prose/code edge and newline inheritance, narrow keyword edges, syntax deletion/replacement, intersecting semantic invalidation, unaffected semantic/diagnostic/search shifts, overlapping chunk correction, reversed edits, and document replacement. Added `optimistic_comment_style_extends_until_authoritative_replacement` integration coverage.
    - Verified `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, all 885 library tests, `decoration_transport` (15 passed), `editor_performance_invariants` (22 passed), `primitives_docs` (124 passed), `performance_protocol` (19 passed), and `git diff --check`.

- [x] Verify token-boundary behavior and syntax/semantic layering across first-party languages
  - Acceptance Criteria:
    - Functional: Rust, TypeScript, TSX, JavaScript, and Markdown fixtures demonstrate whole-token transitions for keywords/identifiers/operators and continuous broad-span decoration for comments, strings, prose, and code blocks; whitespace, newline, punctuation, quotes, brackets, operators, caret movement, and idle are treated as UI/token events—not the sole parse trigger.
    - Performance: Rapid typing produces bounded latest-version work and no letter-by-letter parse amplification; slower semantic updates refine syntax without clearing it.
    - Code Quality: Tests use package queries/style maps and generic capture/token types; no first-party language-name branch enters scheduling, interpolation, or paint.
    - Security: Existing package provenance, grammar tier selection, permission checks, and semantic publication validation remain unchanged.
  - Approach:
    - Documentation Reviewed:
      - `packages/{rust,typescript,javascript,markdown}/queries/highlights.scm`
      - `tests/fixtures/syntax/*`
      - Context7 `/tree-sitter/tree-sitter`: `Tree::edit`, `Parser::parse` with an old tree, changed-range diffs, and byte-range queries return intersecting captures.
      - VS Code syntax/semantic highlighting guides and existing Clay layered-decoration decision.
    - Options Considered:
      - Explicit whitespace debounce: rejected by approved decision.
      - Parser/capture-defined token transitions plus provisional span continuity. Chosen.
    - Chosen Approach:
      - Add table-driven edit sequences over real package fixtures and assert range-level states before and after authoritative updates.
    - API Notes and Examples:
      ```text
      "le" -> "let": authoritative result replaces entire token as Keyword
      "// note" -> "// notes": provisional Comment extends immediately
      syntax(Function) + semantic(Function, Declaration) -> semantic refinement over syntax
      ```
    - Files to Create/Edit:
      - `tests/syntax_grammar.rs`: first-party package-query edit-sequence matrix; existing fixture smoke remains sufficient, so no new fixture files are needed.
      - `tests/language_intelligence.rs`: existing semantic preservation/refinement and stale-publication coverage is sufficient.
      - `docs/wiki/modules/{syntax-grammar-registry,low-latency-incremental-syntax-decoration-primitive-review}.md`: package-query boundary, layering, and no-debounce implementation/test knowledge.
    - References:
      - `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md`
  - Test Cases to Write:
    - Keyword completion/removal, identifier growth, punctuation/operator insertion.
    - Line/block comment and single/multiline string growth/closure.
    - Markdown prose, heading, emphasis, code span/block boundary edits.
    - Semantic arrival after syntax and stale semantic rejection.
  - Completion Evidence:
    - Added two table-driven exact-edit matrices over the real first-party package queries and native grammars. Rust, TypeScript, TSX, JavaScript, and Markdown verify current authoritative keyword/prose-heading, declaration/identifier, and punctuation ranges after completion/growth/insertion; Rust/TypeScript/TSX/JavaScript keyword removal proves only the removed capture disappears.
    - The same real-query path verifies complete current broad captures after edits: Rust line comments/raw multiline strings; TypeScript/TSX/JavaScript block comments/template strings; Markdown prose, code spans, and fenced code blocks. Existing delimiter/comment tests retain opener/closer/newline recovery coverage. No scheduler, interpolator, or paint branch changed; parser/capture boundaries—not whitespace or idle—remain source of truth.
    - Confirmed existing generic layer tests are sufficient: semantic arrival retains syntax and theme-resolves its refinement, while stale/forged/invalid semantic publication fails validation. Existing coordinator latest-version coverage confirms rapid edits coalesce/cancel superseded work without parse work per output chunk.
    - Updated the syntax registry and low-latency primitive-review wiki pages with matrix behavior, package-owned capture boundaries, layering, and test paths.
    - Verified Context7 `/tree-sitter/tree-sitter`, `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `syntax_grammar` (58 passed), `language_intelligence` (31 passed), `parse_coordinator` (29 passed), `decoration_transport` (15 passed), `performance_protocol` (19 passed), `primitives_docs` (124 passed), `editor_performance_invariants` (22 passed), and `git diff --check`.

- [x] Update primitive/package grammar documentation and verify one-line package loading
  - Acceptance Criteria:
    - Functional: Public primitive and package-author documentation explains one parse per edit/window, exact edits, changed-range querying, bounded output fan-out, provisional client interpolation, and token/capture boundaries; existing `loadPackage("@clay/<language>")` one-line setup still activates normal defaults without manual parser/decorator plumbing.
    - Performance: Documentation preserves 4 KiB package window ceilings or records any evidence-backed budget adjustment, one-parse semantics, viewport output bounds, and no package work in paint/text-event hot paths.
    - Code Quality: Update existing generic grammar/parse/decor contracts rather than introducing language-specific APIs; deterministic docs tests prevent old “256-byte sibling parse jobs” text from returning.
    - Security: Package grammar authority remains first-party/resolver-validated as currently decided; no new third-party native/WASM, filesystem, network, shell, AI, raw-op, client-JavaScript, or native-widget authority is implied.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/{index,registry,parse-update-strategy,rendering-strategy}.md`
      - `docs/reference/packages/creating-packages.md`
      - `docs/wiki/modules/syntax-grammar-registry.md`
      - `.agents/skills/create-plan/references/clay.md` package grammar/default-loading requirements.
    - Options Considered:
      - Internal-only documentation: rejected because package performance contract changes.
      - New public configuration/API: unnecessary.
      - Update existing primitive/package contracts and loading tests. Chosen.
    - Chosen Approach:
      - Keep syntax contribution schema unchanged unless implementation proves a generic field is required; document scheduler semantics as Clay-owned and verify all first-party one-line loads.
    - API Notes and Examples:
      ```js
      import { loadPackage } from "clay:packages";
      await loadPackage("@clay/rust");
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/{index,registry,parse-update-strategy,rendering-strategy}.md`: implemented primitive semantics.
      - `docs/reference/packages/creating-packages.md`: grammar author performance/lifecycle contract.
      - `tests/{primitives_docs,package_loading_docs,syntax_grammar}.rs`: deterministic docs/loading coverage.
    - References:
      - `.agents/skills/project-patterns/references/{mode-primitive-first,documentation-as-code}.md`
  - Test Cases to Write:
    - All first-party one-line `loadPackage` fixtures still register grammar/query/style maps and highlight edits.
    - Docs require one-parse/fan-out/interpolation/token-boundary language and reject whitespace-only scheduling.
    - Package author docs retain authority and hot-path prohibitions.
  - Completion Evidence:
    - Updated the public primitive index, registry, parse-update strategy, rendering strategy, and package-author guide. They now describe one exact `ParseInputEdit` and one parse/capture pass per accepted stable version/window; UTF-8-safe changed-range plus invalidation querying; stable 128-byte changed/visible-first output fan-out; atomic bounded member validation; empty authoritative syntax replacements; provisional inert-span interpolation; grammar-owned token/capture boundaries; the existing 4 KiB window ceiling; and existing no-hot-path/authority limits.
    - Added a deterministic primitive-doc guard requiring the one-parse/fan-out contract across every public reference surface, the 4 KiB `maxWindowBytes` example, and rejection of obsolete `256-byte sibling parse jobs` wording. Existing package docs tests now require explicit Rust, TypeScript, JavaScript, and Markdown one-line loads; the runtime `syntax_grammar_packages_default_load_from_init_js` integration test loads and verifies all four package defaults in one `init.js` run. Existing fixture/query tests retain grammar/query/style-map/highlight coverage.
    - No syntax schema, configuration option, parser authority, or language-specific scheduler branch was added. `loadPackage` remains explicit, resolver-validated, and capability-neutral; package parser work remains server-owned background work outside typing/paint/layout/scroll/text-event paths.
    - Verified `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `primitives_docs` (125 passed), `package_loading_docs` (52 passed), `syntax_grammar` (58 passed), `performance_protocol` (19 passed), `editor_performance_invariants` (22 passed), `decoration_transport` (15 passed), and `git diff --check`.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Audit all changed Rust functions/types and determine whether this plan adds any public programmatic behavior; expose only genuine public capabilities through documented Clay JS facade/op APIs, otherwise keep implementation private or `pub(crate)` and record that existing syntax/decoration APIs are sufficient.
    - Performance: No API permits per-keystroke JavaScript callbacks, dynamic parse scheduling, raw decoration interpolation, or paint-path execution.
    - Code Quality: Any required API follows stable ID, user-facing name, key binding/custom property, docs, inventory, facade, op, generated registry, lookup, and test conventions; raw Rust functions and `Deno.core.ops` are not user-facing APIs.
    - Security: APIs cannot bypass package provenance, permissions, grammar validation, document versions, payload budgets, or server-side parser ownership.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` Clay JS API requirement.
      - `.agents/skills/project-patterns/references/{clay-js-api-boundary,clay-js-api-naming,clay-js-api-schema,documentation-as-code,doc-registry-tests}.md`
      - Existing `clay:syntax`, `clay:parse`, and `clay:decorations` reference docs/inventory.
    - Options Considered:
      - Expose scheduler/interpolation tuning: rejected unless implementation finds a real package/user need.
      - Keep optimization internal behind existing APIs. Expected/chosen default.
    - Chosen Approach:
      - Perform the audit after implementation; add no API solely for observability or speculative tuning.
    - API Notes and Examples:
      ```text
      Expected public surface change: none.
      Existing package surface: serverRegisterSyntaxGrammar / serverRegisterParseHandler / serverPublishDecorations.
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`, `docs/index.md`, `docs/reference/clay-js-api/api-inventory.toml`, facade/op files, and registry tests only if audit proves a new public capability.
      - `src/bin/update-doc-registry.rs` output via `cargo run --bin update-doc-registry` only when API docs change.
    - References:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
  - Test Cases to Write:
    - Rust visibility/API mapping tests cover changed public functions.
    - Existing syntax/parse/decoration facade, inventory, docs registry, and lookup tests remain green.
    - Regenerated registry matches sources if docs change.
  - Completion Evidence:
    - Audited the changed scheduler, Tree-sitter, protocol, decoration, and client-interpolation surfaces. `ParseInputEdit`, stable window/task identity, coalescing, changed-range queries, bounded `decoration_updates`, package/layer chunk identity, and `EditorDecorationState` interpolation are Clay-owned server/protocol/client mechanics, not caller-controlled behavior. No new Clay JS facade, deno op, permission, configuration property, API inventory entry, or generated-registry entry is justified.
    - Verified existing public contracts remain sufficient: `clay.parse.serverRegisterParseHandler` owns bounded server parser declaration, `clay.syntax.serverRegisterSyntaxGrammar` owns validated grammar metadata, and `clay.decorations.serverPublishDecorations` owns inert validated span publication. The parse API now explicitly documents that it may supply read-only exact accepted-edit metadata to an already-registered server handler, while scheduling, changed-range query, output fan-out, interpolation, and authoritative replacement remain internal.
    - Added a deterministic API-boundary audit: it requires those three existing inventory IDs, requires the parse API and low-latency wiki audit to state the no-new-control result, and rejects internal Plan 056 names as parse/syntax/decoration facade exports. The wiki links authoritative API docs and records that the existing generated registry remains fresh; `cargo run --bin update-doc-registry` confirms no new metadata entry.
    - Verified `cargo fmt --check`, `cargo test --test clay_js_api_inventory`, `cargo test --test rust_visibility_api_mapping` (17 passed), `cargo test --test clay_js_doc_registry` (34 passed), `cargo test --test primitives_docs low_latency_incremental_syntax_decoration_primitive_review`, and `git diff --check`.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Review whether users/packages need a documented configuration choice after implementation; default to no new setting because scheduling, stable windows, coalescing, changed-range queries, and provisional interpolation are correctness/performance internals.
    - Performance: Internal budgets remain compiled/validated and cannot be raised dynamically per keystroke; no hidden debounce, word-boundary, parse-chunk, interpolation, or client-parser option is introduced.
    - Code Quality: Any genuine behavior-changing setting is a documented Clay JS API through `~/.config/clay/init.js`, with complete schema/registry/tests; otherwise document the fixed defaults and rejected hidden keys.
    - Security: Configuration grants no parser code execution, filesystem, network, shell, extension loading, AI mutation, raw-op, package, or workspace authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md` configuration requirement.
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `docs/reference/clay-js-api/configuration.md`
    - Options Considered:
      - User-tunable debounce/window/chunk/interpolation controls: rejected absent measured need.
      - Fixed safe defaults with metrics and later evidence-based revisit. Chosen.
    - Chosen Approach:
      - Add a configuration-review section recording no new public setting unless implementation uncovers a user-facing policy choice.
    - API Notes and Examples:
      ```text
      No planned clay.configuration.* additions.
      Rejected hidden keys: syntaxDebounceMs, syntaxWordBoundaryOnly,
      syntaxParseWindowBytes, syntaxDecorationChunkBytes, clientSyntaxParser.
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: review outcome and fixed internal behavior.
      - `tests/clay_js_api_inventory.rs`: no-hidden-setting and empty-custom-property assertions.
      - API docs/inventory/facades only if a setting is explicitly justified and documented.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - No undocumented syntax-latency configuration IDs/custom properties exist.
    - Existing syntax engine preference remains the only relevant user engine-selection surface.
    - Configuration cannot alter authority or enter paint/text hot paths.
  - Completion Evidence:
    - Audited Plan 056 scheduler, stable-window, query/fan-out, replacement, and interpolation behavior. It introduces no user policy choice: stable-window/query/fallback rules, coalescing, compiled payload/cache limits, 128-byte output splitting, and provisional interpolation remain Clay-owned correctness and latency invariants. No `clay:configuration` facade, op, inventory row, custom property, registry entry, or hidden setting was added.
    - Added the `Plan 056 low-latency syntax configuration review` to `docs/reference/clay-js-api/configuration.md`. It keeps `clay.syntax.setSyntaxEnginePreference(target, tier)` as the only relevant documented engine-selection surface, states parser-registration fields are load-time package metadata rather than `init.js` tuning, fixes the no-config default, and rejects debounce, word-boundary, parse-window, chunk-size, interpolation, and client-parser knobs.
    - Added a deterministic inventory guard verifying the configuration review and implementation wiki, exact `target`/`tier` syntax-preference properties, and absence of all rejected Plan 056 names from configuration facade/ops, API inventory, and generated registry. The wiki records that configuration cannot run on keypress, edit, parse, publication, paint, layout, or scroll paths and grants no parser callback or external authority.
    - Verified `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --test clay_js_api_inventory` (60 passed), `cargo test --test package_loading_docs` (52 passed), `cargo test --test clay_js_doc_registry` (34 passed), `cargo test --test primitives_docs` (125 passed), and `git diff --check`.

- [x] Run end-to-end Linux verification and record measured results
  - Acceptance Criteria:
    - Functional: Real `cargo run` editing of Rust, TypeScript/TSX, JavaScript, and Markdown demonstrates immediate text, continuous comment/string/prose styling, whole-token authoritative transitions, current-version stale rejection, syntax-plus-semantic layering, scrolling, save, undo/redo, and document switching without regressions.
    - Performance: Deterministic tests prove one parse per version/window and bounded fan-out; benchmark results record parse/query/publish work and accepted-edit-to-current-decoration latency; Linux `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and benchmark compilation pass.
    - Code Quality: No TODO-only paths, dead compatibility scheduler, duplicate parse loop, or language-specific hot-path branch remains; docs and decision references match final implementation.
    - Security: Fuzz/negative cases for malformed ranges, stale versions, oversized payloads, wrong provenance, and runtime-generation replacement fail closed; live logs/metrics contain no text or absolute paths.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/{performance,launch-and-gui-smoke}.md`
      - Linux-primary validation instructions in `AGENTS.md`.
    - Options Considered:
      - Unit-only verification: insufficient for visible latency issue.
      - Wall-clock-only CI gate: unstable before baseline.
      - Structural assertions + benchmarks + live Linux smoke. Chosen.
    - Chosen Approach:
      - Run focused suites first, then all Linux gates, compile benchmarks, and execute a manual real-config smoke with recorded before/after work counts and latency distribution.
    - API Notes and Examples:
      ```bash
      cargo fmt --check
      cargo check --all-targets
      cargo clippy --all-targets -- -D warnings
      cargo test --all-targets
      cargo bench --no-run
      ```
    - Files to Create/Edit:
      - `docs/development/{performance,launch-and-gui-smoke}.md`: final commands, results, and manual syntax-latency checklist.
      - `plans/056-Low-Latency-Incremental-Syntax-Decoration.md`: completion evidence, actual compromises, and further actions.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Focused: `syntax_grammar`, `parse_coordinator`, `decoration_transport`, `performance_protocol`, `editor_performance_invariants`, `language_intelligence`.
    - Full Linux gates listed above.
    - Manual rapid typing in keywords, identifiers, comments, strings, Markdown prose/code, and large scrolled files.
  - Completion Evidence:
    - Linux verification host: kernel `7.1.3-43.stable`, `x86_64`, Rust/Cargo `1.96.1`. Passed `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `cargo bench --no-run`; the full test run includes focused syntax/transport/performance/editor/language-intelligence suites plus malformed edit/range, stale version, oversized payload, provenance, and generation-replacement fail-closed coverage.
    - Ran the five-language `first_party_incremental_edit` Criterion benchmark (10 samples, 1 s warm-up, 2 s measurement). Local advisory medians: Rust 168.91 µs, TypeScript 356.50 µs, TSX 125.02 µs, JavaScript 124.91 µs, Markdown 217.54 µs. The performance reference records intervals/throughput and explains that deterministic metrics prove one logical accepted-edit item, one current-version parser task/window, changed query work, bounded chunk publication, cancellation, and one current-version `syntax.edit_to_publish` sample without retaining source text or paths.
    - Launched actual `cargo run -- smoke-gui --config-fixture language-packages --profile-perf` on the GNOME Wayland/X11 host. The managed server started, client connected, runtime fixture installed, and a native window was created; the bounded smoke session was intentionally stopped and left no managed smoke server. Existing deterministic five-language edit/scroll/token/layering tests provide repeatable coverage for interactions not driven by GUI automation; the launch doc retains the manual rapid-edit, scroll, save, undo/redo, and multi-document checklist.
    - Updated `docs/development/performance.md` and `docs/development/launch-and-gui-smoke.md` with command, host, benchmark, metric, smoke, and manual-matrix evidence. Added `manual_smoke_docs::plan056_linux_syntax_smoke_and_measurements_are_recorded` to lock both records.
    - Verified after documentation/test updates: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --test manual_smoke_docs` (18 passed), `cargo test --test performance_budgets` (16 passed), and `git diff --check`.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, covering accepted-edit scheduling, stable windows/exact edits, one-parse changed-range querying, decoration fan-out, provisional interpolation, token transitions, layering, metrics, and tests.
    - Performance: Wiki documents work-count and latency behavior, large-file budgets, cancellation/coalescing, and why output chunks no longer multiply parse work; updates add no runtime work.
    - Code Quality: Pages explain source modules, data flow, invariants, tradeoffs, extension guidance, test paths/commands, and link from `docs/wiki/index.md`; obsolete 256-byte sibling parse-job descriptions are removed.
    - Security: Wiki records server parser/package provenance authority, inert client interpolation, stale rejection, bounded validation, and authorities not introduced without exposing source or sensitive data.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`
      - Primitive review and all final code/tests from this plan.
    - Options Considered:
      - Update each implementation task: noisy and likely stale.
      - Update once after final verification. Chosen.
    - Chosen Approach:
      - Update existing educational pages and the master index once final behavior/tests are known; keep public API usage in reference docs.
    - API Notes and Examples:
      ```text
      docs/wiki/modules/parse-coordinator.md
      docs/wiki/modules/parse-task-lifecycle.md
      docs/wiki/modules/syntax-grammar-registry.md
      docs/wiki/modules/decoration-transport.md
      docs/wiki/modules/masonry-editor.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: verify navigation.
      - `docs/wiki/modules/{parse-coordinator,parse-task-lifecycle,syntax-grammar-registry,decoration-transport,masonry-editor}.md`: final implementation knowledge.
      - `tests/primitives_docs.rs`: deterministic wiki/reference coverage where practical.
    - References:
      - `.agents/skills/create-plan/references/wiki-task.md`
      - `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`
  - Test Cases to Write:
    - Manual wiki review: all relevant pages linked and current.
    - Deterministic docs test rejects obsolete repeated-parse description and requires final one-parse/interpolation architecture.
    - `cargo test --test primitives_docs`
  - Completion Evidence:
    - Verified the indexed `Low-Latency Incremental Syntax Decoration Primitive Review` now identifies itself as completed implementation knowledge and records exact accepted edits/stable windows, one parse/query capture pass, 128-byte fan-out, atomic current-version replacement, provisional interpolation, token/layer behavior, source-safe metrics, authority boundaries, and test paths.
    - Updated stale lifecycle/protocol descriptions: `parse-task-lifecycle.md` now documents `ParseInputEdit`, `ParseWindowSnapshot.window_id`, start-gated native/runtime tasks, `decoration_updates: Vec<DecorationSet>`, one affected-envelope query, and safe provisional fallback. `parse-coordinator.md` and `syntax-grammar-registry.md` now consistently describe validated multi-set batches rather than the superseded singular `decoration_update` shape. `decoration-transport.md` and `masonry-editor.md` were verified current for package/layer-keyed authoritative replacement and bounded client interpolation.
    - Updated `docs/wiki/index.md` navigation to label the review as implemented flow/verification rather than pre-implementation material. Public use remains linked to authoritative `docs/reference/` pages; no Clay JS API was added.
    - Added `primitives_docs::plan056_final_wiki_records_one_parse_authoritative_decoration_flow`, requiring the index and all five implementation pages to retain exact-edit, one-parse/fan-out, multi-set, provisional replacement, and interpolation knowledge while rejecting the old singular update shape.
    - Verified `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --test primitives_docs` (126 passed), `cargo test --test manual_smoke_docs` (18 passed), and `git diff --check`.

## Compromises Made

- Disjoint changed ranges are still queried as their smallest contiguous affected envelope; fan-out bounds publication, but separate query passes are deferred unless measurements show envelope work matters inside the 4 KiB window.
- Server-side parsing remains asynchronous; provisional spans may briefly show stale style after structural delimiters before authoritative correction.
- Client-side Tree-sitter is intentionally deferred. Revisit only if measured one-parse server latency remains visibly insufficient.
- Wall-clock latency stays advisory until results are stable on a consistent CI runner; deterministic parser/query/chunk work-count assertions are blocking immediately.

## Further Actions

- **Low — establish a stable Linux CI benchmark host**: collect comparable `first_party_incremental_edit` distributions before promoting advisory wall-clock numbers to a regression threshold.
- **Low — measure disjoint-range envelopes**: split changed-range queries only if profiling shows the current smallest-contiguous-envelope query matters within the bounded 4 KiB window.
- **Deferred — client parser**: consider only if optimized server-side one-parse latency remains visibly insufficient after the prior two measurements; it requires a separate authority/memory decision.
