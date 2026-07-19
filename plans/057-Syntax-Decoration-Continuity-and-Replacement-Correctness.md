# Syntax Decoration Continuity and Replacement Correctness

## Objectives

- Fix the reported per-letter base-color flash while appending ordinary word characters to an already decorated syntax token.
- Prevent newline and local code edits from erasing unrelated syntax decoration in the same transport chunk.
- Make changed-range query coverage and authoritative replacement coverage identical, including UTF-8-safe chunk boundaries.
- Validate visual continuity through the real local-edit, acknowledgement, parse, transport, and client-application sequence rather than testing each stage in isolation.

## Expected Outcome

- Appending letters/digits/underscore to an already classified syntax word inherits that word's provisional style immediately; whitespace, newline, and punctuation end the inherited word run and let current server syntax correct it.
- Pressing Enter after comments or code does not turn previously decorated text in the affected 128-byte chunk white.
- Every authoritative `DecorationSet` contains complete syntax state for exactly the range it replaces; empty sets clear only a fully queried replacement range.
- Existing server-authoritative parsing, one parse per version/window, stale-result rejection, bounded payload/cache budgets, semantic layering, and package-neutral grammar behavior remain intact.
- A newly created token with no prior span may transition once when first classified; repeated per-letter flashing after classification is eliminated without adding a client parser.

## Evaluation

- `src/editor/surface.rs::interpolate_decoration_span` deliberately treats insertion at the end of a narrow syntax span as outside that span. The new byte therefore paints with the base brush until an authoritative update arrives. `optimistic_narrow_span_does_not_inherit_edge_insertions` currently locks this reported flicker into the test suite.
- `src/server/syntax.rs::decorations_for_window` queries only the changed envelope, but `decoration_sets_for_range` aligns publication down to a whole 128-byte chunk. The resulting set can omit unchanged captures inside the range its key says it authoritatively replaces.
- `src/editor/surface.rs::EditorDecorationState::apply_set` correctly removes an overlapping provisional package/layer chunk before installing authoritative data. Combined with the incomplete server set above, this deletes unrelated decoration from the same chunk. Short files often occupy one chunk, explaining why Enter can make all visible syntax white.
- Existing tests separately verify parser captures, synthetic interpolation, and authoritative clearing. They do not compose initial real grammar output, optimistic local edit, edit acknowledgement, and each streamed authoritative set while asserting that previously decorated bytes never regress to the base brush.
- Tree-sitter documents that `QueryCursor::set_byte_range` returns matches intersecting the configured range. Therefore querying the complete range Clay will replace is sufficient to reconstruct that replacement chunk without querying the whole document. Local Tree-sitter 0.25.10 documentation confirms `Tree::changed_ranges` compares the edited old tree with the new tree after incremental parsing.
- Word/newline boundaries should control provisional visual inheritance, not parser scheduling. Parsing every accepted edit preserves structural correctness for quotes, comment delimiters, brackets, and operators; the client can still avoid per-letter flashes by extending an already known same-word style synchronously.

## Tasks

