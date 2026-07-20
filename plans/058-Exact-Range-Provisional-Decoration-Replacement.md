# Exact-Range Provisional Decoration Replacement

## Objectives

- Stop repeated edits before a decoration chunk boundary from exposing one additional default-colored byte in downstream code per edit.
- Make `DecorationSet` authority exact: current server output replaces only its declared viewport, even when optimistic client chunk geometry has shifted and overlaps that viewport.
- Preserve immediate local interpolation and existing one-parse/one-query server behavior without extra syntax chunks, protocol messages, debounce, client parser, or full-cache edit-path normalization.
- Keep provisional residual geometry and cache accounting bounded under repeated insertion, deletion, UTF-8 edits, empty authority, and current-version correction.

## Expected Outcome

- Typing repeatedly inside a Rust comment does not peel decoration from code after the nearest 128-byte boundary.
- Authoritative `[start,end)` output replaces provisional decoration inside `[start,end)` only; left/right provisional span fragments outside that range remain painted until current authority covers them.
- Insertions and deletions before chunk boundaries cannot create gaps or transient whole-block clearing while authoritative members are applied one at a time.
- Compatible provisional residuals are coalesced locally, so repeated edits do not create one retained chunk per keystroke or bypass `SYNTAX_CACHE_BUDGET_BYTES` accounting.
- Parser/query invocation count, query bytes, payload limits, package provenance, syntax/semantic layering, stale-version rejection, Clay JS APIs, and configuration remain unchanged.

## Evaluation

- `src/editor/surface.rs::EditorDecorationState::apply_edit` transforms chunk keys with document geometry. Inserting before a boundary changes `[0,128)`/`[128,256)` into provisional `[0,129)`/`[129,257)`.
- `src/server/syntax.rs::replacement_ranges` correctly recomputes current-version authoritative ranges on the stable 128-byte grid. A comment edit inside the first chunk may publish only `[0,128)`.
- `src/editor/surface.rs::EditorDecorationState::apply_set` currently removes any same-package/same-layer provisional chunk that intersects the authoritative key. Installing `[0,128)` while retaining the next provisional chunk at `[129,257)` leaves `[128,129)` unowned; each additional insertion grows that gap by one byte.
- Plan 057's query-coverage-equals-replacement-coverage invariant remains correct inside each server set. The remaining defect is client composition: whole overlapping provisional chunk removal discards geometry outside the server set's declared authority.
- The approved correction is exact range subtraction in `EditorDecorationState`, not server neighbor publication, atomic protocol batching, client-side re-chunking, debounce, larger chunks, or additional parsing.

## Tasks

