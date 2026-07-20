# IPC Framing Cancellation Safety and Markdown Inline Syntax

## Objectives
- Eliminate the IPC frame desync that disconnects the client (`frame length 1080257633` / `"@cla"`) by making framed reads cancellation-safe on both connection ends.
- Make the default prose palette visually distinct (heading levels, code, links, quotes, lists, plain text).
- Style Markdown inline constructs (`**bold**`, `_italic_`, `` `code` ``, links, autolinks) inside mixed prose through a generic Tree-sitter injection/composite-grammar primitive, not Markdown-specific Rust branches or query predicates.
- Give syntax captures a declarative precedence so narrow captures (link, code span) override broad prose captures deterministically.
- Reduce decoration IPC/event pressure with bounded batched decoration transport.

## Expected Outcome
- Opening and editing Markdown files no longer disconnects the client; fragmented-frame regression tests pass on client and server.
- Default theme renders distinct colors for Heading1–6, ListItem, Quote, CodeBlock, CodeSpan, Link; Paragraph stays base text color.
- Mixed-prose inline Markdown renders per-span styles (bold/italic/code/link) via the already-declared `queries/injections.scm` mechanism and the crate-provided `INLINE_LANGUAGE`.
- Decoration transport sends one bounded `DecorationBatch` per parse update instead of dozens of single-chunk frames.
- `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and targeted tests pass on Linux.

## Tasks

- [x] Review existing editor primitives and plan generic primitive gaps before implementation
  - Acceptance Criteria:
    - Functional: Written inventory of existing primitives (codec, decoration transport, parse coordinator, styleMap promotion, theme registry) with what each fix can reuse; every new primitive is generic and reusable across future modes/languages.
    - Performance: Confirmed new primitives keep IPC out of paint/text-event hot paths and keep parse work background/bounded.
    - Code Quality: No Markdown-specific Rust control flow proposed; new primitives documented before implementation.
    - Security: No new authority (file IO, network, shell, WASM, remote listener) introduced by any proposed primitive.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, `docs/reference/primitives/syntax-vocabulary.md`
      - `docs/wiki/modules/protocol-codec.md`, `docs/wiki/modules/decoration-transport.md`, `docs/wiki/modules/syntax-grammar-registry.md`, `docs/wiki/modules/editor-theme-registry.md`, `docs/wiki/modules/first-party-markdown-package.md`
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`, `.agents/skills/project-patterns/references/protocol-and-performance.md`
    - Options Considered:
      - Markdown-only special cases: smaller diff now, but violates primitive-first rule and blocks future embedded languages.
      - Generic primitives (framed I/O pump, injection engine, capture priority, decoration batch): reusable, matches decision-log precedent.
    - Chosen Approach:
      - Generic primitives only; the injection engine executes the already-declared-but-unused `queries.injections` contribution.
    - API Notes and Examples:
      ```text
      tree-sitter-md-025 0.5.6: LANGUAGE (block) + INLINE_LANGUAGE (inline);
      inline parsing requires ts_parser_set_included_ranges over `inline` nodes.
      tree-sitter-markdown/queries/injections.scm already declares
      ((inline) @injection.content (#set! injection.language "markdown_inline")).
      ```
    - Files to Create/Edit:
      - None (assessment recorded in this plan's task notes and later wiki).
    - References:
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
      - `decision-logs/2026-06-29-2006-package-provided-grammar-and-capability-phases.md`
  - Test Cases to Write:
    - None directly; gates the implementation tasks below.
  - Task Notes (completed 2026-07-20):
    - **Primitive inventory and reuse assessment:**
      - `Codec` (`src/protocol/codec.rs`): sole serialization boundary; big-endian 4-byte length prefix, `DEFAULT_MAX_FRAME_SIZE` 1 MiB, `rkyv::from_bytes` checked validation. Reused as-is; gap is cancellation safety of its async callers, not the codec. Fix: generic per-connection read/write pumps in `src/client/mod.rs` and `src/server/connection.rs`; no protocol semantic change, no version bump needed for this fix.
      - Decoration transport (`src/protocol/decorations.rs`, `src/server/decorations.rs`, `src/editor/surface.rs`): `DecorationSet`/`DecorationSpan`/`DecorationChunkKey` already carry version, viewport, provenance, priority, `TokenType` + `Modifiers`, and Plan 058 exact-range replacement. Fully reused for inline spans; transport-only change is the new bounded `ServerMessage::DecorationBatch` (protocol version bump per codec invariant).
      - `SyntaxGrammarRegistry` + `TreeSitterSyntaxHandler` (`src/server/syntax.rs`): generic tiered engine, one parse/capture pass per version/window, `SyntaxCapture { byte_start, byte_end, capture_name }` engine-neutral extraction, shared `map_capture_to_vocabulary`. Reused wholesale. Gaps: (a) `NativeGrammarDescriptor` has no embedded-grammar/injections field and line 868 constructs contributions with `injections_query_path: None`; (b) `packages/*/package.json` declare no `queries.injections`; (c) no injection executor exists despite the schema field. Fix: generic injection layer resolving embedded grammars by language name against a registered table; Markdown registers `markdown_inline` → `tree_sitter_md_025::INLINE_LANGUAGE`.
      - StyleMap promotion: native style maps are `(&str, TokenType, Modifiers, Option<DocumentFontRole>)` tuples; package styleMaps are validated in `src/packages/record.rs` with no priority field (existing `priority` fields there belong to keybindings/completion providers only). Gap: add optional bounded `priority` to styleMap entries (package schema) and native tuples; `MARKDOWN_NATIVE_STYLE_MAP` also needs inline-capture aliases (`strong_emphasis`→`strong`, `code_span`→`code-span`, `inline_link`/`uri_autolink`/`email_autolink`→`link`).
      - `StyleRegistry` (`src/editor/theme.rs`): single color source; per-token `[Color; 35]` table + `style_for`; theme override path generic. Reused; only default table entries change (data, not shape).
      - Overlap resolution (`font_role_precedes` in `src/editor/surface.rs`): priority → layer → provenance → role; attributes compose. Reused as the consumer of the new styleMap priority.
    - **What existing primitives already achieve:** default palette fix needs zero new primitives; IPC fix needs no protocol change; batching is a pure transport addition.
    - **New generic primitives required (no Markdown-specific Rust control flow):**
      1. Framed I/O pump ownership per connection (transport architecture, language-agnostic).
      2. Tree-sitter injection executor consuming declared `queries.injections` (`@injection.content` + `@injection.language`/`#set!`), generic for any host/embedded grammar pair (also enables future fenced-code highlighting via the same block `injections.scm`).
      3. Optional styleMap `priority` field (declarative capture precedence for all grammars).
      4. `DecorationBatch` wire variant (transport efficiency, language-agnostic).
    - **Explicitly rejected:** Markdown-named Rust branches, more query predicates, raising the frame limit, exposing engine budgets as user config (internal constants until measured).
    - **Authority/hot-path confirmation:** injection parsing stays in `ParseCoordinator` background tasks; paint stays on cached inert spans (`tests/editor_performance_invariants.rs` guards unchanged); no new permissions, no filesystem/network/shell/native-load authority; third-party grammar loading remains deferred per package-provided-grammar decision.

- [x] Make framed IPC reads cancellation-safe on client and server
  - Acceptance Criteria:
    - Functional: A competing `select!` branch can never strand a partially-read frame; the `0x40636c61` (`"@cla"`) desync class is impossible by construction on both ends.
    - Performance: No extra copies beyond current buffer reuse; no IPC work added to Masonry paint/text handlers; write pump batches ready queue items without blocking edits.
    - Code Quality: Read/write pump ownership is per-connection; `tokio::select!` no longer races `read_exact` futures; protocol semantics unchanged.
    - Security: 1 MiB frame ceiling and archived-bytes validation unchanged; all IPC input still treated as fallible.
  - Approach:
    - Documentation Reviewed:
      - Local tokio 1.52.2 docs (`src/macros/select.rs`, `src/io/util/async_read_ext.rs`): `read_exact` is explicitly not cancellation-safe.
      - `src/protocol/codec.rs` (`Codec::read_server_message`/`read_client_message`, `DEFAULT_MAX_FRAME_SIZE`).
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
    - Options Considered:
      - Raise frame limit: treats symptom only; desync remains. Rejected.
      - Hold read future across loop iterations: fragile pin/state juggling. Rejected.
      - Split connection into a persistent reader task (owns the read half, decodes whole frames, forwards via mpsc) and a serialized writer task (owns the write half, single writer). Select only over channels. Chosen.
    - Chosen Approach:
      - Per-connection read pump + write pump on both `src/client/mod.rs` and `src/server/connection.rs`; the `select!` loop multiplexes channels, never in-progress frame reads. Mirrors the existing client `io::split` shape but removes cancellation of partial reads.
    - API Notes and Examples:
      ```rust
      // reader pump: loop { let msg = codec.read_server_message(&mut read).await?; tx.send(msg).await?; }
      // select! { msg = rx.recv() => ..., edit = edit_rx.recv() => write_tx.send(...) }
      ```
    - Files to Create/Edit:
      - `src/client/mod.rs`: replace read-in-`select!` with read-pump task + channel; keep single writer (or writer task if half-clone required).
      - `src/server/connection.rs`: same restructure for the server connection loop.
      - `src/protocol/codec.rs`: no semantic change expected; may expose a `DuplexStream` test helper.
    - References:
      - Tokio `select!` cancellation-safety docs (local crate source).
      - Commit `adc95b8` (prior IPC bug fix) for protocol-version guard context.
  - Test Cases to Write:
    - Client fragmented-frame regression: server writes a large `DecorationSet` frame byte-by-byte while the client queue concurrently receives an edit; assert no `ConnectionError` and full frame decode.
    - Server fragmented-frame regression: client writes a frame byte-by-byte while typography/parse updates fire; assert no disconnect and full decode.
    - Broadcast-lag recovery still delivers complete latest-state recovery after the pump restructure.
  - Completion Notes (2026-07-20):
    - Added `ReadPumpGuard` in `src/protocol/codec.rs` (aborts pump on connection-loop exit; documents the `read_exact` cancellation hazard).
    - Client `run_connection`: dedicated read-pump task owns the read half and forwards `Result<ServerMessage, CodecError>` over an `EDIT_QUEUE_CAPACITY` channel; loop selects only over `outgoing_edits`/`incoming_rx`; `writer` stays the single write half; `S` bound extended with `Send + 'static`.
    - Server `handle_connection_with_analysis`: post-handshake `tokio::io::split` with the write half rebound as `stream` (49 write sites untouched); read pump forwards over a 64-slot channel; select read branch is `incoming_rx.recv()`; `None`/clean-EOF arms share the cleanup path.
    - Tests: `client::tests::fragmented_frame_survives_concurrent_outgoing_message` (drip-fed DecorationSet + racing viewport request, second ActiveTheme frame proves alignment), `server::connection::tests::fragmented_client_frame_survives_concurrent_server_write` (drip-fed ListDocuments + mid-frame typography broadcast, second request proves alignment).
    - Validation: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (37 suites, 0 failures).
    - Deviation: no broadcast-lag-specific new test needed; existing runtime-generation suite covers latest-state recovery and stayed green through the restructure.

- [x] Correct the default prose palette in StyleRegistry::clay_default
  - Acceptance Criteria:
    - Functional: Heading1–6 get six distinct colors (bold via attributes), CodeBlock/CodeSpan distinct from headings, Link distinct with underline, Quote/ListItem distinct; Paragraph unchanged (base text color).
    - Performance: No runtime cost change; color table remains a static `[Color; 35]`.
    - Code Quality: Theme override path (`parse_override_token`) untouched; packaged Gruvbox themes still override every token independently.
    - Security: No new inputs; no behavior surface change.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/editor-theme-registry.md` (Paragraph → base.text contract).
      - `packages/theme-gruvbox-material-dark/package.json` / `-light/package.json` palettes as the distinctness reference.
    - Options Considered:
      - Reuse Gruvbox palette verbatim in the default: ties default to one external theme's taste.
      - Pick a small distinct default palette aligned with the existing default accent family. Chosen.
    - Chosen Approach:
      - Minimal edit of the prose token entries in `StyleRegistry::clay_default()`; add underline to Link via existing attributes plumbing if the default registry supports it, otherwise color-only distinction (note compromise).
    - API Notes and Examples:
      ```rust
      // src/editor/theme.rs: syntax table entries indexed by TokenType::index()
      ```
    - Files to Create/Edit:
      - `src/editor/theme.rs`: distinct default colors for Heading1–6, ListItem, Quote, CodeBlock, CodeSpan, Link.
    - References:
      - `docs/reference/primitives/syntax-vocabulary.md` (prose token list).
  - Test Cases to Write:
    - Default-palette distinctness: pairwise-unequal colors for Heading1–6, CodeBlock, CodeSpan, Link, Quote, ListItem; Paragraph equals base text.
  - Completion Notes (2026-07-20):
    - `src/editor/theme.rs::clay_default`: Heading1..6 step through red/yellow/green/blue/purple/teal (alpha 0x55) with `ATTR_BOLD` defaults; `ListItem` gray; `Quote` italic gray; `CodeBlock` green; `CodeSpan` yellow; `Link` blue with `ATTR_UNDERLINE`; `Paragraph` unchanged at `base.text`. Monospace for code stays span-driven (font role), not theme-driven.
    - Tests: extended `style_for_drives_color_from_kind_and_token_type` (Heading1!=Heading2 + bold, Paragraph->base.text, Link underline + distinct, Quote italic, CodeSpan distinct); updated the Plan 046 baseline lock `free_form_style_token_decoration_colors_baseline_locked` in `src/editor/surface.rs` (`markup.heading.1` now red) with an intentional-revision comment.
    - Wiki: `docs/wiki/modules/editor-theme-registry.md` default-table paragraph records the prose palette revision.
    - Validation: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (0 failures).

- [x] Implement the generic Tree-sitter injection/composite-grammar engine
  - Acceptance Criteria:
    - Functional: Grammar packages' `queries.injections` contributions execute generically: host parse → collect `@injection.content` ranges + `@injection.language` / `#set! injection.language` → parse each range with the registered embedded grammar (parser `set_included_ranges`) → run the embedded grammar's highlights query → emit `DecorationSpan`s with host-package provenance. Markdown block+inline (`markdown_inline`) works through this path with no Markdown-named Rust branch.
    - Performance: One host parse per accepted version/window (unchanged); injection parsing bounded by window bytes, recursion depth (≤ 2 layers), and per-layer cache keyed by (document version, window, language layer); no parser-job multiplication over the same window.
    - Code Quality: Engine lives beside `TreeSitterSyntaxHandler` as a generic composition layer; grammar registration gains an optional embedded-grammar resolver; style-token mapping unchanged (`MARKDOWN_NATIVE_STYLE_MAP` reused via capture aliases).
    - Security: Only resolver-validated first-party grammar artifacts; no arbitrary third-party native grammar loading (per package-provided-grammar decision); query/payload bounds unchanged.
  - Approach:
    - Documentation Reviewed:
      - `tree-sitter-md-025` 0.5.6 crate README/source: two grammars, `LANGUAGE` + `INLINE_LANGUAGE`, `ts_parser_set_included_ranges` requirement.
      - tree-sitter 0.25.10 Rust binding: `Query::property_settings(index)` for `#set!`, `QueryCursor`, `Parser::set_included_ranges`.
      - `tree-sitter-markdown/queries/injections.scm`, `tree-sitter-markdown-inline/queries/{highlights,injections}.scm` in the local crate registry.
      - `docs/wiki/modules/syntax-grammar-registry.md` (current predicate workaround and its stated limit).
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`, `protocol-and-performance.md`.
    - Options Considered:
      - Keep block grammar + extend query predicates: cannot split mixed inline runs; known dead end documented in the wiki. Rejected.
      - Markdown-specific inline post-pass in Rust: violates primitive-first rule. Rejected.
      - Generic injection executor over declared `queries.injections`. Chosen.
    - Chosen Approach:
      - Execute the existing (currently unused) `injections_query_path` contribution. For each pattern, read `@injection.content` captures and the language from `@injection.language` capture text or `#set! injection.language` property settings; resolve the embedded grammar from a registry keyed by language name (`markdown_inline` → `tree_sitter_md_025::INLINE_LANGUAGE` for the built-in Markdown grammar); parse content ranges with `set_included_ranges`; run the embedded highlights query; translate captures through the same styleMap promotion. Remove the standalone-inline predicate hacks from `packages/markdown/queries/highlights.scm` once injection covers them; add aliases for inline capture names (`strong_emphasis` → `strong`, `code_span` → `code-span`, `inline_link`/`uri_autolink`/`email_autolink` → `link`) so `MARKDOWN_NATIVE_STYLE_MAP` applies unchanged.
    - API Notes and Examples:
      ```rust
      // tree-sitter 0.25.10
      parser.set_included_ranges(&[Range { start_byte, end_byte, .. }])?;
      query.property_settings(pattern_index) // -> &[QueryProperty] for #set! injection.language
      ```
    - Files to Create/Edit:
      - `src/server/syntax.rs`: generic injection executor (collect ranges, resolve embedded grammar, layered parse, capture translation); embedded-grammar registration on the descriptor/registry; remove Markdown predicate dependence.
      - `packages/markdown/queries/highlights.scm`: drop standalone-inline predicate patterns now covered by the inline layer (keep block captures).
      - `packages/markdown/queries/injections.scm` + `packages/markdown/package.json`: add the `queries.injections` contribution (confirmed absent in task 1); content mirrors upstream block `injections.scm` (`inline` → `markdown_inline`).
      - `src/server/syntax.rs`: extend `NativeGrammarDescriptor` with an optional embedded-grammar/injections field (confirmed absent in task 1; line 868 hard-codes `injections_query_path: None`).
      - `src/packages/record.rs`: only if the injections contribution is not already wired into the native descriptor.
    - References:
      - `decision-logs/2026-06-29-2006-package-provided-grammar-and-capability-phases.md` (grammar-only package constraints).
      - `runtime/js/syntax.ts` (`queries.injections` declaration surface).
  - Test Cases to Write:
    - Mixed-prose inline: `Plain **bold** and \`code\` plus [link](url).` yields per-span Paragraph+Bold, CodeSpan, Link decorations inside one paragraph.
    - Inline inside headings: `# Title with \`code\`` keeps Heading1 base with CodeSpan sub-range winning.
    - Autolinks: `<https://example.com>` and `<a@b.c>` map to Link.
    - Nested injection guard: html/yaml/latex injection languages without a registered grammar produce no decorations and no error.
    - Bounds: oversized window, depth cap, and cache reuse verified (no duplicate parse jobs for an unchanged window).
    - Existing `first_party_language_fixtures_produce_themed_vocabulary_decorations` and markdown vocabulary tests stay green.
  - Completion Notes (2026-07-20):
    - Engine (`src/server/syntax.rs`): `TreeSitterSyntaxHandler.enable_injections(query)` compiles a host-language injection query (`@injection.content` required, `@injection.language` optional); `injection_captures_for_window` groups content ranges per language (from `#set! injection.language` property settings via `Query::property_settings`, or `@injection.language` capture text), re-parses each group with `Parser::set_included_ranges`, and emits embedded highlight captures through the same styleMap/provenance pipeline. Unregistered language names and timed-out embedded parses are skipped so host decorations still ship.
    - Resolver: `FIRST_PARTY_EMBEDDED_GRAMMARS` static keyed by injection language name (`markdown_inline` → `tree_sitter_md_025::INLINE_LANGUAGE` + `packages/markdown/queries/inline-highlights.scm`); layer parsers cached per name in `InjectionState`. `NativeGrammarDescriptor` gained `injections_query_path`/`injections_query` (markdown only); `native_handler` wires it; `contribution_from_native_descriptor` propagates the path.
    - Queries: new `packages/markdown/queries/injections.scm` (`inline`/`pipe_table_cell` → `markdown_inline`, fenced-code info-string language) and `inline-highlights.scm` (reuses existing styleMap keys `strong`/`emphasis`/`code-span`/`link` — no styleMap change needed); retired the regex-predicate standalone-inline hacks from `highlights.scm`; `package.json` gained `queries.injections`.
    - Tests: `markdown_inline_injection_styles_mixed_runs` (mixed-run bold/italic/code/link, code-inside-heading, uri/email autolinks, unregistered fenced language skipped with CodeBlock intact); `markdown_native_descriptor_enables_inline_injection` (descriptor wiring, rust stays single-language); updated vocabulary/continuity tests to enable injections like production. Note: spans crossing the 128-byte chunk boundary split per chunk; editor-side plan058 coalescing keeps rendering continuous — tests assert start-byte coverage.
    - Allowlist: `enable_injections` added to `rust_visibility_api_mapping` non-JS infrastructure list.
    - Wiki: `syntax-grammar-registry.md` (predicate workaround paragraph replaced with executor description) and `first-party-markdown-package.md` (Phase 18.18 paragraph) updated.
    - Skipped (note in code): recursion into embedded grammars' own injections (html/latex inside inline) — depth is 2 layers per acceptance; add if a package ever needs 3.
    - Validation: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (37 suites, 0 failures).

- [x] Add a declarative capture-priority primitive to styleMap
  - Acceptance Criteria:
    - Functional: styleMap entries accept an optional priority; narrow captures (Link, CodeSpan, strong/emphasis) outrank broad prose (Paragraph, Heading) when ranges overlap; omitted priority keeps today's default (70).
    - Performance: Constant-time priority read per span; normalization path unchanged.
    - Code Quality: Priority lives in the style-map promotion layer, generic for all grammars; equal-priority determinism (existing `font_role_precedes` tie-breakers) preserved.
    - Security: Priority is validated/bounded; invalid values rejected at package resolution like other styleMap fields.
  - Approach:
    - Documentation Reviewed:
      - `src/server/syntax.rs` (`captures_to_decoration_spans`, priority 70 constant).
      - `src/editor/surface.rs` (`font_role_precedes` tie-break chain).
      - `docs/reference/primitives/syntax-vocabulary.md` (styleMap contract).
    - Options Considered:
      - Hard-code per-token priorities in Rust: token-shape policy baked into server; not package-declarative. Rejected.
      - Optional `priority` field on styleMap entries with validation. Chosen.
    - Chosen Approach:
      - Extend the style-map entry schema (`{ type, modifiers?, fontRole?, priority? }`), thread through package record validation into `captures_to_decoration_spans`; set Markdown defaults: broad block captures (paragraph/heading) below inline captures (link/code-span/strong/emphasis).
    - API Notes and Examples:
      ```json
      { "link": { "type": "Link", "priority": 80 } }
      ```
    - Files to Create/Edit:
      - `src/server/syntax.rs`: priority field in style-map entry + span emission.
      - `src/packages/record.rs`: styleMap validation for the new field (follow existing validation patterns).
      - `docs/reference/primitives/syntax-vocabulary.md`: document the field.
    - References:
      - `tests/syntax_grammar.rs` existing styleMap promotion tests.
  - Test Cases to Write:
    - Overlapping broad/narrow captures resolve to the narrow token at the overlap.
    - Missing/invalid priority falls back to default or is rejected per schema; equal priorities keep existing deterministic order.
  - Completion Notes (2026-07-20):
    - Schema: `SyntaxStyleMapEntry` gained `priority: u16`; `DEFAULT_SYNTAX_STYLE_PRIORITY = 70` and `MAX_SYNTAX_STYLE_PRIORITY = 100` in `src/packages/record.rs`. Object-form entries accept optional `"priority"`; integers outside 0-100, floats, and strings are rejected at package resolution (`InvalidContributionDescriptor`); omitted keeps 70. Legacy string-token entries default to 70.
    - Promotion: `SyntaxVocabularySpan`/`map_capture_to_vocabulary`/`captures_to_decoration_spans` thread the entry priority into `DecorationSpan.priority` (the literal 70 is gone); `font_role_precedes` already compares priority first, so the normalization path is untouched.
    - Native maps: 5th tuple element added to both native style maps; markdown narrow captures (`code-span`, `strong`, `emphasis`, `link`) at 80, everything else 70. This fixes the link-color-suppressed-by-paragraph issue noted in the task-1 inventory.
    - Surface: `runtime/js/syntax.ts` styleMap object type documents `priority?: number`; `docs/reference/primitives/syntax-vocabulary.md` documents the field and resolution semantics; wiki `syntax-grammar-registry.md` updated.
    - Tests: `syntax_style_map_accepts_bounded_priority_and_rejects_invalid_values` (valid 80 parsed, default 70 preserved, 101/-1/1.5/"80" rejected); surface test `narrow_capture_priority_outranks_broad_prose_at_overlap` (broad-first emission order still resolves the overlap run to the Link color); markdown mixed-run test now asserts Paragraph=70 and CodeSpan/Link=80 span priorities. Equal-priority determinism covered by the unchanged existing normalization tests.
    - Validation: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (0 failures).

- [x] Batch decoration transport per parse update
  - Acceptance Criteria:
    - Functional: One parse update's 128-byte authority chunks ship in a single bounded `DecorationBatch` server message; client applies chunks atomically in key order; single-chunk `DecorationSet` wire behavior remains for compatibility or is migrated behind the same protocol version.
    - Performance: Fewer frames and fewer client event-queue dispatches per syntax update; 1 MiB frame ceiling still enforced; no full-document payloads.
    - Code Quality: Protocol semantics documented; rkyv archived validation extended to the batch variant; cache chunking unchanged (transport-only change).
    - Security: Batch payload bounded by frame ceiling; all archived input validated before access.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/decoration-transport.md` (chunk key/authority semantics).
      - `.agents/skills/project-patterns/references/protocol-and-performance.md` (deltas, bounded frames, no full-document IPC).
    - Options Considered:
      - Keep per-chunk messages: simpler, but dozens of frames+events per 4 KiB window was part of the disconnect amplification.
      - New `ServerMessage::DecorationBatch { chunks: Vec<...> }` validated as one frame. Chosen.
    - Chosen Approach:
      - Add the batch variant to the protocol enum; server syntax-update path accumulates the window's chunks and sends once; client applies the whole batch to `EditorDecorationState` in one pass; protocol version bump with the existing unsupported-version rejection path.
    - API Notes and Examples:
      ```rust
      ServerMessage::DecorationBatch { document_id, chunks: Vec<DecorationChunk> }
      ```
    - Files to Create/Edit:
      - `src/protocol/mod.rs` (+ decoration message module): batch variant + protocol version bump.
      - `src/server/connection.rs`: accumulate-and-send-once in the parse-update branch.
      - `src/client/mod.rs`: batch application path.
      - `src/editor/surface.rs` or decoration state: batch apply helper if needed.
    - References:
      - Commit `adc95b8` protocol-version rejection test as the versioning pattern.
  - Test Cases to Write:
    - One 4 KiB window update produces one client message and identical final decoration state vs per-chunk application.
    - Stale-version batch rejection matches single-set behavior; resync still clears all state.
    - Protocol v1 client rejection still enforced after version bump.
  - Completion Notes (2026-07-20):
    - Protocol (`src/protocol/mod.rs`): `PROTOCOL_VERSION` 4 → 5; new `ServerMessage::DecorationBatch(Vec<DecorationSet>)` — chunks self-describe (document/version/viewport per set), no separate chunk header type needed.
    - Server (`src/server/connection.rs`): parse-update branch ships multi-chunk updates as one `DecorationBatch` frame in viewport-key order; single-chunk updates keep the plain `DecorationSet` wire shape so existing single-chunk tests/clients see no change. Per-set validation before publication is unchanged, so the batch stays under the 1 MiB frame ceiling by construction (window budget × per-chunk budget).
    - Client (`src/client/mod.rs`): one `ClientConnectionEvent::DecorationBatch` per batch frame — one event-queue dispatch per parse update. Widget (`src/masonry_editor.rs`) applies every chunk in order through `apply_decoration_set` (no short-circuit; clippy's `any()` suggestion rejected because it would skip chunks after the first state change), so staleness rejection and plan058 exact-range replacement semantics are identical to sequential single sets.
    - Tests: `multi_chunk_parse_update_ships_as_single_decoration_batch` (server E2E: 512-byte rust file edit → one batch frame, ordered chunks, zero per-chunk frames); `decoration_batch_frame_dispatches_single_event` (client: one batch frame → one event, chunk order preserved); `decoration_batch_applies_chunks_atomically_and_rejects_stale_versions` (widget: fresh batch applies, stale-version batch rejected). Existing `stale_client_is_rejected_after_native_decoration_semantics_change` still rejects protocol v2 clients after the bump.
    - Wiki: `decoration-transport.md` steps 5-8 document the batch wire shape and invariants.
    - Validation: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (0 failures).

- [x] Rendered-output (visible style run) coverage for Markdown
  - Acceptance Criteria:
    - Functional: End-to-end tests assert visible text style runs (post-normalization), not only emitted token types: mixed inline, six heading levels, links/autolinks, fenced code, plain prose at base color, UTF-8 offsets, typing and scrolling through authority replacement.
    - Performance: Tests run in the existing fast suite (no GUI); no per-frame allocations added to production paths.
    - Code Quality: Tests use existing `normalize_visible_text_style_runs` seams; no test-only production branches.
    - Security: None new.
  - Approach:
    - Documentation Reviewed:
      - `docs/wiki/modules/decoration-transport.md` (Plan 058 exact-range replacement semantics).
      - `tests/syntax_grammar.rs`, `tests/decoration_transport.rs` fixtures.
    - Options Considered:
      - Extend existing token-emission tests only: misses the reported "everything one color" failure class. Rejected.
      - Add visible-run assertions at the surface seam. Chosen.
    - Chosen Approach:
      - Feed grammar-produced spans + default registry through the normalization seam and assert per-range font role/color/attributes.
    - API Notes and Examples:
      ```rust
      // normalize_visible_text_style_runs(spans, viewport) -> VisibleTextStyleRun assertions
      ```
    - Files to Create/Edit:
      - `tests/syntax_grammar.rs` or new `tests/markdown_rendered_styles.rs`: rendered-run assertions.
    - References:
      - `src/editor/surface.rs` normalization + tie-break functions.
  - Test Cases to Write:
    - All scenarios in Functional criteria above.
  - Completion Notes (2026-07-20):
    - Seam: new `EditorSurface::visible_text_style_runs_for_test()` (`src/editor/surface.rs`) returns normalized runs as `VisibleTextStyleRunForTest` tuples `(range, font_role, [bold, italic, underline, strike], color)` — the exact post-normalization presentation state, reachable from integration tests without touching private layout types (`TextAttributes` stays `pub(crate)`).
    - New `tests/markdown_rendered_styles.rs` builds the production handler (registry contribution + block query + `enable_injections`), renders through `EditorSurface`, and asserts against `StyleRegistry::clay_default()` expectations rather than hard-coded hex (palette stays free to evolve; hex locked in theme unit tests).
    - `markdown_visible_runs_style_all_constructs`: six heading levels (per-level color, bold, pairwise-distinct colors), plain prose at base color with no attributes, mixed inline run (bold/italic/code-span/link with correct attributes, CodeSpan color, Monospace role), both autolink kinds as Link, quote marker, fenced code block (CodeBlock color, Monospace), UTF-8 offsets (emoji before a strong run — no style shift).
    - `markdown_visible_runs_survive_typing_scrolling_and_authority_replacement`: scroll round-trip keeps runs stable; paste at document start + ack + version-2 authority re-parse replaces shifted chunks — heading/link/code-span re-assert at shifted offsets and the typed intro stays plain prose. Gotchas recorded: `note_confirmed_version` takes the document id (11, not 1); newline insertion needs `paste_text_with_event` (`insert_text` rejects control chars) plus an installed behavior manifest.
    - Validation: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (0 failures).

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Inventory of Rust public functions introduced/changed by this plan (injection executor entry points, styleMap priority validation, batch transport); each public programmatic capability either gets an explicit `deno_core` op + stable Clay JS/TS facade or is made `pub(crate)`/private; no raw `Deno.core.ops.op_*` as user API.
    - Performance: No JS round trip added to paint/typing hot paths.
    - Code Quality: Every Clay JS API doc has stable ID, user-facing name, key bindings (or empty list), custom properties, examples, errors, permissions, backing Rust path, op wrapper, JS facade path, lookup tags; linked from `docs/index.md`; registry regenerated.
    - Security: No configuration/API implicitly grants filesystem, network, shell, extension-loading, AI-mutation, or workspace authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `clay-js-api-naming.md`, `clay-js-api-schema.md`, `doc-registry-tests.md`.
    - Options Considered:
      - Expose injection/priority knobs as JS APIs: likely unnecessary if they are package-declarative only.
      - Keep new surfaces internal (`pub(crate)`) and document; expose only what users configure. Chosen unless inventory shows a real user-facing capability.
    - Chosen Approach:
      - Default to internal visibility; expose documented JS APIs only for genuinely user-facing capabilities found in the inventory.
    - API Notes and Examples:
      ```text
      cargo run --bin update-doc-registry
      ```
    - Files to Create/Edit:
      - TBD after inventory; `docs/index.md` + registry artifacts only if new APIs land.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`
  - Test Cases to Write:
    - Doc-registry coverage stays green; new API docs (if any) covered by `tests/primitives_docs.rs` or successor checks.
  - Completion Notes (2026-07-20):
    - Inventory-only verification; no new TS/JS code, ops, or docs required. All four checklist items confirmed already present in the runtime facades.
    - **(a) DecorationKind vocabulary**: `DecorationSpanInput.kind` (`runtime/js/decorations.ts`) accepts `"syntax" | "semantic" | "diagnostic" | "search-match"` — the four Rust `DecorationKind` variants. `"render"` does not exist yet; add it with the associated `DecorationKind::Render` variant when Phase 18.20 ships. TokenType + Modifiers input is string-based (server validates against the closed set at `src/server/ops/decorations.rs:130-157`). Legacy `styleToken` field preserved.
    - **(b) styleMap with priority**: `ServerRegisterSyntaxGrammarOptions.styleMap` (`runtime/js/syntax.ts`) object-form entries accept `priority?: number` (0-100, default 70, validated at `src/packages/record.rs` `MAX_SYNTAX_STYLE_PRIORITY`). Legacy string form defaults to 70. Per-span publishing via `DecorationSpanInput` also carries `priority?: number` (parsed at `src/server/ops/decorations.rs:130`).
    - **(c) injections contribution**: `ServerRegisterSyntaxGrammarOptions.queries?.injections` (`runtime/js/syntax.ts`) mirrors the `queries.injections` path in `SyntaxGrammarContribution` (`src/packages/record.rs:146-147`) for WASM/third-party grammars. First-party native grammars use embedded `NativeGrammarDescriptor.injections_query_path` (task 4); `packages/markdown/package.json` `queries.injections` provides documentation parity for third-party inspection. No new JS API needed.
    - **(d) batch decoration API**: Not applicable — `DecorationBatch` frames are an internal protocol optimization (v5). Packages publish individual spans through `serverPublishDecorations`; the server connection groups multi-chunk parse updates transparently. No JS surface change.
    - Existing TS types (`priority`, `injections`) added in tasks 4-5 match the Rust side. All build + test steps green (`cargo fmt`, `clippy`, `cargo test --all-targets`).

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Inventory behavior-changing settings introduced by this plan (e.g., any theme default change is a theme concern, not a config key; injection depth/byte budgets if user-tunable); each is a documented Clay JS API reachable from `~/.config/clay/init.js` or explicitly internal.
    - Performance: Configuration reads stay off hot paths.
    - Code Quality: Undocumented behavior-changing settings fail tests; config docs linked from `docs/index.md`.
    - Security: No config option grants new authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/configuration-system.md`, `planning-checklist.md`.
    - Options Considered:
      - Expose engine budgets as user config: premature; keep internal constants until measured need.
      - Keep budgets internal; document defaults in primitive docs. Chosen unless review finds a real user need.
    - Chosen Approach:
      - Internal constants with documented values; revisit as config only with evidence.
    - API Notes and Examples:
      ```js
      // ~/.config/clay/init.js remains the only config entry point
      ```
    - Files to Create/Edit:
      - TBD after inventory; likely docs-only.
  - Completion Notes (2026-07-20):
    - Inventory confirmed: **zero new configuration APIs needed**. All six completed primitives either stay internal or are already exposed through the existing config surface.
    - **IPC cancelation safety** (task 2): Internal protocol change. `DEFAULT_MAX_FRAME_SIZE` (1 MiB) stays at `src/protocol/codec.rs:18`. Not user-tunable — no evidence tunability helps.
    - **Default prose palette** (task 3): `StyleRegistry::clay_default()` compile-time constants. Users override via existing `setTheme(specifier)` API (`runtime/js/theme.ts`). No new config key.
    - **Injection engine** (task 4): Injection depth bounded by host-tree call-graph, not by a separate budget. Per-grammar `budgets.maxWindowBytes` already exists in `ServerRegisterSyntaxGrammarOptions`; injection reuses the host's parse window. No new budget knob.
    - **Capture priority** (task 5): `MAX_SYNTAX_STYLE_PRIORITY = 100` is a validation constant. Users supply per-capture `priority` in package `styleMap` entries (task 8 verified). Per-span priority is package-published, not user-configured.
    - **Batch decoration** (task 6): Protocol v5, internal optimization. Per-chunk 128-byte grid and `DECORATION_PAYLOAD_BUDGET_BYTES` are internal constants. No user-visible change.
    - **Rendered-output tests** (task 7): Test-only, zero config impact.
    - `~/.config/clay/init.js` surface stays unchanged: `setTheme`, `setTypography`, `loadPackage`, `setPackageOption` all unchanged by this plan.
    - `cargo test --all-targets` green; no undocumented behavior-changing setting flagged.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - Existing configuration coverage gates stay green.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Wiki updated after implementation: protocol-codec (pump architecture), syntax-grammar-registry (injection engine replacing predicate workaround), decoration-transport (batching), editor-theme-registry (default palette), first-party-markdown-package (inline styling path).
    - Performance: Wiki notes performance-relevant details (frame batching, layered parse caching, budgets).
    - Code Quality: Pages explain what changed, how it works, invariants/tradeoffs, source/test paths; master index links updated.
    - Security: Touched boundaries (frame validation, grammar trust) documented without secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`.
    - Options Considered:
      - Update after each task: noisy churn.
      - Update once after tests pass. Chosen.
    - Chosen Approach:
      - Single wiki pass after verification, including `docs/wiki/index.md` navigation.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`, `docs/wiki/modules/protocol-codec.md`, `docs/wiki/modules/syntax-grammar-registry.md`, `docs/wiki/modules/decoration-transport.md`, `docs/wiki/modules/editor-theme-registry.md`, `docs/wiki/modules/first-party-markdown-package.md`.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: index links resolve; updated pages match final implementation.
  - Completion Notes (2026-07-20):
    - Seven wiki/reference pages updated across the six implementation tasks; no new pages created (read-pump pattern fits cleanly into `protocol-codec.md` + `server-ipc-skeleton.md`).
    - **(Task 2 — cancelation-safe framing)**: `server-ipc-skeleton.md` — new `Cancellation-Safe Framing (Plan 059)` section documents the read-pump task + `ReadPumpGuard` + `mpsc` channel, server `tokio::io::split` with writer-keeps-a-name-for-zero-churn, and both regression tests. `protocol-codec.md` — version 5, DecorationBatch, pattern diagram, `ReadPumpGuard` invariant.
    - **(Task 3 — palette)**: `editor-theme-registry.md` — prose-palette paragraph revised (already done).
    - **(Task 4 — injections)**: `syntax-grammar-registry.md` — predicate workaround replaced with injection executor description. `first-party-markdown-package.md` — composite block+inline parsing path documented.
    - **(Task 5 — priority)**: `syntax-grammar-registry.md` — one-sentence priority addition. `docs/reference/primitives/syntax-vocabulary.md` — priority field documented with semantics.
    - **(Task 6 — batch)**: `decoration-transport.md` — steps 5-8 updated with batch wire shape (already done). `protocol-codec.md` — version 5 and DecorationBatch variant noted.
    - All pages verified to match final shipped code. No stale references, dead paths, or incorrect invariants. `docs/wiki/index.md` module entries remain accurate (short descriptions don't enumerate per-plan specifics — each linked page is self-contained).

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.