- [x] Reproduce visual regressions through the complete decoration state machine
  - Acceptance Criteria:
    - Functional: Add deterministic tests that apply real first-party grammar output to `EditorSurface`, perform an optimistic local edit, acknowledge the new version, then apply every authoritative output set in transport order while inspecting visible style ranges after every transition.
    - Performance: Tests use bounded first-party windows and assert no extra parser job is introduced; no timing-only pass condition is used.
    - Code Quality: Tests fail for both identified defects before implementation: token-end word insertion paints the inserted byte with the base style, and newline/changed-range publication removes unrelated syntax from the same replacement chunk.
    - Security: Fixtures contain synthetic source only and exercise existing validated inert decoration paths; no new package/runtime authority is introduced.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`: approved one-parse/provisional-decoration architecture and its intended visual outcome.
      - `docs/wiki/modules/{decoration-transport,masonry-editor,parse-coordinator,syntax-grammar-registry}.md`: current edit, parse, transport, replacement, and paint flow.
      - `.agents/skills/project-patterns/references/{mode-primitive-first,protocol-and-performance,authority-boundaries,maintenance-validation}.md`: generic primitive, hot-path, authority, and regression-test constraints.
      - Tree-sitter 0.25.10 local Rust source: `Tree::changed_ranges` and `QueryCursor::set_byte_range`.
      - Context7 `/tree-sitter/tree-sitter`: byte-range queries return intersecting matches.
    - Options Considered:
      - Keep unit tests per stage: rejected because they already passed while the composed GUI behavior failed.
      - Add pixel/GPU snapshots: too broad and not deterministic for byte-level style continuity.
      - Add one composed render-state test using existing test observability. Chosen.
    - Chosen Approach:
      - Reuse `EditorSurface::visible_decoration_paint_ranges_for_test`, real `TreeSitterSyntaxHandler` output, and existing local-edit/version APIs. Assert stable decorated byte ranges after local edit, ack, and each streamed set—not only final state.
    - API Notes and Examples:
      ```text
      initial grammar sets -> EditorSurface
      local Insert/Newline -> inspect provisional paint ranges
      EditAck(version + 1)  -> inspect retained paint ranges
      for set in authoritative.decoration_updates:
          apply set -> inspect that unrelated decorated bytes never become base-colored
      ```
    - Files to Create/Edit:
      - `tests/syntax_grammar.rs`: real grammar-to-editor continuity regressions using existing `EditorSurface` paint-range observability; no new test-only production hook was needed.
    - References:
      - `src/editor/surface.rs::{EditorDecorationState::apply_edit,EditorDecorationState::apply_set}`
      - `src/server/syntax.rs::{decorations_for_window,decoration_sets_for_range}`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
  - Completion Evidence:
    - Added `plan057_function_suffix_stays_decorated_through_local_ack_and_authoritative_states`, composing first-party Rust parsing, initial editor application, token-end local insertion, version acknowledgement, incremental parse, and each authoritative member. Its explicit ignored run fails at the expected stages: the grown function suffix is base-colored after local edit/ack, then the unchanged `fn` keyword is base-colored after authoritative replacement.
    - Added `plan057_newline_keeps_unrelated_short_file_syntax_through_every_state`, composing the same path for Enter inside a line comment. Its explicit ignored run proves the current authoritative member removes the untouched `fn`, function declaration, and opening punctuation while retaining the changed comment capture.
    - Both red regressions are `#[ignore]` until their respective implementation tasks remove the defects; `cargo test --test syntax_grammar <name> -- --ignored --exact --nocapture` exits 101 for each. Normal validation remains green: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --test syntax_grammar` (58 passed, 2 ignored).
    - The helpers assert one initial full parse and one incremental parse through `syntax_tree_delta`; no parser job, timing gate, language-specific production branch, or new test-only production API was added.
  - Test Cases Written:
    - Rust function suffix growth checks the already classified token after optimistic edit, acknowledgement, and every authoritative member.
    - Rust short-file newline checks an earlier keyword, function declaration, punctuation, and comment prefix after every transition.
    - One changed range inside a multi-capture short-file chunk proves untouched captures are omitted by current authoritative replacement.
    - Edit acknowledgement is explicitly checked before server output and preserves all pre-existing provisional ranges while retaining the known suffix gap.