- [x] Review the decoration replacement primitive and reproduce shifted-boundary gaps
  - Acceptance Criteria:
    - Functional: Add deterministic red regressions that compose real first-party Rust grammar output, optimistic repeated comment edits before a 128-byte boundary, edit acknowledgements, and every authoritative `DecorationSet`; prove one downstream decorated byte becomes default-colored per edit under current behavior. Cover insertion and deletion boundary drift separately.
    - Performance: Regressions retain one parser/query pass per accepted version and bounded windows; no timing-only assertion, full-document fixture, extra server publication, or test-only production callback is added.
    - Code Quality: Inventory existing `DecorationSet`/`DecorationChunkKey`, `EditorDecorationState::{apply_edit,apply_set}`, interpolation, replacement-grid, cache-budget, paint-observability, and syntax/semantic layering primitives before implementation. Lock the defect at the composed state-machine boundary rather than only unit-testing a helper.
    - Security: Use validated inert first-party fixture output only; preserve document versions, package/layer provenance, server authority, and no package/client JavaScript execution.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-07-19-2238-exact-range-provisional-decoration-replacement.md`: approved exact-range subtraction semantics and rejected alternatives.
      - `decision-logs/2026-07-19-1912-syntax-decoration-continuity-and-complete-authoritative-replacement.md`: retained same-word interpolation and complete server chunk guarantees.
      - `docs/reference/primitives/{index,registry,parse-update-strategy,rendering-strategy}.md`: parse/decor primitive contracts.
      - `docs/wiki/modules/{primitive-architecture,decoration-transport,masonry-editor,parse-task-lifecycle,syntax-grammar-registry}.md`: current client/server flow.
      - `.agents/skills/project-patterns/references/{protocol-and-performance,authority-boundaries,maintenance-validation}.md`.
    - Options Considered:
      - Unit-test range subtraction only: insufficient because Plan 057 stage-isolated tests missed composed chunk drift.
      - GPU/pixel snapshots: nondeterministic and broader than byte-level paint continuity.
      - Existing `EditorSurface::visible_decoration_paint_ranges_for_test` plus real grammar output. Chosen.
    - Chosen Approach:
      - Construct source whose decorated downstream token crosses or begins at a nominal 128-byte boundary, apply several one-byte edits before that boundary, and inspect visible painted ranges after local edit, acknowledgement, and each authoritative member. Keep pre-fix regressions ignored only until the implementation task enables them.
    - API Notes and Examples:
      ```text
      initial chunks:       [0,128) [128,256)
      optimistic insertion: [0,129) [129,257)
      authority arrives:    [0,128)
      current bug:          [0,128) [gap] [129,257)
      ```
    - Files to Create/Edit:
      - `tests/syntax_grammar.rs`: real Rust grammar/editor repeated-insertion regression through all state transitions.
      - `tests/decoration_transport.rs`: synthetic insertion/deletion/empty-authority boundary regressions where a smaller fixture is clearer.
    - References:
      - `src/editor/surface.rs::{EditorDecorationState::apply_edit,EditorDecorationState::apply_set,interpolate_range}`
      - `src/server/syntax.rs::{replacement_ranges,decoration_sets_for_ranges}`
      - `src/protocol/decorations.rs::{DecorationSet,DecorationChunkKey}`
  - Completion Evidence:
    - Primitive inventory confirmed no new protocol, parser, package, paint, or observability primitive is needed. `DecorationSet`/`DecorationChunkKey` already declare exact half-open authority; `EditorDecorationState::apply_edit` already shifts retained geometry and marks changed chunks provisional; `visible_decoration_paint_ranges_for_test` observes composed client paint state. The primitive gap is confined to `EditorDecorationState::apply_set`, which currently removes an entire intersecting provisional package/layer chunk.
    - Added ignored real-flow regression `plan058_repeated_comment_edits_do_not_grow_a_shifted_chunk_boundary_gap`. It parses a first-party Rust line comment crossing byte 128, applies three local one-byte edits and acknowledgements, performs one incremental `parse_sync`/one touched authoritative member per version, and inspects paint after every transition/member. Explicit ignored execution exits 101 with observed authority gap counts `[1, 2, 3]` instead of `[0, 0, 0]`, reproducing the reported one-byte-per-letter downstream whitening.
    - Added ignored transport regressions `plan058_empty_authority_after_insertion_preserves_shifted_right_residual` and `plan058_empty_authority_after_deletion_preserves_shifted_right_residual`. Explicit runs exit 101: insertion loses byte `128` outside authority `[0,128)`, while deletion removes every byte `128..139` from the right residual of the overlapping shifted chunk.
    - Generalized the existing integration-test notification helper with explicit base/current versions so repeated accepted edits retain canonical `ParseInputEdit` metadata without a production hook.
    - Existing metric test `server::syntax::tests::first_party_continuity_edits_keep_one_bounded_parse_and_query` passes and records one parse, one query range, and one member for the bounded Rust case (20 queried bytes), plus equivalent bounded results for all first-party languages. Red tests assert geometry rather than elapsed time and add no parser job, IPC publication, package JavaScript, or production callback.
    - Normal Linux validation passes with regressions ignored pending task 2: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --test syntax_grammar` (65 passed, 1 ignored), `cargo test --test decoration_transport` (17 passed, 2 ignored), full `cargo test --all-targets` including benchmark smoke targets, and `git diff --check`.
  - Test Cases Written:
    - Three one-byte Rust comment insertions before byte 128 expose exactly `[1, 2, 3]` downstream default-colored bytes with current behavior.
    - Deletion before byte 128 shifts the next provisional chunk left; applying empty authority `[0,128)` currently removes geometry `128..139` beyond authority.
    - Empty authority after insertion currently removes byte `128` outside its viewport rather than preserving the shifted right residual.
    - Existing first-party metrics retain one parser invocation, one query invocation, and one member per bounded edit; new failures are based on paint geometry, not elapsed time.

