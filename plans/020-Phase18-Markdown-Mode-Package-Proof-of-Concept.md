# Phase 18: Markdown Mode Package Proof of Concept — markdown-it Rewrite

## Objectives

- Fully remove the `mdast-util-from-markdown` implementation from the first-party Markdown package and replace it with a rewritten `markdown-it` token-stream adapter.
- Prove that Markdown mode remains a JavaScript package implementation built only on generic Clay editor primitives, with no Markdown-specific Rust server/client logic.
- Enrich Clay's reusable primitive library only where necessary, keeping every new Rust-side primitive generic enough for future modes such as Python, Org, AsciiDoc, or other language packages.
- Preserve Clay's authority and latency boundaries: server-side JavaScript execution only, inert declarations to the client, no client-side package JavaScript, no raw ops as public APIs, no full-document IPC for ordinary edits, and no synchronous JavaScript/IPC in typing/paint/scroll/text-event handlers.
- Document and test the primitive inventory so future JS package authors and AI agents can discover existing primitives before proposing new ones.

## Expected Outcome

- `@clay/markdown` depends on `markdown-it`, not `mdast-util-from-markdown`, and its parser/decorator adapter is a full token-stream rewrite rather than a patch to the mdast adapter.
- The Markdown package produces validated Clay `DecorationSpan` data for headings, strong/emphasis, inline code, fenced code blocks, and ordered/unordered list markers by traversing markdown-it tokens and package-owned source/line indexes.
- Rust server/client code exposes only generic primitives: package loading, mode activation, commands/key routing, inert editor behavior rules, parse scheduling, decoration validation/transport/rendering, SDUI, configuration, and documentation coverage. Rust does not branch on Markdown syntax, markdown-it token names, or package-specific parser details.
- The plan includes explicit cleanup work for mdast dependencies, adapter code, tests, docs, benchmark wording, and stale decision references.
- Primitive docs, wiki pages, index navigation, and deterministic tests make every new or changed primitive discoverable and keep future mode-package work primitive-first.

## Tasks

- [x] Reconfirm Phase 18 rewrite scope, decision sources, and parser evidence
  - Acceptance Criteria:
    - Functional: The Phase 18 plan is treated as a markdown-it rewrite plan; old mdast implementation notes are either removed, moved to historical references, or explicitly marked superseded.
    - Performance: The rewrite scope cites large-file benchmark evidence showing why the mdast path is removed and why markdown-it is the selected parser candidate.
    - Code Quality: The plan cites the approved decision log and relevant project patterns before implementation tasks begin.
    - Security: Scope confirmation preserves the no-client-JavaScript, no raw-op, no filesystem/network/shell/AI/WASM, and server-validated inert-data package boundaries.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`: Approved parser replacement and primitive-first planning decision.
      - `decision-logs/2026-06-03-2306-start-markdown-poc-with-mdast-util-from-markdown.md`: Superseded mdast start decision.
      - `docs/development/performance.md`: Actual parser benchmark evidence using existing repository Markdown files.
      - `.agents/skills/project-patterns/references/markdown-parser-adapters.md`: Current parser-adapter rule: markdown-it replaces mdast.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`: Primitive-first planning rule for mode packages.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: No full-document IPC for ordinary edits and no hot-path JavaScript/IPC.
    - Options Considered:
      - Keep mdast for small documents and add markdown-it for large documents: rejected; dual parser paths preserve deprecated code and complicate correctness/security tests.
      - Treat markdown-it as a follow-up spike only: rejected; benchmark evidence and user direction require a full replacement now.
      - Rewrite Phase 18 around markdown-it and primitive-first package work: selected.
    - Chosen Approach:
      - Start the rewrite by making the plan, decision log, and project patterns agree that markdown-it is the active implementation target and mdast is cleanup scope.
    - API Notes and Examples:
      ```text
      cargo test --test performance_budgets
      node --check tools/bench/markdown-parser.mjs
      node tools/bench/markdown-parser.mjs --dry-run --sizes 1MiB --source-limit 8
      ```
    - Files to Create/Edit:
      - `plans/020-Phase18-Markdown-Mode-Package-Proof-of-Concept.md`: Rewrite around markdown-it.
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`: Decision source.
      - `.agents/skills/project-patterns/references/markdown-parser-adapters.md`: Current parser-adapter guidance.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`: Current primitive-first guidance.
    - References:
      - Context7 `/markdown-it/markdown-it` docs: token stream architecture and Token object fields.
      - Context7 CLI confirmation on 2026-06-04: `npx ctx7@latest library markdown-it "Complete Phase 18 Markdown mode package rewrite: confirm markdown-it token stream parse API, Token fields, inline children, and parse not render HTML"` selected `/markdown-it/markdown-it` (High reputation, 1089 snippets); `MSYS_NO_PATHCONV=1 npx ctx7@latest docs /markdown-it/markdown-it "markdown-it JavaScript parse token stream API Token fields children map markup content block hidden parse versus render HTML"` confirmed block-token streams, inline `children`, Token fields, and default `html: false` parser options.
      - `tools/bench/markdown-parser.mjs`
  - Test Cases to Write:
    - `markdown_plan_references_markdown_it_rewrite_decision`: Plan/docs reference the current decision log and do not describe mdast as the active parser.
    - `markdown_performance_docs_record_parser_replacement_reason`: Performance docs retain benchmark evidence for the parser switch.
  - Verification Completed:
    - Re-read the approved parser replacement decision, superseded mdast start decision, performance evidence, Clay plan requirements, and project patterns before implementation tasks begin.
    - Confirmed this plan states markdown-it as the active Phase 18 implementation target, treats mdast as superseded cleanup/historical rationale only, preserves primitive-first Rust boundaries, and keeps no-client-JavaScript/no-raw-op/no-hot-path IPC security constraints in scope.
    - `rg "mdast|fromMarkdown|mdast-util-from-markdown|markdown-it" ...` confirmed remaining active mdast references are in package/tests/docs/tooling slated for the explicit cleanup task, while this plan and current decision/pattern sources select markdown-it.
    - Added deterministic guards in `tests/performance_budgets.rs`: `markdown_plan_references_markdown_it_rewrite_decision` and `markdown_performance_docs_record_parser_replacement_reason`.
    - `node --check tools/bench/markdown-parser.mjs`: passed.
    - `node tools/bench/markdown-parser.mjs --dry-run --sizes 1MiB --source-limit 8`: passed and confirmed repository-Markdown corpus coverage without importing parsers or mutating fixtures.
    - `cargo test --test performance_budgets`: passed (9 tests).