- [x] Make authoritative syntax replacement chunks complete and UTF-8 safe
  - Acceptance Criteria:
    - Functional: Before capture extraction, expand normalized changed/invalidated coverage to the exact UTF-8-safe output chunk ranges that will be replaced; emit every capture intersecting each replacement range and never advertise a wider authoritative viewport than was fully queried.
    - Performance: Keep one Tree-sitter parse per accepted version/window and one bounded query over the replacement envelope; query expansion is limited to touched 128-byte chunks inside the existing 4 KiB window and does not multiply work by output member count.
    - Code Quality: Use one shared chunk-range calculation for query coverage and `DecorationSet` construction so the two cannot drift; remove capture-envelope logic that widens replacement beyond complete query coverage.
    - Security: Existing version, package/layer provenance, permission, payload, cache, timeout, and atomic coordinator validation remain fail-closed.
  - Approach:
    - Documentation Reviewed:
      - Tree-sitter query API: `set_byte_range` returns matches intersecting the byte range.
      - Tree-sitter 0.25.10 local Rust source for `QueryCursor::set_byte_range` and `Tree::changed_ranges`.
      - `src/server/syntax.rs`: changed-range normalization, capture extraction, 128-byte fan-out, and validation.
      - `src/server/decorations.rs`: `SyntaxChunkCache` and payload/cache limits.
    - Options Considered:
      - Merge partial authoritative sets with stale client spans: rejected because absence must remain authoritative and client merge rules would duplicate parser semantics.
      - Query the full 4 KiB window and republish all chunks on every keypress: correct but unnecessary transport/render churn.
      - Query complete touched replacement chunks and publish only those chunks. Chosen.
      - Send the whole internal batch atomically to the client: insufficient alone because an incomplete replacement batch still deletes omitted captures.
    - Chosen Approach:
      - Convert changed ranges to shared UTF-8-safe chunk ranges first, query their smallest bounded envelope, clip captures only at those exact chunk boundaries, and construct sets from the same range list. Empty members remain valid only because their full range was queried.
    - API Notes and Examples:
      ```rust
      let chunks = replacement_chunks(changed_ranges, window_text, 128);
      cursor.set_byte_range(chunks.first().start..chunks.last().end);
      let sets = complete_sets_for_chunks(chunks, captures);
      ```
    - Files Created/Edited:
      - `src/server/syntax.rs`: shared UTF-8-safe replacement-range partitioning, one-envelope query coverage, exact output clipping, metrics assertions, and unit tests.
      - `tests/syntax_grammar.rs`: complete-chunk, real-editor newline, empty replacement, UTF-8 boundary, and broad-capture regressions.
      - `tests/performance_protocol.rs`: unchanged; its existing first-party per-member payload-budget test covers the retained transport ceiling, while invocation/member-count coverage belongs beside the private handler recorder in `src/server/syntax.rs`.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `docs/reference/primitives/{parse-update-strategy,rendering-strategy}.md`
  - Completion Evidence:
    - `replacement_ranges` now partitions the current bounded parse window on nominal 128-byte boundaries moved forward to UTF-8 character boundaries, then selects only chunks intersecting normalized Tree-sitter/explicit invalidations. The same selected range list defines the single `QueryCursor::set_byte_range` envelope and every published `DecorationSet`, removing capture-envelope widening.
    - Capture nodes intersecting the one bounded query are clipped into each exact selected replacement range. Empty members are emitted only for selected fully queried chunks; disjoint selected chunks may share one bounded query envelope but do not publish untouched intervening chunks.
    - The composed newline regression is enabled and passes: the shortened comment and unchanged keyword, declaration, and punctuation remain decorated after every authoritative member. The suffix regression's authoritative stage also passes; it remains ignored only for the separate provisional same-word task.
    - Added deterministic coverage for unchanged captures in a touched short-file chunk, empty authoritative clearing, a multibyte scalar crossing nominal byte 128, a broad line-comment capture filling one touched middle chunk, one touched chunk without adjacent publication, bounded query bytes, and one parse/query invocation despite multi-member output.
    - Full Linux validation passes: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `git diff --check`. `syntax_grammar`: 62 passed, 1 ignored for the next task; parse coordinator 29, decoration transport 15, performance protocol 19, and editor performance invariants 22 passed.
  - Test Cases Written or Verified:
    - Partial keyword completion republishes unchanged `fn` plus the completed keyword in one authoritative chunk.
    - Newline inside a line comment republishes the shortened comment plus unrelated syntax in the same chunk through the real editor state machine.
    - Empty authoritative chunk is emitted for a complete queried replacement range.
    - A multibyte scalar straddling nominal byte 128 moves the shared boundary to byte 129; all set/span boundaries remain valid character boundaries.
    - A changed broad line comment crossing multiple chunks completely fills only its touched 128-byte replacement chunk.
    - Multi-member output records one parse invocation and one query invocation; existing dense and first-party payload tests keep every member within budget.