- [x] Replace only the exact authoritative viewport and coalesce local provisional residuals
  - Acceptance Criteria:
    - Functional: `EditorDecorationState::apply_set` subtracts the incoming authoritative `[viewport_byte_start,viewport_byte_end)` from every overlapping provisional chunk with the same package/layer, preserves left/right span fragments outside it, removes exact current authority inside it, installs incoming spans, and keeps semantic/diagnostic/search authority behavior unchanged. Empty sets clear exactly their declared viewport.
    - Performance: Work is bounded to the existing retained-chunk scan plus spans in overlapping provisional chunks; no parser/query/IPC work, full-document scan, full-cache sort/rechunk, global coalescing pass, per-keypress callback, or extra server chunk is introduced. Coalesce only compatible residuals touched by the current replacement and keep repeated-edit chunk count bounded.
    - Code Quality: Implement one small generic half-open range-subtraction primitive returning zero, one, or two span/range fragments; reuse it for chunk/span residual construction. Preserve UTF-8 byte-boundary validation assumptions, exact version/package/layer keys, deterministic ordering, and accurate/conservative cache accounting without language names or knowledge of `SYNTAX_DECORATION_CHUNK_BYTES`.
    - Security: Current-version server data remains final inside its viewport; provisional residuals cannot override authority, cross package/layer ownership, bypass provenance/style validation, survive resync/document replacement, or become executable state.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-07-19-2238-exact-range-provisional-decoration-replacement.md`.
      - `src/editor/surface.rs`: `EditorDecorationChunk`, `EditorDecorationState`, cache eviction/accounting, visible-span composition, optimistic transforms.
      - `src/protocol/decorations.rs`: half-open viewport/span semantics and package/layer chunk identity.
      - `.agents/skills/project-patterns/references/{protocol-and-performance,authority-boundaries}.md`.
    - Options Considered:
      - Publish neighboring server chunks: extra work and sequential-member transient risk.
      - Add atomic decoration batches: unnecessary protocol expansion and still requires old/new-grid coverage.
      - Rechunk optimistic state onto the server grid: duplicates syntax-specific server policy in generic client state.
      - Exact viewport subtraction plus local residual coalescing. Chosen.
    - Chosen Approach:
      - Before installing a set, drain only same-package/same-layer provisional overlaps through a helper that clips each span against authority and emits residual chunks for non-empty left/right geometry. Preserve unaffected chunks unchanged, remove the exact authoritative key, install non-empty authority, then coalesce compatible adjacent/overlapping provisional residuals produced/touched by this application. Recompute retained-byte accounting through the smallest existing serialization/accounting path that remains exact and measured.
    - API Notes and Examples:
      ```rust
      subtract(0..129, 0..128) == [128..129]
      subtract(127..257, 0..128) == [128..257]
      subtract(0..128, 0..128) == []
      ```
    - Files to Create/Edit:
      - `src/editor/surface.rs`: exact half-open range/span subtraction, residual chunk reconstruction, localized compatible coalescing, cache accounting, and focused unit tests.
      - `tests/syntax_grammar.rs`: enable repeated-comment real grammar regression.
      - `tests/decoration_transport.rs`: enable insertion/deletion/empty-authority composed regressions.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `tests/decoration_transport.rs::{optimistic_comment_style_outside_authority_survives_exact_replacement,rapid_local_versions_reject_stale_authority_without_losing_provisional_geometry}`
  - Completion Evidence:
    - `EditorDecorationState::apply_set` now removes the exact current key, subtracts only the incoming half-open viewport from overlapping provisional chunks with matching package/layer identity, preserves left/right span and chunk fragments outside authority, installs current authority last, and leaves different packages/layers untouched.
    - Added private generic `subtract_half_open_range`, provisional chunk reconstruction, one-neighbor-per-side localized residual coalescing, compatible adjacent/overlapping span coalescing, and serialized residual byte-size recomputation with conservative fallback. No syntax chunk-size knowledge, language branch, full-cache sort/rechunk, parser/query work, protocol message, package callback, or edit-path callback was added.
    - Enabled all three Plan 058 regressions. The real first-party Rust flow now records authority gap counts `[0, 0, 0]` after three repeated comment edits and checks continuity after local edit, acknowledgement, and each authoritative member; insertion/deletion empty-authority transport regressions preserve every byte outside `[0,128)`.
    - Added focused `src/editor/surface.rs` tests for zero/one/two half-open subtraction results, validated UTF-8 endpoint preservation, crossing-span left/authority/right splitting, exact authority with local right-residual coalescing, different-package and semantic-layer isolation, and four repeated edit/authority cycles retaining exactly two chunks with one provisional residual.
    - Updated the former whole-overlap transport expectation: empty `[0,7)` authority preserves provisional `[7,8)` until a separate authoritative `[7,8)` clear arrives. Existing stale-version rejection, resync clearing, semantic-layer preservation, and cache-budget tests remain green.
    - Updated `docs/wiki/modules/{decoration-transport,masonry-editor}.md` with exact-viewport subtraction and local residual coalescing behavior; updated the superseded Plan 056 wiki assertion to require current `exact half-open viewport` language.
    - Linux validation passes: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, all 99 `editor::surface::tests`, `syntax_grammar` (66 passed), `decoration_transport` (19 passed), `primitives_docs` (127 passed), full `cargo test --all-targets` including benchmark smoke targets, and `git diff --check`.
  - Test Cases Written:
    - Insertion drift: authority `[0,128)` preserves provisional byte `128` and retained next-chunk geometry; no visible gap.
    - Deletion drift: authority `[0,128)` preserves the shifted right residual `128..139`.
    - Crossing span splits into left/right residual fragments around authority; current authority owns the middle range.
    - Empty authority clears only its viewport and preserves neighboring provisional package/layer geometry.
    - Same viewport but different package or kind remains untouched; syntax authority does not clear semantic state.
    - Repeated insert/apply cycles keep residual chunk count bounded through localized chunk/span coalescing.
    - Validated UTF-8 scalar endpoints remain unchanged by subtraction.
    - Existing resync/document replacement and stale-version rejection tests retain clearing and authority behavior.

- [x] Verify continuity, latency, cache bounds, and Linux editing behavior
  - Acceptance Criteria:
    - Functional: Rust, TypeScript, TSX, JavaScript, and Markdown retain decoration across repeated edits before replacement boundaries; insertion, deletion, newline, punctuation, empty authority, rapid versions, and authoritative correction show no default-color gap after local edit, acknowledgement, or any streamed member. Manual Linux GUI editing confirms the reported Rust-comment scenario is fixed.
    - Performance: Confirm parser/query/member counts and query bytes are unchanged from Plan 057; measure authoritative apply cost and repeated residual coalescing on a bounded near-viewport fixture. No statistically significant first-party incremental-edit regression, unbounded chunk/span growth, cache-budget bypass, or paint/text-event parser/IPC work is allowed. Wall-clock values remain advisory without a stable CI host.
    - Code Quality: Verification uses generic package grammar/style maps and editor transport paths with no language-specific production branch. Linux `fmt`, `check`, `clippy -D warnings`, all targets, relevant Criterion benchmarks, and `git diff --check` pass.
    - Security: Validation, current-version checks, package/layer provenance, payload/cache budgets, server parser authority, and no package/client JavaScript hot-path rules remain unchanged.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/{performance,launch-and-gui-smoke}.md`.
      - `tests/fixtures/syntax/*` and first-party package highlight queries.
      - `.agents/skills/project-patterns/references/{maintenance-validation,protocol-and-performance}.md`.
    - Options Considered:
      - Deterministic tests only: rejected because prior plans passed while manual visual behavior remained wrong.
      - Manual smoke only: rejected because byte-gap and fragment-growth regressions need repeatable checks.
      - Deterministic composed matrix plus bounded Linux smoke and advisory measurements. Chosen.
    - Chosen Approach:
      - Extend existing Plan 057 continuity infrastructure with boundary-drift cases, add a bounded repeated-cycle cache/chunk-count assertion, run first-party incremental Criterion baselines, and manually repeat comment typing/deletion near a 128-byte boundary in `smoke-gui --config-fixture language-packages --profile-perf`.
    - API Notes and Examples:
      ```bash
      cargo test --test syntax_grammar
      cargo test --test decoration_transport
      cargo test --test editor_performance_invariants
      cargo bench --bench first_party_language_baselines -- first_party_incremental_edit
      cargo run -- smoke-gui --config-fixture language-packages --profile-perf
      ```
    - Files to Create/Edit:
      - `tests/syntax_grammar.rs`: five-language boundary continuity matrix and unchanged parse/query metrics.
      - `tests/decoration_transport.rs`: repeated residual/coalescing/cache scenarios.
      - `tests/editor_performance_invariants.rs`: static hot-path exclusions if helper placement requires coverage.
      - `src/editor/surface.rs`: private recorder/test coverage only if existing observability cannot prove bounded residual count.
      - `benches/first_party_language_baselines.rs`: bounded current-authority subtraction/coalescing apply benchmark.
      - `docs/development/performance.md`: measured local apply/cache and unchanged parser/query results.
      - `docs/development/launch-and-gui-smoke.md`: exact Rust-comment manual reproduction and expected checkpoints.
      - `tests/manual_smoke_docs.rs`: deterministic documentation guard.
    - References:
      - `benches/first_party_language_baselines.rs`
      - `docs/development/{performance,launch-and-gui-smoke}.md`
  - Completion Evidence:
    - Added `plan058_first_party_languages_preserve_shifted_boundary_continuity`: Rust, TypeScript, TSX, JavaScript, and Markdown each receive three repeated one-byte insertions before byte 128 through real package queries, optimistic local edit, acknowledgement, one incremental parse/member, every authoritative application, and byte-wise visible paint inspection. All five retain the complete decorated suffix with no base-color gap.
    - Added `plan058_repeated_insert_delete_authority_cycles_preserve_boundary_geometry`: 128 insertion/deletion pairs (256 current versions) alternate before a shifted boundary, validate every authoritative set, and assert every document byte remains painted after each apply. Existing Plan 057 newline/punctuation/authoritative-correction and Plan 058 empty-authority/rapid-version tests remain green.
    - Extended the private surface cache test to 512 repeated insert/apply cycles. Every cycle retains exactly two chunks/two spans with one provisional residual, recomputed byte accounting equals the chunk-byte sum, and retained bytes stay below `SYNTAX_CACHE_BUDGET_BYTES`.
    - Added `exact_range_decoration_replacement_stays_off_edit_and_paint_hot_paths`, proving subtraction/coalescing occurs only in authoritative `apply_set`, not local edit or paint bodies, with parser/IPC/package-JavaScript symbols absent.
    - Plan 057 parser/query/member metrics are unchanged exactly: one parser invocation, one query range, and one emitted member; queried bytes remain Rust 20, TypeScript 26, TSX 26, JavaScript 26, Markdown 17. No parser, query, IPC message, payload shape, or server publication changed.
    - Added optimized Criterion target `first_party_authoritative_replacement/apply_and_coalesce_residual`; one exact authority apply plus local residual coalescing measured 1.8150 µs (95% interval 1.6250–1.9959 µs, 20 samples, 1 s warm-up, 2 s measurement). Five-language incremental estimates were Rust 152.39 µs, TypeScript 344.39 µs, TSX 125.50 µs, JavaScript 123.55 µs, and Markdown 199.49 µs; Criterion reported one Rust improvement and no change for the other fixtures, with no statistically significant regression.
    - Manual X11 Linux smoke used the real `language-packages` managed server/client path and a temporary 150-byte Rust line comment before decorated code. Eight per-letter insertions inside the comment, then Backspace and Enter, produced acknowledgements/current authority through version 12; framebuffer checkpoints retained all downstream Rust decoration with no per-letter white peeling. Enter correctly ended only the comment. Temporary fixture content was restored and `/tmp` artifacts were not committed.
    - Recorded commands, deterministic bounds, measurements, and manual checkpoints in `docs/development/{performance,launch-and-gui-smoke}.md`; `plan058_linux_exact_range_smoke_and_measurements_are_recorded` locks the record.
    - Linux gates pass on kernel `7.1.3-43.stable` x86_64 with Rust/Cargo 1.96.1: `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, `cargo bench --no-run`, and `git diff --check`. Focused suites pass: `syntax_grammar` 67, `decoration_transport` 20, `editor_performance_invariants` 23, `manual_smoke_docs` 20, surface tests 99, syntax unit tests 6.
  - Test Cases Written:
    - Five-language repeated insertions before a decorated boundary preserve every downstream painted byte after local edit, acknowledgement, and each authoritative member.
    - 128 repeated insertion/deletion pairs preserve downstream geometry and current authority.
    - 512 bounded insert/apply cycles keep chunk/span counts, provisional count, retained bytes, and cache accounting under deterministic ceilings.
    - Parser invocation, query invocation, emitted-member count, and query-byte metrics match Plan 057 behavior exactly.
    - Authoritative subtraction/coalescing stays outside local-edit and paint hot paths.
    - Manual Rust line-comment typing, Backspace, and Enter retain all later code decoration.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Audit changed Rust functions/types and confirm exact provisional subtraction/coalescing remains internal client replacement behavior; expose a Clay JS API only if implementation introduces a genuine caller-controlled capability.
    - Performance: No API permits per-keypress callbacks, provisional-state mutation, coalescing control, replacement-range override, cache tuning, or paint-path JavaScript.
    - Code Quality: Any necessary public capability follows facade/op/inventory/Markdown/generated-registry conventions; expected result is no new API and private helper functions.
    - Security: No API bypasses current-version authority, package/layer provenance, decoration validation, payload/cache budgets, or server ownership.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: mandatory Clay JS API audit.
      - `.agents/skills/project-patterns/references/{clay-js-api-boundary,clay-js-api-naming,clay-js-api-schema,documentation-as-code,doc-registry-tests}.md`.
      - Existing `clay:decorations`, `clay:syntax`, and `clay:parse` docs/inventory.
    - Options Considered:
      - Expose provisional subtraction/coalescing controls: rejected as correctness internals.
      - Keep behavior behind existing inert decoration transport. Expected/chosen.
    - Chosen Approach:
      - Audit visibility and inventories after implementation; add no speculative facade, op, custom property, or registry entry.
    - API Notes and Examples:
      ```text
      Expected public surface change: none.
      Existing serverPublishDecorations/serverRegisterSyntaxGrammar/serverRegisterParseHandler remain sufficient.
      ```
    - Files to Create/Edit:
      - `tests/{clay_js_api_inventory,rust_visibility_api_mapping,clay_js_doc_registry}.rs`: add Plan 058 rejection/visibility coverage only where needed.
      - `docs/reference/clay-js-api/**`, facade/op files, `docs/index.md`, and generated registry only if audit proves a new public capability exists.
    - References:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
  - Test Cases to Write:
    - New subtraction/coalescing helpers remain private and absent from JS facades/inventory.
    - No provisional replacement, residual, chunk-coalescing, cache-size, or debounce control appears in public surfaces.
    - Existing generated registry remains current.
  - Completion Evidence:
    - `plan058_exact_range_replacement_internals_reuse_existing_clay_js_apis`: 3 existing public surfaces retained; 9 Plan 058 internal names (subtract_half_open_range, subtract_provisional_chunk, coalesce_local_residual, coalesce_compatible_spans, decoration_chunk_byte_size, DecorationResidualSide, setSyntaxExactRangeReplacement, setSyntaxProvisionalSubtraction, setSyntaxResidualCoalescing) rejected from JS facade exports; 13 hidden config names (syntaxExactRangeReplacement, syntaxProvisionalSubtraction, syntaxResidualCoalescing, syntaxSubtractionCoalescing, syntaxExactRangeSubtraction, syntaxProvisionalResidual, syntaxCoalescingStrategy, plus 6 set* variants) rejected from facades/ops/inventory/registry.
    - Configuration docs: Plan 058 section confirms no new clay:configuration API, exact-range subtraction/coalescing as compiled invariants, hidden key rejection, authoritative subtraction staying outside edit/paint hot paths.
    - Wiki review: Plan 058 Clay JS and Configuration audit paragraphs, full implementation section with root cause, fix description (subtract/install/coalesce), source files, test names, verification results, benchmark measurements (1.815µs median apply, no incremental regression), decision log + plan added to source list.
    - All 62 clay_js_api_inventory tests pass, full cargo test --all-targets green (0 failures), cargo fmt/check/clippy clean.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Confirm exact-range subtraction and localized residual coalescing are fixed correctness defaults, not optional user policy; add configuration only if implementation discovers a genuine user choice.
    - Performance: No dynamic replacement, coalescing, residual-count, chunk-grid, debounce, or client-parser setting enters edit/apply/paint paths.
    - Code Quality: Any genuine setting must be a documented Clay JS API through `init.js`; expected result is no new setting.
    - Security: Configuration grants no provisional decoration authority, parser code, filesystem, network, shell, package, raw-op, cache-budget, or client-JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: mandatory configuration audit.
      - `.agents/skills/project-patterns/references/configuration-system.md`.
      - `docs/reference/clay-js-api/configuration.md`.
    - Options Considered:
      - User-selectable residual preservation/coalescing: rejected because correctness cannot be optional.
      - Compiled validated behavior with regression coverage. Chosen.
    - Chosen Approach:
      - Keep the fix non-configurable and verify hidden keys/facades are absent.
    - API Notes and Examples:
      ```text
      No planned clay.configuration.* additions.
      ```
    - Files to Create/Edit:
      - `tests/clay_js_api_inventory.rs`: reject Plan 058 hidden setting/facade names.
      - `docs/reference/clay-js-api/configuration.md`: record Plan 058 audit only if deterministic docs coverage requires an explicit section.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
  - Test Cases to Write:
    - Inventories reject `syntaxPreserveProvisionalResiduals`, `syntaxDecorationResidualCoalescing`, `syntaxAuthoritativeReplacementMode`, `syntaxDecorationChunkGrid`, and equivalent setters.
    - Existing syntax engine preference remains the only relevant syntax-engine user choice.
  - Completion Evidence:
    - `plan058_exact_range_replacement_internals_reuse_existing_clay_js_apis` rejects 17 hidden config names (syntaxExactRangeReplacement, syntaxProvisionalSubtraction, syntaxResidualCoalescing, syntaxSubtractionCoalescing, syntaxExactRangeSubtraction, syntaxProvisionalResidual, syntaxCoalescingStrategy, syntaxPreserveProvisionalResiduals, syntaxDecorationResidualCoalescing, syntaxAuthoritativeReplacementMode, syntaxDecorationChunkGrid, plus 6 set* variants) from all implementation surfaces.
    - `plan056_syntax_latency_configuration_stays_compiled_and_non_configurable` still passes (setSyntaxEnginePreference retains only target/tier).
    - Configuration docs: Plan 058 section confirms exact-range subtraction/coalescing as compiled invariants, no clay:configuration API, hidden key rejection, authoritative subtraction outside edit/paint paths.
    - Wiki Configuration Audit: Plan 058 paragraph documents no clay:configuration surface, exact-range authoritative viewport subtraction and local provisional residual coalescing as compiled correctness invariants.
    - Full clay_js_api_inventory 62 passed, all cargo test --all-targets green.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation, verification, Clay JS API, and configuration tasks complete.
    - Performance: Wiki updates add no runtime work and document localized overlap/span complexity, bounded residual coalescing, cache accounting, and unchanged parser/query/transport behavior.
    - Code Quality: Wiki pages explain exact authoritative viewport subtraction, left/right residual preservation, local coalescing, invariants/tradeoffs, source/test paths, and links from the master wiki index; stale whole-overlap-deletion wording is removed.
    - Security: Wiki pages preserve current server authority inside the declared viewport, package/layer isolation, inert client interpolation, validation, and no-new-authority boundaries without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: project wiki workflow and quality bar.
      - `.agents/skills/create-plan/references/wiki-task.md`.
      - `docs/wiki/index.md` and relevant decoration/editor/syntax pages.
    - Options Considered:
      - Update after each implementation step: noisy and likely to document intermediate residual behavior.
      - Update once after final tests/audits pass. Chosen.
    - Chosen Approach:
      - Update existing module/review pages once, keep index navigation current, and add deterministic documentation coverage rejecting old whole-overlap deletion semantics.
    - API Notes and Examples:
      ```text
      provisional overlap - authoritative viewport = left/right residuals
      residuals + authoritative spans -> gap-free current paint state
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: verify navigation remains current.
      - `docs/wiki/modules/{decoration-transport,masonry-editor,low-latency-incremental-syntax-decoration-primitive-review}.md`: final exact-range replacement flow, complexity, authority, tests, and tradeoffs.
      - `docs/reference/primitives/{rendering-strategy,parse-update-strategy}.md`: update replacement semantics if current wording implies whole overlapping provisional chunks are deleted.
      - `tests/primitives_docs.rs`: deterministic Plan 058 wiki/reference contract and stale-wording rejection.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - `decision-logs/2026-07-19-2238-exact-range-provisional-decoration-replacement.md`
  - Test Cases to Write:
    - Wiki index retains links to relevant pages.
    - Documentation requires exact authoritative viewport subtraction, preserved left/right provisional residuals, and localized compatible coalescing.
    - Documentation rejects wording that any overlapping provisional chunk is removed wholesale.
    - `cargo test --test primitives_docs`.
  - Completion Evidence:
    - Added `plan058_final_wiki_records_exact_range_replacement_and_residual_coalescing` primitives_docs test (128 total): asserts wiki review contains Plan 058 section with root cause, fix description, source files, test names, and benchmark measurements; decoration-transport and masonry-editor contain exact half-open viewport subtraction and residual coalescing language; primitives reference docs contain exact-range subtraction phrases and reject superseded whole-provisional-replacement wording.
    - Updated `parse-update-strategy.md`: client-side replacement line now describes exact half-open viewport subtraction with left/right residual preservation and local coalescing.
    - Updated `rendering-strategy.md`: authoritative-set replacement line now describes exact half-open viewport subtraction with residual preservation and coalescing.
    - Existing Plan 056/057 wiki tests still pass (plan056_final_wiki, plan057_final_wiki, low_latency_syntax_reference_docs_preserve_parse_and_fan_out_contract).
    - Full test suite green (0 failures), cargo fmt/check/clippy clean.

## Compromises Made

- Exact-range subtraction operates on individual provisional spans within chunks rather than chunk-level set operations, so worst-case residual fragment count in a pathological alternating-authority pattern could approach the number of spans; the 512-iteration bounded test and `SYNTAX_CACHE_BUDGET_BYTES` ceiling confirm practical bounded behavior.
- Subtraction/coalescing runs synchronously in `apply_set` when the asynchronous authoritative server set arrives, not in the local-edit hot path; this trades a microsecond-scale apply cost for correctness of downstream provisional geometry.
- Left/right residual fragments that abut the authority boundary and are compatible with adjacent chunks are coalesced eagerly to prevent per-keystroke fragment accumulation; coalescing uses the same span-compatibility rules as chunk merging.

## Further Actions

- Low priority: measure residual fragment count in multi-hour editing sessions with real-world alternating comment/code patterns to confirm the 512-cycle bounded ceiling holds indefinitely.
- Low priority: if future editor states add more decoration layers beyond Syntax/Semantic/Diagnostic/SearchMatch, verify the per-package/layer isolation in `apply_set` remains correct.
- Deferred: client-side decoration chunk grid awareness. The client still shifts retained provisional chunks by `inserted_len` on every edit, but exact-range subtraction now correctly trims the shifted boundary on each authoritative arrival. Making the client aware of the 128-byte grid would eliminate the shift altogether but couples client/server internals — defer until proven necessary by latency measurements.<｜end▁of▁thinking｜>

<｜｜DSML｜｜tool_calls>
<｜｜DSML｜｜invoke name="edit">
<｜｜DSML｜｜parameter name="newText" string="true">## Compromises Made

- Exact-range subtraction operates on individual provisional spans within chunks rather than chunk-level set operations, so worst-case residual fragment count in a pathological alternating-authority pattern could approach the number of spans; the 512-iteration bounded test and `SYNTAX_CACHE_BUDGET_BYTES` ceiling confirm practical bounded behavior.
- Subtraction/coalescing runs synchronously in `apply_set` when the asynchronous authoritative server set arrives, not in the local-edit hot path; this trades a microsecond-scale apply cost for correctness of downstream provisional geometry.
- Left/right residual fragments that abut the authority boundary and are compatible with adjacent chunks are coalesced eagerly to prevent per-keystroke fragment accumulation; coalescing uses the same span-compatibility rules as chunk merging.

## Further Actions

- Low priority: measure residual fragment count in multi-hour editing sessions with real-world alternating comment/code patterns to confirm the 512-cycle bounded ceiling holds indefinitely.
- Low priority: if future editor states add more decoration layers beyond Syntax/Semantic/Diagnostic/SearchMatch, verify the per-package/layer isolation in `apply_set` remains correct.
- Deferred: client-side decoration chunk grid awareness. The client still shifts retained provisional chunks by `inserted_len` on every edit, but exact-range subtraction now correctly trims the shifted boundary on each authoritative arrival. Making the client aware of the 128-byte grid would eliminate the shift altogether but couples client/server internals — defer until proven necessary by latency measurements.