- [x] Review existing editor primitives and plan generic primitive gaps before package work
  - Acceptance Criteria:
    - Functional: Before changing `@clay/markdown`, inventory existing primitives and state what Markdown can achieve with them: document classification, major-mode activation, command/key routing, inert text transforms, parse handlers, decoration publication/rendering, SDUI, configuration, package permissions, and docs/registry coverage.
    - Performance: The review identifies which primitives run at load/open/reload/configuration/background time and confirms none require package JavaScript or IPC in typing/paint/scroll/text-event handlers.
    - Code Quality: Any new Rust-side work proposed by this plan is named and shaped as a reusable primitive, not as a Markdown-specific type, branch, parser, renderer, or style map.
    - Security: The review records required permissions and validation boundaries for each primitive used by the Markdown package.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, `docs/reference/primitives/rendering-strategy.md`, and `docs/reference/primitives/parse-update-strategy.md`: Authoritative primitive design and registry.
      - `docs/wiki/modules/primitive-architecture.md`, `docs/wiki/modules/mode-registry.md`, `docs/wiki/modules/parse-coordinator.md`, `docs/wiki/modules/decoration-transport.md`, `docs/wiki/modules/rendering-primitives.md`, and `docs/wiki/modules/first-party-markdown-package.md`: Implementation wiki for current primitives.
      - `.agents/skills/create-plan/references/clay.md`: Required primitive-first phase task.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`: Reusable project pattern.
    - Options Considered:
      - Implement markdown-it first and document primitive gaps later: rejected; risks adding Markdown-specific Rust shortcuts.
      - Require a primitive inventory first, then implement only generic gaps: selected.
    - Chosen Approach:
      - Add an explicit primitive review artifact in docs/wiki or plan notes. If gaps are found, create/update generic primitive docs and tests before package-specific implementation depends on them.
    - API Notes and Examples:
      ```text
      cargo test --test primitives_docs
      cargo test --test package_primitive_gate
      cargo test --test decoration_transport
      cargo test --test parse_coordinator
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/phase18-markdown-primitive-review.md`: Added the primitive-first review artifact for the markdown-it rewrite, including existing primitive inventory, hot-path/security boundaries, and generic gaps.
      - `docs/wiki/index.md`: Linked the new primitive review artifact from the master wiki index.
      - `docs/wiki/modules/primitive-architecture.md`: Linked the review from the primitive architecture related pages.
      - `tests/primitives_docs.rs`: Added deterministic coverage for the review artifact, wiki index link, required inventory categories, hot-path/security text, and generic-only gap guidance.
      - `docs/wiki/modules/rendering-primitives.md`, `docs/wiki/modules/parse-coordinator.md`, `docs/wiki/modules/decoration-transport.md`, `docs/wiki/modules/mode-registry.md`: No change; existing primitive contracts were reviewed but not changed.
      - `docs/reference/primitives/**`: No change; no new or changed primitive contract was required before package work.
    - References:
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - `phase18_markdown_primitive_review_records_existing_inventory`: Review artifact and wiki index record the primitive inventory, timing/hot-path classification, package permissions, validation boundaries, and documentation coverage.
    - `phase18_markdown_primitive_review_records_generic_gaps_only`: Review artifact records only reusable primitive gaps and explicitly rejects Markdown-specific Rust parser/rendering/token branches.
    - Existing focused coverage retained: `cargo test --test package_primitive_gate`, `cargo test --test decoration_transport`, and `cargo test --test parse_coordinator` verify the current generic primitives used by Markdown.
  - Verification Completed:
    - Re-read the approved markdown-it/primitive-first decision, Clay primitive-first plan requirements, project patterns, primitive reference docs, and implementation wiki pages before package work.
    - Inventoried existing generic primitives for package validation/permissions, document classification, major-mode activation, command/key routing, inert text transforms, parse registration/scheduling, decoration validation/transport/rendering, SDUI, configuration, and docs/registry coverage.
    - Confirmed the markdown-it parser adapter can proceed inside `@clay/markdown` using existing generic parse/decorations primitives and package-owned source/line indexing; no Markdown-specific Rust parser, token mapper, renderer, style map, or mode branch is required before package work.
    - Recorded generic follow-up gaps only: complete reusable list/fence transform engines if needed, add a language-neutral parse-input/range-snapshot/line-index primitive only if runtime handler execution needs it, remove mode-specific fallback defaults from generic ops when touched, and defer style-token expansion to a generic decoration/theme registry.
    - Added `docs/wiki/modules/phase18-markdown-primitive-review.md`, linked it from `docs/wiki/index.md` and `docs/wiki/modules/primitive-architecture.md`, and added deterministic coverage in `tests/primitives_docs.rs`.
    - `cargo fmt --check`: passed.
    - `cargo test --test primitives_docs`: passed (30 tests).
    - `cargo test --test package_primitive_gate`: passed (11 tests).
    - `cargo test --test decoration_transport`: passed (7 tests).
    - `cargo test --test parse_coordinator`: passed (7 tests).

- [x] Clean up mdast implementation, dependencies, tests, docs, and stale references
  - Acceptance Criteria:
    - Functional: `@clay/markdown` no longer imports, depends on, dynamically loads, tests, documents, or benchmarks `mdast-util-from-markdown` as an active implementation path.
    - Performance: Cleanup removes the known slow mdast adapter from benchmark defaults and prevents accidental reintroduction as the active parser.
    - Code Quality: Removed code leaves no dead exports, stale fixtures, stale package metadata, or tests asserting mdast-specific adapter behavior.
    - Security: Cleanup does not broaden package permissions or introduce new install-time scripts, network fetches, shell hooks, raw ops, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `packages/markdown/package.json`: Current parser dependency metadata.
      - `packages/markdown/dist/parser.js` and `packages/markdown/src/parser.js`: Current mdast adapter/export boundary to remove/rewrite.
      - `tests/markdown_mode.rs`, `tests/performance_budgets.rs`, and `tools/bench/markdown-parser.mjs`: Tests/benchmarks that mention mdast.
      - `docs/development/performance.md`, `docs/wiki/modules/first-party-markdown-package.md`, and `docs/wiki/modules/performance-fixtures.md`: Docs that currently record mdast results/recommendations.
    - Options Considered:
      - Leave mdast as an optional benchmark parser only: acceptable only if clearly isolated as historical/comparison tooling and not a package dependency; rejected for this cleanup pass to prevent accidental active-path reintroduction.
      - Remove all mdast references including benchmark history: rejected; benchmark evidence should remain as rationale, but active implementation docs must say markdown-it.
      - Remove active mdast implementation while keeping historical benchmark evidence: selected.
    - Chosen Approach:
      - Replaced package dependency/code/tests with markdown-it and updated docs so mdast appears only in historical decision/performance rationale, superseded decision logs, plan cleanup context, or guard-test strings.
    - API Notes and Examples:
      ```text
      rg "mdast|fromMarkdown|mdast-util-from-markdown" packages tests docs tools plans decision-logs
      npm install --prefix packages/markdown --no-save --no-package-lock --ignore-scripts markdown-it@^14.1.0
      ```
    - Files to Create/Edit:
      - `packages/markdown/package.json`: Remove `mdast-util-from-markdown`; add/retain `markdown-it` dependency for the active package.
      - `packages/markdown/dist/parser.js` and `packages/markdown/src/parser.js`: Delete mdast adapter code and replace with markdown-it adapter implementation.
      - `tests/markdown_mode.rs`: Remove mdast-specific tests and add markdown-it tests.
      - `tests/performance_budgets.rs`: Update parser benchmark/documentation expectations.
      - `tools/bench/markdown-parser.mjs`: Made markdown-it and the active adapter the default and only benchmark parser paths; mdast timings remain only in docs as historical replacement rationale.
      - `src/server/js_runtime.rs`: Updated the parser adapter runtime test fixture to inject markdown-it-shaped tokens instead of mdast trees/fromMarkdown.
      - `benches/markdown_baselines.rs`: Removed mdast-specific parse-delta wording from representative benchmark payloads.
      - `docs/development/performance.md`, `docs/wiki/modules/first-party-markdown-package.md`, `docs/wiki/modules/performance-fixtures.md`, `docs/wiki/modules/markdown-mode-activation.md`, `docs/wiki/modules/parse-coordinator.md`, `docs/wiki/index.md`, `docs/reference/packages/markdown.md`, `packages/markdown/docs/index.md`, and `plans/020-Phase18-Markdown-Mode-Package-Proof-of-Concept.md`: Updated active parser wording.
    - References:
      - `decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md`
      - `.agents/skills/project-patterns/references/markdown-parser-adapters.md`
  - Test Cases to Write:
    - `markdown_package_has_no_mdast_dependency`: Package metadata does not include `mdast-util-from-markdown`.
    - `markdown_runtime_code_has_no_from_markdown_import`: Active package runtime/source files do not import or dynamically load `fromMarkdown`.
    - `markdown_docs_do_not_describe_mdast_as_active_parser`: Docs may cite mdast only as historical benchmark rationale.
  - Verification Completed:
    - Replaced `@clay/markdown` metadata dependency `mdast-util-from-markdown` with `markdown-it` without changing package permissions or adding install-time scripts.
    - Rewrote `packages/markdown/dist/parser.js` around markdown-it token input/default parsing, safe parser options (`html: false`, `linkify: false`, `typographer: false`), package-owned source/line indexing, generic Clay syntax spans, and inert decoration publication.
    - Updated benchmark tooling so `tools/bench/markdown-parser.mjs` defaults to and supports only `markdown-it` plus the active package adapter; historical mdast numbers remain in documentation only as replacement rationale.
    - Removed mdast/fromMarkdown active-path expectations from package/runtime/performance tests, runtime parser fixtures, benchmark payload wording, package docs, reference docs, and implementation wiki pages.
    - `rg "mdast|fromMarkdown|mdast-util-from-markdown" . -g '!target' -g '!node_modules'` now finds mdast only in superseded/current decision logs, Phase 18 plan context, historical performance rationale, primitive-review rationale, and guard tests.
    - `node --check packages/markdown/dist/parser.js`: passed.
    - `node --check tools/bench/markdown-parser.mjs`: passed.
    - `node tools/bench/markdown-parser.mjs --dry-run --sizes 1MiB --source-limit 8`: passed.
    - `cargo fmt --check`: passed.
    - `cargo test --test markdown_mode`: passed (29 tests).
    - `cargo test --test performance_budgets`: passed (9 tests).
    - `cargo test markdown_parser_adapter_publishes_viewport_bounded_decorations --lib`: passed.
    - `cargo test --test package_loading`: passed (19 tests).

- [x] Add or verify generic primitives needed by token-stream package adapters
  - Acceptance Criteria:
    - Functional: Any Rust-side changes needed by the markdown-it package are generic primitives usable by multiple modes, such as parse request metadata, viewport/changed-range delivery, line/byte range metadata, decoration publication, style-token validation, inert behavior rules, or primitive documentation coverage.
    - Performance: New primitives avoid repeated full-document scans in Rust hot paths, preserve viewport-prioritized parse/decorations, fit existing payload budgets, and remain outside keypress/paint/scroll/text-event handlers.
    - Code Quality: Primitive names and schemas are language-neutral; docs explain how non-Markdown packages such as Python mode could reuse them.
    - Security: New primitives validate provenance, permissions, ranges, versions, payload size, and inert data before client delivery.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/registry.md`: Existing primitive categories and schema.
      - `docs/reference/primitives/parse-update-strategy.md`: Parse notification/result contract.
      - `docs/reference/primitives/rendering-strategy.md`: Decoration and rendering contract.
      - `docs/wiki/modules/phase18-markdown-primitive-review.md`, `docs/wiki/modules/parse-coordinator.md`, and `docs/wiki/modules/decoration-transport.md`: Current primitive implementation inventory and token-stream adapter boundaries.
      - Context7 `/markdown-it/markdown-it` docs fetched on 2026-06-04: markdown-it uses token streams, block tokens, inline child tokens, and Token fields including `map`, `markup`, `content`, `nesting`, `block`, and `hidden`.
    - Options Considered:
      - Add Rust helpers named for Markdown markers/fences/headings: rejected; violates package boundary and future-mode reuse.
      - Compute token-to-byte ranges entirely in the JS package using text and line indexes: selected for the markdown-it adapter; keeps Rust parser primitives generic.
      - Add a generic parse-request line-index primitive now: rejected for this task; no current benchmark or correctness gap requires Rust-provided line starts.
      - Expand decoration style validation with a small language-neutral token allowlist: selected so non-Markdown packages can publish syntax spans without a Markdown-specific style map.
    - Chosen Approach:
      - Verified the existing generic parse metadata is sufficient for token-stream adapters to receive document/version/provenance, viewport, and invalidated byte ranges without exposing markdown-it tokens to Rust.
      - Added only generic decoration validation support for common code syntax tokens and removed a touched generic op fallback that implicitly defaulted package metadata to Markdown.
    - API Notes and Examples:
      ```ts
      // Generic package parse input shape, not Markdown-specific.
      type ParseRequest = {
        documentId: number;
        documentVersion: number;
        behaviorVersion: number;
        packagePrefix: string;
        mode: string;
        viewport: { byteStart: number; byteEnd: number };
        invalidatedRanges: Array<{ byteStart: number; byteEnd: number }>;
      };
      ```
    - Files to Create/Edit:
      - `src/server/decorations.rs`: Added language-neutral syntax style tokens (`keyword.control`, `string.quoted`, `comment.line`, `punctuation.definition`) to the inert decoration allowlist.
      - `src/server/ops/decorations.rs`: Removed the touched Markdown-specific fallback by deriving default package mode metadata from the package API prefix when no explicit mode is provided.
      - `tests/parse_coordinator.rs`: Added generic token-stream parse metadata coverage and a Rust source guard against Markdown/markdown-it parser branches.
      - `tests/decoration_transport.rs`: Added non-Markdown (`@clay/python`) decoration publication coverage for generic syntax spans.
      - `tests/primitives_docs.rs`: Extended primitive-review documentation coverage for token-stream verification.
      - `docs/reference/primitives/rendering-strategy.md` and `docs/reference/clay-js-api/decorations/server-publish-decorations.md`: Documented generic style-token validation.
      - `docs/wiki/modules/phase18-markdown-primitive-review.md`, `docs/wiki/modules/parse-coordinator.md`, and `docs/wiki/modules/decoration-transport.md`: Recorded token-stream primitive verification, non-Markdown reuse, hot-path/security boundaries, and tests.
      - `src/protocol/parse.rs`, `src/server/parse_coordinator.rs`, `src/server/ops/parse.rs`, `runtime/js/parse.ts`, `src/protocol/decorations.rs`, `src/editor/surface.rs`, and `src/editor/layout.rs`: Reviewed; no contract change required.
    - References:
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - `generic_parse_request_metadata_supports_token_stream_adapters`: Parse inputs expose enough generic metadata for package-owned token/range mapping without Markdown-specific Rust.
    - `generic_decoration_publication_accepts_language_package_spans`: Validation accepts package-prefixed syntax spans without knowing the package language.
    - `rust_code_has_no_markdown_specific_parser_branch`: Rust source search/test rejects new parser branches named for Markdown syntax or markdown-it tokens.
  - Verification Completed:
    - Verified markdown-it token-stream requirements against Context7 docs and confirmed token-to-byte range recovery remains package-owned JavaScript work.
    - Verified parse primitives stay package-neutral: Python-mode test coverage receives document/version/behavior/package/mode metadata, viewport byte range, and viewport-prioritized invalidated ranges without Markdown-specific Rust fields.
    - Added generic decoration style-token validation for non-Markdown syntax spans and proved `@clay/python` can publish inert `keyword.control` and `string.quoted` spans through the same `DecorationRange` primitive.
    - Removed the touched `unwrap_or("markdown")` fallback in generic decoration publication metadata; default mode metadata now derives from the package API prefix when an explicit mode is absent.
    - Added a Rust source guard against markdown-it token/parser branch markers in parse/decorations/editor/client production paths.
    - `cargo fmt --check`: passed.
    - `cargo test --test parse_coordinator`: passed (9 tests).
    - `cargo test --test decoration_transport`: passed (8 tests).
    - `cargo test --test primitives_docs`: passed (30 tests).
    - `cargo test --test package_primitive_gate`: passed (11 tests).
    - `cargo test --test markdown_mode`: passed (29 tests).
    - `cargo test phase18_parse_and_decoration_facades_are_runtime_backed --lib`: passed.

- [x] Rewrite the Markdown parser/decorator adapter around markdown-it tokens
  - Acceptance Criteria:
    - Functional: The `@clay/markdown` adapter uses `markdown-it` parsing/token APIs to emit Clay decoration spans for ATX heading markers/content, strong/emphasis, inline code, fenced code blocks, and ordered/unordered list markers.
    - Performance: The adapter uses package-owned line/source indexes to avoid repeated start-of-document scans, filters spans to the viewport before publication, and remains suitable for 1 MiB, 5 MiB, and 16 MiB repository-Markdown benchmark corpora.
    - Code Quality: Adapter code is a clean token-stream implementation with no mdast compatibility layer, no AST assumptions, and no parser-specific data escaping into Clay protocol shapes.
    - Security: The package uses markdown-it for parsing tokens only; it does not render HTML, does not enable arbitrary raw HTML/script output, and publishes only inert validated decoration data.
  - Approach:
    - Documentation Reviewed:
      - Context7 `/markdown-it/markdown-it` docs: token stream architecture; inline container `children`; Token fields `type`, `tag`, `attrs`, `map`, `nesting`, `level`, `children`, `content`, `markup`, `info`, `meta`, `block`, and `hidden`.
      - `docs/reference/primitives/rendering-strategy.md`: Required decoration span shape and validation.
      - `docs/reference/primitives/parse-update-strategy.md`: Background parse/update model.
      - `.agents/skills/project-patterns/references/markdown-parser-adapters.md`: markdown-it adapter boundary rules.
    - Options Considered:
      - Use `markdown-it.render(...)` and infer ranges from HTML: rejected; HTML output is unnecessary, lossy for source ranges, and increases security risk.
      - Use `markdown-it.parse(...)`/token streams and package-owned source scanning for exact byte ranges: selected.
      - Add Rust token post-processing for range recovery: rejected unless generalized as a package-neutral primitive.
    - Chosen Approach:
      - Instantiate markdown-it with safe parser options, parse to token streams, traverse block and inline child tokens, derive exact byte ranges from source text/line maps/markup/content, convert UTF-16 offsets to UTF-8 byte offsets in JS, and publish viewport-bounded Clay decoration spans.
    - API Notes and Examples:
      ```js
      import MarkdownIt from "markdown-it";

      const md = new MarkdownIt({ html: false, linkify: false, typographer: false });
      const tokens = md.parse(markdownText, {});

      for (const token of tokens) {
        if (token.type === "inline" && token.children) {
          for (const child of token.children) {
            // Convert token/markup/source positions to Clay DecorationSpan ranges.
          }
        }
      }
      ```
    - Files to Create/Edit:
      - `packages/markdown/dist/parser.js`: Rewritten markdown-it token-stream adapter with safe parser options, source/line index, inline child traversal, UTF-8 byte conversion, viewport filtering, and inert decoration update/publication helpers.
      - `packages/markdown/src/parser.js`: Source re-export remains aligned with the package convention.
      - `packages/markdown/package.json`: Active markdown-it dependency/export metadata verified unchanged.
      - `tests/fixtures/markdown/token-ranges.md`: Added token/range fixture for headings, UTF-8, inline markup, fences, and ordered list markers.
      - `tests/fixtures/markdown/inline-nesting.md`: Added nested inline child fixture.
      - `tests/markdown_mode.rs`: Added fixture/static adapter coverage for source indexes, token-stream traversal, markdown-it parse usage, safe options, and no HTML rendering.
      - `src/server/js_runtime.rs`: Extended runtime-backed parser adapter coverage for exact UTF-8 byte ranges, inline child traversal, marker/fence ranges, viewport filtering, facade publication, and parse-not-render behavior with an injected markdown-it-shaped parser.
      - `docs/wiki/modules/first-party-markdown-package.md` and `docs/wiki/modules/markdown-mode-activation.md`: Updated implementation wiki notes for the token-stream adapter and tests.
    - References:
      - Context7 `/markdown-it/markdown-it` docs.
      - `docs/wiki/modules/first-party-markdown-package.md`
  - Test Cases to Write:
    - `markdown_it_adapter_emits_required_span_kinds`: Required syntax spans are emitted from markdown-it tokens.
    - `markdown_it_adapter_maps_utf8_byte_ranges_exactly`: Multibyte Unicode fixtures map to valid Clay byte ranges.
    - `markdown_it_adapter_handles_inline_children`: Strong/emphasis/code spans nested under inline tokens are found without mdast nodes.
    - `markdown_it_adapter_derives_marker_ranges`: Heading/list/fence marker ranges are exact and viewport filtered.
    - `markdown_it_adapter_does_not_render_html`: Tests assert the adapter uses token parsing and publishes inert spans, not HTML output.
  - Verification Completed:
    - Re-read Context7 `/markdown-it/markdown-it` docs via the required CLI flow: `npx ctx7@latest library markdown-it ...` selected `/markdown-it/markdown-it`, and `npx ctx7@latest docs /markdown-it/markdown-it ...` confirmed safe options, token streams, inline `children`, token fields, and parse-vs-render boundaries.
    - Rewrote `packages/markdown/dist/parser.js` as a package-owned markdown-it token-stream adapter. The adapter uses `markdownIt.parse(text, {})` with `html: false`, `linkify: false`, and `typographer: false`, never calls `render`, traverses block tokens plus inline child tokens, and publishes only Clay `DecorationSpan` fields.
    - Added package-owned source/line indexing and UTF-16-to-UTF-8 conversion so ATX heading spans, strong/emphasis spans, inline code spans, fenced code block spans, and ordered/unordered list marker spans use exact Clay byte ranges without Rust parser/token helpers.
    - Added viewport filtering before publication so spans outside the requested viewport are not sent to the generic decoration facade.
    - Added `tests/fixtures/markdown/token-ranges.md` and `tests/fixtures/markdown/inline-nesting.md` to cover headings, UTF-8, inline nesting, inline code, fences, and list markers.
    - Updated runtime-backed tests to assert required span kinds, exact UTF-8 byte ranges for `# Hé 🦀`, exact inline/fence/list ranges, viewport filtering, decoration facade publication, and parse-not-render behavior with an injected markdown-it-shaped parser whose `render()` would fail the test.
    - Updated code wiki pages for the parser adapter implementation details and test coverage.
    - `node --check packages/markdown/dist/parser.js`: passed.
    - `cargo fmt --check`: passed.
    - `cargo test --test markdown_mode`: passed (30 tests).
    - `cargo test markdown_parser_adapter_publishes_viewport_bounded_decorations --lib`: passed.

- [x] Reintegrate the markdown-it package runtime, activation workflow, SDUI, and fallback behavior
  - Acceptance Criteria:
    - Functional: Markdown documents still classify/activate by `.md`, `.markdown`, `.mdown`, and `text/markdown`; package-owned commands, key bindings, inert editor rules, parse/decorations registration, and SDUI preview/status continue to work after the parser rewrite.
    - Performance: Package activation/load validation remains load/open/reload/configuration-time work; parse/decorations remain background and viewport-prioritized; typing and local paint do not wait for markdown-it.
    - Code Quality: Runtime integration uses generic Clay JS facades and package metadata; no Rust package-loading, mode-registry, parse-coordinator, client-rendering, or SDUI code special-cases Markdown.
    - Security: Package permissions remain minimal (`mode-registration`, `mode-activation`, `command-registration`, `parse-document`, `render-decorations`, plus configuration only if needed); disabled/invalid packages lose command/keybinding/decoration/SDUI authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/package-security.md`: Package authority and validation.
      - `docs/reference/primitives/markdown-mode-requirements.md`: User-visible Markdown POC behavior.
      - `docs/wiki/modules/markdown-mode-activation.md`, `docs/wiki/modules/package-loading.md`, and `docs/wiki/modules/first-party-markdown-package.md`: Current package runtime flow.
    - Options Considered:
      - Limit replacement to parser tests only: rejected; package activation/smoke must prove the real package path.
      - Revalidate the whole Markdown workflow on markdown-it: selected.
    - Chosen Approach:
      - Update package load/runtime code to import the markdown-it adapter and re-run end-to-end package activation, parsing, decoration, SDUI, fallback, and smoke tests.
    - API Notes and Examples:
      ```text
      cargo test --test markdown_mode
      cargo test --test package_loading
      cargo run -- smoke-gui --config-fixture markdown-mode
      ```
    - Files to Create/Edit:
      - `packages/markdown/dist/index.js`: Added shared `markdownPackageManifest()` runtime metadata so the package loader, parser, SDUI, and tests use one manifest shape.
      - `packages/markdown/dist/load.js`: Wired `loadMarkdownPackage(clay, options)` to the actual Clay facade signatures for package validation, mode-pattern registration, major-mode activation, command registration, and parse-handler registration with the markdown-it parser adapter path.
      - `packages/markdown/dist/sdui.js`: Updated default parse/decorations status to report `markdown-it registered`/`published` through inert SDUI labels.
      - `packages/markdown/src/index.js`, `packages/markdown/src/load.js`, `packages/markdown/src/sdui.js`, and `packages/markdown/src/parser.js`: Verified source stubs re-export the updated runtime files without additional code.
      - `src/server/js_runtime.rs`: Added runtime-backed coverage that imports the real Markdown package modules, loads the package through Clay facades, publishes injected markdown-it-token decorations, and publishes SDUI.
      - `tests/markdown_mode.rs`: Updated runtime/fallback/SDUI expectations and static guards for real Clay facade signatures.
      - `tests/performance_budgets.rs`: Updated performance documentation guard for the renamed non-blocking markdown-it typing test.
      - `tests/fixtures/configuration/markdown-mode/init.js`: Kept smoke fixture status text aligned with the markdown-it parse state.
      - `docs/development/launch-and-gui-smoke.md`, `docs/development/performance.md`, `docs/wiki/modules/first-party-markdown-package.md`, and `docs/wiki/modules/markdown-mode-activation.md`: Updated manual smoke notes, performance test-name references, and implementation wiki coverage.
    - References:
      - `.agents/skills/project-patterns/references/package-distribution.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - `markdown_fixture_activates_with_markdown_it_adapter`: Fixture activation registers parse/decorations through the markdown-it adapter.
    - `markdown_disabled_falls_back_to_plain_text_after_rewrite`: Disabled package removes Markdown contributions safely.
    - `markdown_sdui_status_reports_markdown_it_parse_state`: Structural SDUI observes parse/decorations status without executable client hooks.
    - `markdown_typing_does_not_wait_for_markdown_it_parse`: Slow parser simulation does not block edit acknowledgement/local application.
    - `server::js_runtime::tests::markdown_package_runtime_loads_markdown_it_workflow`: Real package runtime modules validate/load the manifest, activate Markdown, register commands/parse, publish injected markdown-it-token decorations, and publish inert SDUI through Clay facades.
  - Verification Completed:
    - Re-read package security requirements, Markdown POC requirements, package-loading/Markdown wiki pages, approved markdown-it decision, and package-distribution/authority/performance patterns before changing runtime wiring.
    - `packages/markdown/dist/load.js` now uses the actual facade signatures (`serverLoadPackage(packageManifest)`, `serverRegisterModePattern(packageManifest, ...)`, `serverActivateMajorMode(packageManifest, ...)`, `serverRegisterCommand(packageManifest, ...)`, and `serverRegisterParseHandler({ packageManifest, ... })`) instead of stale single-object stubs.
    - `packages/markdown/dist/index.js` now exports `markdownPackageManifest()` so runtime loading and tests share the first-party package manifest, permissions, contribution metadata, parser adapter path, and SDUI adapter path.
    - `packages/markdown/dist/sdui.js` now reports `Parse: markdown-it registered` and `Decorations: published` through inert labels; package SDUI still targets only registered `markdown.*` commands.
    - Added runtime-backed test coverage that imports the real package modules into the controlled server runtime, activates Markdown for `sample.md`, registers commands and parse metadata, publishes markdown-it-token-derived decorations with the package adapter, publishes SDUI, and verifies the behavior manifest has Markdown command authority only while the package is active.
    - Retained disabled/invalid fallback coverage proving disabled packages cannot compose Markdown behavior manifests and plain-text fallback has no `markdown.*` command authority.
    - Updated implementation wiki and smoke/performance documentation for the reintegrated runtime and markdown-it SDUI status.
    - `node --check packages/markdown/dist/index.js && node --check packages/markdown/dist/load.js && node --check packages/markdown/dist/sdui.js && node --check packages/markdown/dist/parser.js`: passed.
    - `cargo fmt --check`: passed.
    - `cargo test --test markdown_mode`: passed (30 tests).
    - `cargo test --test package_loading`: passed (19 tests).
    - `cargo test markdown_package_runtime_loads_markdown_it_workflow --lib`: passed.
    - `cargo test markdown_config_fixture_opens_workspace_and_publishes_status_sdui --lib`: passed.
    - `cargo test --test performance_budgets`: passed (9 tests).

- [x] Add markdown-it performance, regression, and benchmark verification
  - Acceptance Criteria:
    - Functional: Automated and documented manual verification cover markdown-it package activation, parsing, decoration rendering, editing, reload/restart, disabled/invalid fallback, docs/registry lookup, and smoke flow.
    - Performance: Benchmarks execute actual markdown-it parsing and the active package adapter on repository Markdown corpora at 1 MiB, 5 MiB, and 16 MiB; representative payloads stay within Phase 14/16 budgets; Criterion timing remains advisory unless deterministic budget guards apply.
    - Code Quality: Benchmark harnesses are deterministic, use existing committed repository Markdown files rather than dummy docs, do not mutate fixtures/source files, and clearly separate parser cost from Rust transport/rendering costs.
    - Security: Benchmark/smoke logs contain no user documents, secrets, absolute user paths, network fetches, shell authority beyond documented local commands, raw ops, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/performance.md`: Benchmark/security policy and previous parser findings.
      - `docs/development/ui-observability.md`: Structural UI regression policy.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Payload/no-hot-path rules.
    - Options Considered:
      - Reuse old mdast benchmark result names: rejected; active parser docs/tests must be markdown-it-specific.
      - Keep mdast comparison optional but benchmark active adapter by default: selected.
    - Chosen Approach:
      - Update parser benchmark defaults and docs for markdown-it, run CI-friendly syntax/dry-run checks, run local parser/adapter timings, and record results in performance docs and plan verification notes when implementation completes.
    - API Notes and Examples:
      ```text
      npm install --prefix packages/markdown --no-save --no-package-lock --ignore-scripts markdown-it@^14.1.0
      node --check tools/bench/markdown-parser.mjs
      node tools/bench/markdown-parser.mjs --dry-run --sizes 1MiB --source-limit 8
      node --expose-gc tools/bench/markdown-parser.mjs --sizes 1MiB,5MiB,16MiB --parser markdown-it,adapter --iterations 1 --warmup 0
      cargo bench --no-run
      cargo test --test performance_budgets
      ```
    - Files to Create/Edit:
      - `tools/bench/markdown-parser.mjs`: Active parser/adapter benchmark paths use markdown-it and were verified unchanged by syntax, dry-run, and actual parser/adapter runs.
      - `benches/markdown_baselines.rs`: Generic Rust-side package/rendering baselines were compiled by `cargo bench --no-run`; no wording change required.
      - `src/server/js_runtime.rs`: Added `markdown_it_adapter_large_fixture_span_counts_are_stable`, a runtime-backed repeated token-stream adapter regression guard.
      - `docs/development/performance.md`: Recorded markdown-it commands/results, active parser/adapter benchmark findings, hard regression guards, registry/docs lookup coverage, smoke flow, and parser decision.
      - `docs/development/ui-observability.md`: Verified structural Markdown SDUI coverage remains accurate; no edit required.
      - `docs/wiki/modules/performance-fixtures.md`, `docs/wiki/modules/first-party-markdown-package.md`, and `docs/wiki/modules/markdown-mode-activation.md`: Updated implementation wiki coverage for active benchmark results and large fixture span-count regression.
      - `tests/performance_budgets.rs`: Added active markdown-it benchmark documentation guards and aligned benchmark-script policy test naming.
      - `tests/performance_protocol.rs` and `tests/editor_performance_invariants.rs`: Verified existing hard budget/hot-path guards pass unchanged.
    - References:
      - `src/perf/budgets.rs`
      - `docs/wiki/modules/performance-fixtures.md`
  - Test Cases to Write:
    - `markdown_it_parser_benchmark_script_uses_real_parser_and_repo_corpus`: Benchmark script calls markdown-it and builds corpora from committed Markdown files.
    - `markdown_it_adapter_large_fixture_span_counts_are_stable`: Active adapter emits deterministic nonzero spans at large repeated-fixture scale.
    - Existing `markdown_parse_and_decoration_payloads_fit_budgets`: Representative updates remain under budget after the rewrite.
    - `markdown_benchmark_docs_record_markdown_it_results`: Performance docs include commands, timing/memory notes, and recommendation.
  - Verification Completed:
    - Re-read the approved markdown-it decision, performance guide, UI observability guide, protocol/performance pattern, markdown parser-adapter pattern, maintenance-validation pattern, and Markdown/performance wiki pages before verification work.
    - Confirmed Context7 `/markdown-it/markdown-it` docs through the required CLI flow: `library markdown-it ...` selected `/markdown-it/markdown-it`; the docs query confirmed benchmark guidance and markdown-it parsing architecture. One docs call needed `MSYS_NO_PATHCONV=1` because Git Bash path conversion rewrote the library ID.
    - Added runtime-backed large repeated token-stream coverage in `src/server/js_runtime.rs` so the package adapter produces stable deterministic counts: 192 repeated blocks yield 1,344 spans, including 384 list-marker spans, without exposing markdown-it tokens to Clay protocol shapes.
    - Updated `docs/development/performance.md` with active 2026-06-04 markdown-it/parser-adapter results for repository-Markdown corpora: 1.01 MiB (`markdown-it` 127.234 ms, adapter 190.213 ms), 5.02 MiB (428.597 ms, 608.680 ms), and 16.01 MiB (1007.381 ms, 1577.844 ms), including token/span counts, memory notes, corpus policy, and security boundaries.
    - Updated implementation wiki pages for performance fixtures, the first-party Markdown package, and Markdown mode activation/regression coverage.
    - `npm install --prefix packages/markdown --no-save --no-package-lock --ignore-scripts markdown-it@^14.1.0`: passed and populated ignored local `packages/markdown/node_modules` only.
    - `node --check tools/bench/markdown-parser.mjs`: passed.
    - `node tools/bench/markdown-parser.mjs --dry-run --sizes 1MiB --source-limit 8`: passed and confirmed repository-Markdown corpus coverage without importing parsers or mutating fixtures.
    - `node --expose-gc tools/bench/markdown-parser.mjs --sizes 1MiB,5MiB,16MiB --parser markdown-it,adapter --iterations 1 --warmup 0`: passed and produced the active benchmark results recorded above.
    - `cargo fmt --check`: passed.
    - `cargo test --test performance_budgets`: passed (10 tests).
    - `cargo test --test markdown_mode`: passed (30 tests).
    - `cargo test markdown_it_adapter_large_fixture_span_counts_are_stable --lib`: passed.
    - `cargo test markdown_parser_adapter_publishes_viewport_bounded_decorations --lib`: passed.
    - `cargo test markdown_package_runtime_loads_markdown_it_workflow --lib`: passed.
    - `cargo test markdown_config_fixture_opens_workspace_and_publishes_status_sdui --lib`: passed.
    - `cargo test --test performance_protocol`: passed (4 tests).
    - `cargo test --test editor_performance_invariants`: passed (4 tests).
    - `cargo test --test clay_js_doc_registry`: passed (20 tests).
    - `cargo bench --no-run`: passed and compiled all benchmark targets, including `benches/markdown_baselines.rs`.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Markdown user-visible behavior-changing settings are either exposed as documented Clay JS configuration APIs or explicitly verified as fixed defaults for the POC; candidate settings include mode preference, decoration theme/style token mapping, parse policy/timeout, preview visibility, and package-owned Markdown options.
    - Performance: Configuration evaluation remains load-time or explicit setting-change work and does not execute on keypress/paint/scroll/text-event paths; parse/decorations policy changes apply asynchronously and are bounded.
    - Code Quality: Configuration options use `~/.config/clay/init.js` and Clay JS APIs with `custom_properties`, not ad hoc package metadata or hidden settings.
    - Security: Configuration cannot implicitly grant package enable/disable authority, filesystem, network, shell, extension loading, raw ops, AI mutation, workspace mutation, or client-side JavaScript authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay configuration task.
      - `.agents/skills/project-patterns/references/configuration-system.md`: `init.js` and configuration-as-Clay-JS-API rule.
      - `docs/reference/primitives/registry.md`: `PackageOwnedConfiguration`, `setPackageOption`, `setModePreference`, `setDecorationTheme`, and `setParsePolicy` planned surfaces.
    - Options Considered:
      - Store Markdown parser settings only inside package-private JSON: rejected for user-visible behavior changes.
      - Use existing configuration surfaces where sufficient and document fixed defaults if no options ship: selected.
    - Chosen Approach:
      - Audit the final markdown-it behavior and add/verify configuration APIs only for real user-facing settings, with docs/index/registry/tests and security notes.
    - API Notes and Examples:
      ```ts
      import { setPackageOption, setModePreference, setDecorationTheme, setParsePolicy } from "clay:configuration";

      setModePreference({ extension: "md", mode: "markdown" });
      setDecorationTheme({ mode: "markdown", token: "markup.heading.1", style: "heading-1" });
      setParsePolicy({ mode: "markdown", timeoutMs: 50, viewportPriority: true });
      setPackageOption({ packagePrefix: "markdown", key: "preview.enabled", value: true });
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration/**`: Add/update docs for concrete configuration APIs.
      - `docs/reference/clay-js-api/api-inventory.toml`, generated registry artifacts, and `docs/index.md`: Add/update configuration entries.
      - `src/server/ops/**`, `runtime/js/configuration.ts`, and configuration modules: Implement/finalize APIs only if concrete settings are introduced.
      - `tests/package_loading_docs.rs` and/or configuration tests: Coverage for docs, custom properties, lookup, and security validation.
      - `packages/markdown/docs/index.md`: Document package-owned options when present.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
  - Test Cases to Write:
    - `markdown_configuration_options_have_custom_properties`: Behavior-changing options fail docs/registry tests when custom metadata is missing.
    - `markdown_parse_policy_configuration_is_bounded`: Timeout/viewport policy rejects unsafe values.
    - `markdown_configuration_does_not_enable_package_authority`: Config cannot silently install/enable packages or grant prohibited permissions.
    - `markdown_fixed_defaults_require_no_configuration_api`: If no options ship, verification documents that no user-visible behavior-changing setting exists.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Every public programmatic surface introduced or changed by the markdown-it rewrite is exposed through stable Clay JS/TS facades and documented Markdown pages, or explicitly kept private/`pub(crate)` when internal; raw Rust public functions and raw `Deno.core.ops.op_*` names are not user-facing APIs.
    - Performance: Public APIs document hot-path policy and budget expectations; APIs used by parse/decorations are asynchronous/background where required and do not make ordinary typing/rendering synchronously dependent on JavaScript.
    - Code Quality: API docs include stable ID, JS module/export, facade path, op wrapper/backing Rust path where applicable, user-facing name, key bindings or empty list, custom properties or empty list, examples, options, errors, permissions, authority notes, lookup tags, and app/help/agent visibility.
    - Security: Permission-bearing APIs document and enforce permissions; package-owned APIs/commands carry the `markdown` prefix/provenance; docs state prohibited authorities and sanitized diagnostics behavior.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`: Required Clay JS API verification task.
      - `.agents/skills/project-patterns/references/documentation-as-code.md`: Markdown-authoritative docs and generated registry contract.
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`: Rust public function to op/facade rule.
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`: Module/export/stable-ID/user-facing-name layers and package prefix rule.
      - `.agents/skills/project-patterns/references/doc-registry-tests.md`: Non-mutating registry/test requirements.
    - Options Considered:
      - Document only Clay-owned APIs and skip package-provided commands/options: rejected; users and AI agents must discover package capabilities and provenance.
      - Inventory all changed public surfaces after implementation and update docs/registry in one pass: selected.
    - Chosen Approach:
      - Audit changed Rust modules, ops, JS facades, package commands, package configuration, parse/decorations APIs, mode APIs, registry entries, and runtime exports; make internal Rust helpers private/`pub(crate)` or expose them through documented Clay JS APIs.
    - API Notes and Examples:
      ```text
      JS module: clay:decorations
      JS export: serverPublishDecorations
      Stable ID: clay.decorations.serverPublishDecorations

      Package command ID: markdown.togglePreview
      User-facing name: Toggle Markdown Preview
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/**`: Add/update API docs for changed parse, decorations, mode, command, configuration, and package surfaces.
      - `docs/index.md`: Link all new/changed public API docs.
      - `docs/reference/clay-js-api/api-inventory.toml`: Add/update inventory entries.
      - `docs/generated/clay-js-api-registry.json`: Regenerate with project command.
      - `src/server/ops/**`, `runtime/js/**`, and touched Rust modules: Align public/private/facade boundaries.
      - `tests/package_loading_docs.rs`, `tests/clay_js_api_inventory.rs`, `tests/clay_js_doc_registry.rs`, and `tests/rust_visibility_api_mapping.rs`: Update coverage.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
  - Test Cases to Write:
    - `markdown_it_public_surfaces_have_clay_js_api_docs`: Fails when a public Markdown-related API lacks docs.
    - `markdown_api_docs_are_linked_from_index`: Fails when docs are absent from `docs/index.md`.
    - `markdown_generated_registry_is_fresh`: Fails with actionable update command when generated artifacts are stale.
    - `markdown_package_commands_are_lookup_visible`: App/help/agent lookup can find Markdown commands and provenance metadata.
    - `server_public_functions_are_private_or_facade_backed`: Changed server-side public Rust functions either have documented facades/ops or are not public.

- [ ] Record primitive coverage in reference docs, wiki pages, and tests
  - Acceptance Criteria:
    - Functional: Every new or changed generic primitive from this rewrite is recorded in `docs/reference/primitives/**`, relevant `docs/wiki/modules/**` pages, and `docs/wiki/index.md` navigation.
    - Performance: Primitive docs identify hot-path policy, payload budgets, viewport/incremental behavior, and benchmark or invariant tests where relevant.
    - Code Quality: Primitive docs explain source paths, protocol/API shapes, package permissions, validation rules, examples of reuse by current/future modes, and boundaries between generic Rust primitives and package-specific JS logic.
    - Security: Primitive docs record permissions, validation, prohibited authorities, inert-data rules, and stale/invalid payload handling without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Primitive documentation workflow and required deterministic tests.
      - `.agents/skills/project-wiki/references/page-template.md`: Primitive coverage section.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`: Future agents must consult primitive wiki before JS package work.
      - `.agents/skills/project-patterns/references/maintenance-validation.md`: Deterministic docs/wiki tests.
    - Options Considered:
      - Rely on reference docs only: rejected; implementation wiki must teach how primitives work internally.
      - Rely on wiki only: rejected; public primitive references and tests must stay authoritative for APIs/security.
      - Maintain both with deterministic coverage tests: selected.
    - Chosen Approach:
      - Update primitive reference docs and wiki pages together, then add or extend tests that fail when primitive registry entries, wiki pages, or index links are stale.
    - API Notes and Examples:
      ```text
      cargo test --test primitives_docs
      cargo test --test package_primitive_gate
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/**`: Update registry/strategy/backlog docs for changed primitives.
      - `docs/wiki/modules/primitive-architecture.md`, `docs/wiki/modules/rendering-primitives.md`, `docs/wiki/modules/parse-coordinator.md`, `docs/wiki/modules/decoration-transport.md`, and related module pages: Update implementation details.
      - `docs/wiki/index.md`: Link every new/changed primitive page.
      - `tests/primitives_docs.rs`: Add coverage that every implemented primitive has reference and wiki documentation.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
  - Test Cases to Write:
    - `primitive_registry_entries_have_wiki_pages`: Implemented primitive entries map to wiki pages.
    - `primitive_wiki_pages_are_linked_from_index`: Master wiki index links all primitive pages.
    - `primitive_docs_cover_permissions_hot_paths_and_tests`: Primitive docs include permission, hot-path, and test metadata.
    - `js_mode_package_docs_reference_primitive_inventory`: Markdown package docs link to the primitive inventory used by package authors.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`: Use the project wiki workflow and quality bar.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown and primitive code wiki once using `project-wiki`, including the master index and relevant pages. If the dedicated primitive coverage task already updated a page, verify it instead of duplicating content.
    - API Notes and Examples:
      ```text
      docs/wiki/index.md
      docs/wiki/modules/<module>.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Add or update navigation links for changed implementation areas.
      - `docs/wiki/**`: Add or update implementation wiki pages for changed code.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: Confirm the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made

- To be filled after tasks are completed and tests pass.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.