- [x] Extend provisional syntax by same-word boundaries instead of every server round trip
  - Acceptance Criteria:
    - Functional: Insertion at the end of an existing narrow syntax span inherits that span only when all inserted characters continue the same word (`Unicode alphanumeric` or `_`); broad syntax retains current edge behavior; whitespace, newline, punctuation, structural edits, and non-syntax layers do not inherit narrow-token style.
    - Performance: The decision is synchronous, allocation-free beyond existing edit text, bounded to retained near-viewport spans, and invokes no parser, IPC, package JavaScript, or full-document scan.
    - Code Quality: Reuse the existing word-character predicate and generic `TokenType`/`DecorationKind` data; do not add language names, keyword lists, delimiter tables, debounce timers, or a second tokenizer.
    - Security: Only already validated inert syntax style is interpolated; server current-version output remains authoritative and resync/runtime replacement still clears provisional state.
  - Approach:
    - Documentation Reviewed:
      - `src/editor/surface.rs::{is_completion_word_character,interpolate_decoration_span,edit_extent}`.
      - `docs/wiki/modules/{decoration-transport,masonry-editor}.md`.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: local paint stays wait-free; latest syntax edit remains parse-eligible.
    - Options Considered:
      - Parse only on whitespace/newline: rejected because quotes, comments, brackets, operators, and punctuation can change syntax immediately.
      - Delay all authoritative output until a boundary: adds buffering/stale-state complexity and delays structural correction.
      - Add a client Tree-sitter parser: rejected until the corrected bounded server path is measured and still visibly insufficient.
      - Extend an existing style through same-word suffix input and stop at boundaries. Chosen.
    - Chosen Approach:
      - Pass inserted text—not only byte length—into interpolation. At a syntax span's right edge, inherit only a non-empty all-word-character insertion. Keep broad capture inheritance and existing safe invalidation for replacement/delete and non-syntax overlaps.
    - API Notes and Examples:
      ```text
      Function("ma") + "i" + "n" -> provisional Function("main")
      Keyword("let") + "x"        -> provisional Keyword("letx"), corrected by server
      Keyword("let") + " "        -> space remains base style; word style ends
      Comment("// note") + "\n"   -> no narrow-word inheritance; authoritative chunk stays complete
      ```
    - Files Created/Edited:
      - `src/editor/surface.rs`: `edit_extent` now borrows inserted text, and the existing bounded interpolation pass extends narrow syntax only for non-empty all-word suffixes; focused unit tests cover token families, boundaries, Unicode, and non-syntax layers.
      - `tests/decoration_transport.rs`: added local edit/ack/authoritative correction integration coverage.
      - `tests/syntax_grammar.rs`: enabled the composed first-party Rust suffix continuity regression from task 1.
    - References:
      - `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`
      - `decision-logs/2026-07-19-1912-syntax-decoration-continuity-and-complete-authoritative-replacement.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Completion Evidence:
    - `EditorDecorationState::apply_edit` passes the already-owned inserted text and its checked byte length into `interpolate_decoration_span`; no text copy, parser, IPC, JavaScript, full-document scan, new tokenizer, language branch, or configuration was added.
    - Existing narrow syntax at its right edge now extends only when inserted text is non-empty and every Unicode scalar satisfies the shared `is_completion_word_character` predicate (`is_alphanumeric()` or `_`). Broad syntax keeps its prior edge behavior; mixed or boundary text does not partially inherit.
    - The previously ignored real Rust grammar/editor regression is enabled and passes through optimistic edit, acknowledgement, incremental parse, and every authoritative member without exposing the grown function suffix to the base brush.
    - Authoritative correction remains final: integration coverage removes a provisionally inherited Function span while preserving and shifting an unrelated Keyword span. Semantic, diagnostic, and search layers never inherit same-word authority.
    - Full Linux validation passes: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `git diff --check`. Library: 890 passed; `syntax_grammar`: 63 passed with no ignored regressions; `decoration_transport`: 16 passed; editor performance invariants: 22 passed.
  - Test Cases Written:
    - Function, Type, Variable, Keyword, and Number spans immediately inherit `x2_` suffix geometry.
    - Space, tab, newline, quote, bracket, slash, and operator input leave narrow-token end geometry unchanged.
    - Unicode `é` inherits with its two-byte UTF-8 geometry preserved.
    - Current-version authoritative syntax removes an inherited Function span without clearing the unrelated Keyword span.
    - Semantic, Diagnostic, and SearchMatch spans do not inherit an appended word character.
    - Real first-party Rust Function decoration remains continuous after local suffix insertion, acknowledgement, and each authoritative update.

- [x] Verify continuity across first-party languages and manual Linux editing
  - Acceptance Criteria:
    - Functional: Rust, TypeScript, TSX, JavaScript, and Markdown pass composed continuity cases for word growth, comment/string/prose growth, newline, punctuation, deletion, and authoritative correction; manual GUI testing confirms no reported all-white newline regression or per-letter suffix flash.
    - Performance: Record parse count, queried bytes, emitted members, and edit-to-publication latency for the corrected cases; retain one parse per version/window and existing payload/cache limits. Wall-clock results remain advisory until a stable CI host exists.
    - Code Quality: Use real package queries/style maps and generic editor/transport paths; no first-party language branch is added to scheduling, replacement, interpolation, or paint.
    - Security: Package provenance, grammar-tier selection, server parser ownership, and no package/client JavaScript hot-path rules remain unchanged.
  - Approach:
    - Documentation Reviewed:
      - `packages/{rust,typescript,javascript,markdown}/queries/highlights.scm`.
      - `tests/fixtures/syntax/*`.
      - `docs/development/{performance,launch-and-gui-smoke}.md`.
    - Options Considered:
      - Trust deterministic tests only: rejected because Plan 056 passed while manual visual behavior remained broken.
      - Manual validation only: rejected because visual regressions need repeatable state assertions.
      - Deterministic transition tests plus explicit manual smoke. Chosen.
    - Chosen Approach:
      - Run the same edit-state matrix over real grammar fixtures, then perform a bounded Linux `smoke-gui --config-fixture language-packages --profile-perf` session using the user's reported typing sequences.
    - API Notes and Examples:
      ```bash
      cargo test --test syntax_grammar
      cargo test --test decoration_transport
      cargo run -- smoke-gui --config-fixture language-packages --profile-perf
      ```
    - Files Created/Edited:
      - `tests/syntax_grammar.rs`: added one real grammar/editor transition matrix over 25 word/comment/string/newline/prose/punctuation/deletion cases plus four authoritative keyword-correction cases.
      - `tests/decoration_transport.rs`: added rapid-version stale-authority/provisional-geometry coverage.
      - `src/server/syntax.rs`: added private deterministic work-count coverage for all five native descriptors.
      - `src/server/parse_coordinator.rs`: made the existing one-sample `syntax.edit_to_publish` test print its advisory local duration.
      - `tests/manual_smoke_docs.rs`: added deterministic Plan 057 smoke/performance documentation guard.
      - `docs/development/launch-and-gui-smoke.md`: recorded exact manual visual checkpoints and repeatable transition tests.
      - `docs/development/performance.md`: recorded corrected parser/query/member counts, publication instrumentation, and Criterion results.
    - References:
      - `.agents/skills/project-patterns/references/{maintenance-validation,protocol-and-performance}.md`
      - `roadmap.md`: manual GUI validation and advisory benchmark policy.
  - Completion Evidence:
    - `plan057_first_party_languages_keep_continuity_across_edit_boundaries` composes initial real grammar output, local edit, acknowledgement, incremental parse, each authoritative member, and paint-range inspection for Rust, TypeScript, TSX, JavaScript, and Markdown. Cases cover declaration and string growth, comment/prose newline, Markdown paragraph/code-span growth, punctuation, and deletion without production language branches.
    - `plan057_authoritative_queries_correct_inherited_code_keywords` proves Rust `letx` and TypeScript/TSX/JavaScript `constx` provisional keyword geometry is removed authoritatively while unrelated function decoration survives. `rapid_local_versions_reject_stale_authority_without_losing_provisional_geometry` proves version 2 authority cannot erase version 3 inherited geometry.
    - Native Wayland smoke boot passed. An agent-observable X11 run opened actual workspace Rust/TypeScript files through the same managed server and package fixture: Rust vocabulary layers rendered; TypeScript `greet` + `x` remained fully function-colored, and Enter after the declaration left earlier and later syntax decorated. Neither reported suffix flash nor all-white newline state appeared. Temporary framebuffer captures were not committed.
    - Deterministic work measurements per suffix edit are one parser call, one query range, and one member for every language; query bytes are Rust 20, TypeScript 26, TSX 26, JavaScript 26, Markdown 17. The existing coordinator instrumentation emitted one `syntax.edit_to_publish` sample (local unit-plumbing sample 140.268 µs).
    - Criterion estimates were Rust 167.95 µs, TypeScript 361.10 µs, TSX 125.92 µs, JavaScript 122.93 µs, and Markdown 199.86 µs. Criterion reported no statistically significant change; values remain machine-local/advisory.
    - Full Linux validation passes: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, `cargo bench --no-run`, and `git diff --check`. `syntax_grammar`: 65 passed; `decoration_transport`: 17 passed; `manual_smoke_docs`: 19 passed; no ignored Plan 057 regressions; managed smoke processes were cleaned up.
  - Test Cases Written or Verified:
    - Function declaration suffixes remain decorated through local edit, acknowledgement, and every authoritative member in Rust, TypeScript, TSX, and JavaScript.
    - Enter inside code comments and Markdown prose retains unaffected visible syntax; code-string and Markdown paragraph/code-span growth remain continuous.
    - Inserted punctuation becomes authoritative without clearing existing declarations; backspace shrinks code declarations and Markdown code spans without base-color gaps.
    - Real grammar output corrects inherited keyword suffixes without collateral clearing.
    - Rapid versions reject stale authority while preserving current provisional geometry.
    - UTF-8 character boundaries and the shared 128-byte replacement boundary remain locked by the existing Plan 057 boundary regressions.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Audit changed Rust functions/types and confirm replacement coverage and interpolation remain internal behavior; expose a Clay JS API only if implementation introduces a genuine caller-controlled capability.
    - Performance: No API permits per-keypress JavaScript callbacks, parse scheduling control, replacement-chunk sizing, or paint-path execution.
    - Code Quality: Any necessary public capability follows facade/op/inventory/Markdown/generated-registry conventions; expected result is no new API.
    - Security: No API bypasses package provenance, grammar validation, server authority, document versions, or decoration budgets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: mandatory Clay JS API audit.
      - `.agents/skills/project-patterns/references/{clay-js-api-boundary,clay-js-api-naming,clay-js-api-schema,documentation-as-code,doc-registry-tests}.md`.
      - Existing `clay:syntax`, `clay:parse`, and `clay:decorations` API docs/inventory.
    - Options Considered:
      - Expose word-boundary/chunk/query tuning: rejected as correctness internals.
      - Keep correction behind existing grammar/parse/decoration facades. Expected/chosen.
    - Chosen Approach:
      - Run visibility/inventory/docs-registry audits after implementation; add no speculative API.
    - API Notes and Examples:
      ```text
      Expected public surface change: none.
      Existing surfaces remain serverRegisterSyntaxGrammar, serverRegisterParseHandler,
      and serverPublishDecorations.
      ```
    - Files Created/Edited:
      - `tests/clay_js_api_inventory.rs`: added `plan057_syntax_continuity_internals_reuse_existing_clay_js_apis` test verifying existing three public surfaces remain the only ones, Plan 057 internals (`replacement_ranges`, `decoration_sets_for_ranges`, `is_completion_word_character`, `same_word_suffix`, `edit_extent`) do not appear in facade exports, wiki audit sections record Plan 057 findings, configuration docs record Plan 057 review with rejected hidden keys, and hidden Plan 057 configuration names are absent from facades/ops/inventory/registry.
      - `docs/reference/clay-js-api/configuration.md`: added Plan 057 configuration review section documenting no new API, `setSyntaxEnginePreference` as sole user choice, rejected hidden keys (`syntaxSameWordBoundary`, `syntaxReplacementChunkGrid`, `syntaxWordInheritance`, `syntaxChunkQueryCoverage`, `syntaxCompleteReplacement`, `syntaxUtf8ChunkGrid`), and hot-path exclusion.
      - `docs/wiki/modules/low-latency-incremental-syntax-decoration-primitive-review.md`: updated Clay JS API Audit section with Plan 057 paragraph (no caller-controlled capability, `replacement_ranges`/same-word inheritance are compiled internals, no facade exports) and Configuration Audit section with Plan 057 paragraph (no configuration surface, compiled correctness invariants, rejected hidden key names).
    - References:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
  - Completion Evidence:
    - `replacement_ranges`, `decoration_sets_for_ranges`, `is_completion_word_character`, `interpolate_decoration_span`, and `edit_extent` are all private (`fn`, no `pub`) in `src/server/syntax.rs` and `src/editor/surface.rs`.
    - `plan057_syntax_continuity_internals_reuse_existing_clay_js_apis` test verifies: existing three public surfaces retained in inventory; 8 Plan 057 internal names rejected from facade exports; wiki audit sections contain Plan 057 findings; configuration docs contain Plan 057 review section and 9 required phrases; 13 hidden Plan 057 configuration names rejected from all implementation surfaces.
    - Full Linux validation passes: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`. `clay_js_api_inventory`: 61 passed; `rust_visibility_api_mapping`: 17 passed; `clay_js_doc_registry`: 34 passed.
  - Test Cases Written:
    - Plan 057 internals (`replacement_ranges`, `decoration_sets_for_ranges`, `is_completion_word_character`, `same_word_suffix`, `edit_extent`, `setSyntaxSameWordBoundary`, `setSyntaxReplacementChunkGrid`, `setSyntaxWordInheritance`) do not appear in parse/syntax/decorations facade exports.
    - Hidden configuration keys (`syntaxSameWordBoundary`, `syntaxReplacementChunkGrid`, `syntaxWordInheritance`, `syntaxCompletionWordCharacter`, `syntaxChunkQueryCoverage`, `syntaxProvisionalInheritance`, `syntaxCompleteReplacement`, `syntaxUtf8ChunkGrid`, and `set*` equivalents) are absent from facades, ops, inventory, and registry.
    - Wiki and configuration docs record Plan 057 audit findings.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Confirm same-word interpolation and complete authoritative replacement are fixed defaults, not optional user policy; add configuration only if implementation discovers a real user-facing choice.
    - Performance: No dynamic debounce, word-boundary, chunk-size, query-window, or client-parser setting enters typing/paint paths.
    - Code Quality: Any genuine setting must be a documented Clay JS API through `init.js`; expected result is no new setting.
    - Security: Configuration grants no parser code, filesystem, network, shell, package, raw-op, or client-JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: mandatory configuration audit.
      - `.agents/skills/project-patterns/references/configuration-system.md`.
      - `docs/reference/clay-js-api/configuration.md`.
    - Options Considered:
      - User-selectable word/newline debounce: rejected because it hides correctness defects and changes syntax freshness.
      - Compiled validated behavior with regression coverage. Chosen.
    - Chosen Approach:
      - Keep the fix non-configurable and verify hidden keys/facades are absent.
    - API Notes and Examples:
      ```text
      No planned clay.configuration.* additions.
      ```
    - Files Created/Edited:
      - `tests/clay_js_api_inventory.rs`: `plan057_syntax_continuity_internals_reuse_existing_clay_js_apis` rejects 13 Plan 057 hidden configuration names from facades, ops, inventory, and generated registry; `plan056_syntax_latency_configuration_stays_compiled_and_non_configurable` still rejects Plan 056 hidden names.
      - `docs/reference/clay-js-api/configuration.md`: added Plan 057 configuration review section documenting no new API, `setSyntaxEnginePreference` as sole user choice, rejected hidden keys (`syntaxSameWordBoundary`, `syntaxReplacementChunkGrid`, `syntaxWordInheritance`, `syntaxChunkQueryCoverage`, `syntaxCompleteReplacement`, `syntaxUtf8ChunkGrid`), and hot-path exclusion.
      - `docs/wiki/modules/low-latency-incremental-syntax-decoration-primitive-review.md`: Configuration Audit section updated with Plan 057 paragraph (no configuration surface, compiled correctness invariants, rejected hidden key names).
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Completion Evidence:
    - Same-word interpolation (`is_completion_word_character`, `same_word_suffix` flag) and complete authoritative replacement (`replacement_ranges`, UTF-8-safe shared chunk grid) are compiled invariants, not user-configurable policy. No debounce, word-boundary, chunk-size, query-window, or client-parser setting exists.
    - Configuration.md Plan 057 section states: "does **not** promote a new user-facing `clay:configuration` API", "`setSyntaxEnginePreference` remains the sole relevant user engine-selection surface", and lists 6 rejected hidden key families. Wiki Configuration Audit section states: "Plan 057 adds no `clay:configuration` surface" and "Complete authoritative replacement chunks … same-word narrow-syntax provisional inheritance … are compiled correctness invariants."
    - `plan057_syntax_continuity_internals_reuse_existing_clay_js_apis` rejects 13 hidden Plan 057 configuration names from all implementation surfaces; `plan056_syntax_latency_configuration_stays_compiled_and_non_configurable` still rejects Plan 056 names and verifies `setSyntaxEnginePreference` has only `target`/`tier` custom properties.
    - Full Linux validation passes: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`. `clay_js_api_inventory`: 61 passed (both Plan 056 and Plan 057 config tests).
  - Test Cases Written or Verified:
    - Plan 056 hidden keys (`syntaxDebounceMs`, `syntaxWordBoundaryOnly`, `syntaxParseWindowBytes`, `syntaxDecorationChunkBytes`, `syntaxInterpolation`, `clientSyntaxParser`, and `set*` equivalents) are rejected from configuration surfaces.
    - Plan 057 hidden keys (`syntaxSameWordBoundary`, `syntaxReplacementChunkGrid`, `syntaxWordInheritance`, `syntaxCompletionWordCharacter`, `syntaxChunkQueryCoverage`, `syntaxProvisionalInheritance`, `syntaxCompleteReplacement`, `syntaxUtf8ChunkGrid`, and `set*` equivalents) are rejected from configuration surfaces.
    - `setSyntaxEnginePreference` retains only `target` and `tier` custom properties; remains the sole relevant user choice.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document complete replacement coverage, query bounds, same-word interpolation, and measured continuity behavior.
    - Code Quality: Wiki pages explain corrected data flow, invariants, tradeoffs, source/test paths, and links from the master wiki index; stale claims that partial changed-range output can replace whole chunks are removed.
    - Security: Wiki pages preserve server parser authority, package provenance, inert client interpolation, and no-new-authority boundaries without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: project wiki workflow and quality bar.
      - `docs/wiki/index.md` and relevant parse/decoration/editor pages.
    - Options Considered:
      - Update after each task: noisy and likely to record intermediate behavior.
      - Update once after implementation and verification pass. Chosen.
    - Chosen Approach:
      - Update the existing low-latency review and module pages once, then add deterministic documentation coverage for replacement completeness and word-run interpolation.
    - API Notes and Examples:
      ```text
      changed ranges -> UTF-8-safe complete replacement chunks -> one bounded query
                     -> complete authoritative sets
      local same-word suffix -> provisional inherited style -> server correction
      ```
    - Files Created/Edited:
      - `docs/wiki/modules/low-latency-incremental-syntax-decoration-primitive-review.md`: added Plan 057 source references, full Plan 057 implementation section documenting both root-cause defects, same-word narrow inheritance fix (source paths, predicate, tests), complete authoritative replacement fix (source paths, `replacement_ranges` grid, tests), and verification results (continuity matrix, correction tests, X11 smoke, Criterion).
      - `docs/wiki/modules/syntax-grammar-registry.md`: updated steps 10-12 from affected-envelope query to complete replacement-chunk grid (`replacement_ranges`, query coverage == replacement coverage, same-grid construction), updated one-query paragraph, and added same-word narrow-syntax provisional inheritance to test coverage list.
      - `docs/wiki/modules/decoration-transport.md`: updated step 4 from affected-window division to shared replacement-chunk grid with complete authoritative capture state; updated step 9 from broad-only edge inheritance to narrow same-word + broad unconditional inheritance.
      - `docs/wiki/modules/parse-task-lifecycle.md`: replaced "one affected-envelope capture query" with "`replacement_ranges` converts them into a shared 128-byte UTF-8-safe replacement-chunk grid, and the handler queries the full envelope covering every touched chunk once — so query coverage and replacement coverage are identical."
      - `docs/wiki/modules/masonry-editor.md`: expanded interpolation description from broad-only edge inheritance to narrow same-word (Unicode alphanumeric/underscore) + broad unconditional.
      - `docs/reference/primitives/parse-update-strategy.md`: replaced "bounded visible/changed envelope" and "normalized UTF-8-safe affected envelope" with `replacement_ranges` grid language and query-coverage-equals-replacement-coverage invariant.
      - `docs/reference/primitives/rendering-strategy.md`: replaced "queried only over the UTF-8-safe visible intersection" with `replacement_ranges` grid language and capture clipping at exact chunk boundaries.
      - `tests/primitives_docs.rs`: added `plan057_final_wiki_records_complete_replacement_and_same_word_inheritance` test (127 passed, was 126) verifying wiki index link, review page Plan 057 implementation section (13 required phrases), lifecycle page complete replacement grid (3 required, 1 rejected stale phrase), syntax-grammar-registry page (4 required), decoration-transport page (3 required), masonry-editor page same-word mention, and primitives reference docs (6 required, 2 rejected stale phrases). Updated Plan 056 wiki test assertion from "stable 128-byte output ranges" to "128-byte replacement-chunk grid" to match corrected wiki text.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/create-plan/references/wiki-task.md`
  - Completion Evidence:
    - All six wiki modules updated: low-latency review (Plan 057 implementation section with both fixes, source paths, test names, verification results), syntax-grammar-registry (complete replacement grid, same-word inheritance), decoration-transport (shared grid, complete authoritative capture, same-word inheritance), parse-task-lifecycle (replacement_ranges grid, coverage identity), masonry-editor (same-word suffixes).
    - Two primitives reference docs updated: parse-update-strategy (replacement_ranges, query==replacement coverage), rendering-strategy (replacement_ranges, clipped at boundaries).
    - All stale "affected-envelope" / "visible intersection" / "partial-query whole-chunk" language removed; no wiki page describes querying less than full replacement range.
    - `plan057_final_wiki_records_complete_replacement_and_same_word_inheritance` test: 13 review-page phrases, 3 lifecycle phrases, 1 rejected lifecycle phrase, 4 syntax-registry phrases, 3 decoration-transport phrases, 1 masonry-editor phrase, 6 reference-doc phrases, 2 rejected reference-doc phrases.
    - Full Linux validation passes: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`. `primitives_docs`: 127 passed; all other suites green.
  - Test Cases Written or Verified:
    - Wiki index links relevant pages; review page records both root-cause defects, fixes, source paths, test names, and verification results.
    - Lifecycle, syntax-registry, decoration-transport, and masonry-editor pages describe complete replacement grid and same-word inheritance.
    - Primitives reference docs describe `replacement_ranges`, query==replacement coverage, and capture clipping at chunk boundaries.
    - No page describes superseded affected-envelope-only or visible-intersection-only query.

## Compromises Made

- **Same-word boundary only (no whole-word parse)**: narrow syntax inherits only when every inserted character is a Unicode word character or `_`. A full identifier/keyword re-parse is not attempted client-side — the narrow span geometry may drift slightly before server correction. Acceptable because same-word suffixes cover the most common user-visible case (appending identifier characters) without introducing a client-side parser or tokenizer.
- **Shared 128-byte replacement grid (not dynamic chunking)**: `replacement_ranges` uses the fixed `SYNTAX_DECORATION_CHUNK_BYTES` (128) as the shared grid step. This means a single-character edit touching a 128-byte boundary may produce two replacement chunks, and the full envelope covering all touched chunks is queried rather than only the exact changed bytes. Acceptable because query bytes remain well under the 4 KiB window (typically 128-256 bytes per edit), and the fixed grid ensures query coverage always equals replacement coverage.
- **One query pass covering all touched chunks**: the handler queries the full envelope over all replacement chunks in one `QueryCursor::set_byte_range` call rather than issuing separate queries per chunk. This keeps one parse/query invocation invariant but means captures from untouched chunks within the queried envelope are also returned and must be clipped. Acceptable because capture clipping at chunk boundaries is a simple byte-range filter on already-materialized captures.
- **No debounce or coalescing of rapid edits**: each keystroke still triggers a local interpolate, an edit acknowledgement, and one server parse. Same-word inheritance eliminates the visual flash during the round trip, so debounce is unnecessary for the common case.

## Further Actions

- **Low priority**: measure whether single-character edits near chunk boundaries (producing two replacement chunks) measurably affect query bytes or latency in real-world editing sessions. Current measurements show query bytes well under 128 bytes per edit; two-chunk envelopes are negligible.
- **Low priority**: consider per-editor-instance Criterion baselines on a stable CI host to track incremental-edit wall-clock latency across releases.
- **Deferred**: client-side Tree-sitter for zero-latency local classification was considered and deferred — the same-word inheritance approach provides sufficient local continuity without duplicating grammar/tree/query state on the client.
