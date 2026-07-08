# Plan 045: Web-Tree-Sitter Syntax Highlighting, Non-Blocking Open Parse, and Markdown Decoration Folding

## Objectives

- Deliver visible syntax highlighting for `.rs`, `.ts`, `.js`, and `.md` files in the real `cargo run` + `~/.config/clay/init.js` workflow through one generic, package-owned mechanism.
- Adopt a host-side `web-tree-sitter` (tree-sitter compiled to WebAssembly, run inside the existing Deno/V8 background worker) adapter as the single syntax-decoration path for packages that declare a `tree-sitter-wasm` grammar. The host loads each package's declared `grammars/*.wasm` + `queries/highlights.scm` generically; no per-language Rust branches.
- Fold Markdown decorations onto the same generic tree-sitter path. Markdown preview SDUI stays package-JS; only the decoration role of `packages/markdown/dist/parser.js` moves to the generic adapter.
- Keep a per-package JS parse-handler fallback available for languages with no tree-sitter grammar (current `@clay/markdown` `parser.js` route shape), documented as the non-tree-sitter fallback.
- Fix the open-time parse pipeline so the document renders immediately on open and `DecorationSet` paints when background parse completes, and so handler errors surface as `RuntimeDiagnostic`s instead of silently dropping and starving a 6s blocking wait (root cause of the `clay.parse.open_activation_timeout` hang observed in Plans 043/044 manual testing).
- Preserve Clay authority boundaries: packages own grammar/query/style assets and JS metadata; the host owns the generic wasm adapter, parse scheduling, decoration transport, and error reporting. No new filesystem/network/shell/AI/raw-op/native-widget authority for packages; parse stays background/bounded through the existing token-backed JS→Rust `ParseHandler` bridge.
- Reject native `tree-sitter-*` Rust crates compiled into Clay core (per decision `2026-07-08-2316-web-tree-sitter-host-adapter-for-syntax-highlighting-and-non-blocking-open-parse.md` and `.agents/skills/project-patterns/references/language-capability-sequencing.md`).

## Expected Outcome

- A user loads `@clay/markdown`, `@clay/rust`, `@clay/typescript`, and `@clay/javascript` in `~/.config/clay/init.js`, runs `cargo run`, opens a workspace file, and sees token-family syntax-highlighting background tints (the `decoration_color()` mapping from Plan 044 unchanged) for Markdown, Rust, TypeScript, and JavaScript.
- Opening any supported file renders the text immediately; `DecorationSet` paints asynchronously when background parse finishes; a slow/failing parse produces a visible `RuntimeDiagnostic` but neither hangs the app nor kills the file browser.
- Markdown preview/SDUI continues to work unchanged.
- A future first-party `@clay/<lang>` package that ships a `tree-sitter-wasm` grammar + query gets highlighting with **no Clay rebuild** and no per-language Rust code.
- Focused Linux validation passes: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, focused unit/integration/doc tests, and `cargo test --all-targets`.

## Tasks

- [ ] Entry gate: reproduce the missing-highlighting and open-parse-hang bug contract
  - Acceptance Criteria:
    - Functional: Lock the reported failures as deterministic repro tests or documented manual repro notes before implementation: (1) opening `.rs`/`.ts`/`.js` produces no `DecorationSet` because no parse handler is registered; (2) `ScheduleOpenParse` blocks up to 6s on `parse_coordinator.next_update()` and emits `clay.parse.open_activation_timeout` when a handler fails or times out; (3) `ParseCoordinator::finish_task` silently drops handler errors (`failed_tasks += 1; return`) without publishing an update or diagnostic; (4) opening `.md` surfaces the same blocking-loop hang as (2)/(3).
    - Performance: Repro tests must run headless (no GUI, no desktop portal, no shell from test code); GUI-only behavior gets headless state/protocol tests plus one manual smoke note.
    - Code Quality: Each repro names the owning layer: grammar-vs-handler registration gap, `schedule_open_parse` blocking loop, `finish_task` silent-drop, or JS worker timeout/heap path.
    - Security: Repros use temp workspaces or existing fixtures; they must not grant packages new authority or bypass the first-party-only resolver.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-07-08-2316-web-tree-sitter-host-adapter-for-syntax-highlighting-and-non-blocking-open-parse.md` — finalized architecture decision and rationale.
      - `src/server/connection.rs::schedule_open_parse` and `open_document_followup_messages` — 6s blocking `next_update()` loop and `clay.parse.open_activation_timeout` emission.
      - `src/server/parse_coordinator.rs::finish_task` — silent `let Ok(update) = result else { failed_tasks += 1; return; }` drop.
      - `src/server/js_runtime.rs::evaluate_js_parse_handler` — JS worker `RuntimeCommand::Parse` execution path and timeout/heap handling.
      - `src/server/ops/parse.rs::op_clay_parse_register_parse_handler` — `runtimeBridge` flag and `register_parse_handler_meta` vs `register_js_parse_handler`.
      - `packages/rust/package.json`, `packages/typescript/package.json`, `packages/javascript/package.json` — `contributes.syntaxGrammars` declared but no parse handler registered.
      - `docs/wiki/modules/parse-coordinator.md` and `docs/wiki/modules/server-ipc-skeleton.md` (Plan 044 updates) — open-time follow-up flow.
    - Options Considered:
      - Reproduce only manually: not CI-stable, fails the existing validation style.
      - Add the smallest headless repro per failure plus one manual smoke note: durable, matches Clay style.
    - Chosen Approach:
      - Add headless repro tests for the registration gap, the blocking-loop starvation, and the silent-drop; keep one documented manual GNOME smoke note for the visible "no highlighting + timeout hang" behavior.
    - API Notes and Examples:
      ```text
      cargo test --lib server::connection --quiet
      cargo test --lib server::parse_coordinator --quiet
      cargo test --lib server::js_runtime --quiet
      cargo test --test manual_smoke_docs --quiet
      ```
    - Files to Create/Edit:
      - `docs/development/syntax-highlighting-and-open-parse-bug-contract.md`: locked repro notes for the four failures, owning layers, and entry-gate rules.
      - `tests/manual_smoke_docs.rs`: add `syntax_highlighting_and_open_parse_bug_contract_locks_reported_failures` verifying the contract file names the four failures, owning layers, the runtime bridge path, and the non-tree-sitter fallback.
      - `src/server/connection.rs`: add a repro test that opening a `.rs` document without a registered handler yields no `DecorationSet` and surfaces a non-fatal diagnostic instead of blocking the session.
      - `src/server/parse_coordinator.rs`: add a repro test that a handler returning `Err` is today silently dropped (assert `failed_tasks` increments and no update is published) to lock the pre-fix behavior before the silent-drop fix lands.
    - References:
      - `src/server/connection.rs::schedule_open_parse`
      - `src/server/parse_coordinator.rs::finish_task`
      - `src/server/js_runtime.rs::evaluate_js_parse_handler`
  - Test Cases to Write:
    - `language_package_without_parse_handler_publishes_no_decoration_set`: opening `.rs` with `@clay/rust` loaded but no registered parse handler yields no `ServerMessage::DecorationSet`.
    - `parse_coordinator_silently_drops_handler_error_before_fix`: a registered handler returning `Err` increments `failed_tasks` and publishes no update (locks pre-fix behavior for the silent-drop task).
    - `schedule_open_parse_timeout_emits_diagnostic_not_session_hang`: a handler that never publishes causes `schedule_open_parse` to return a `RuntimeDiagnostic` rather than blocking indefinitely (headless bound on the loop).

- [ ] Review existing editor/parse/grammar primitives before implementing the highlighter
  - Acceptance Criteria:
    - Functional: Inventory the existing primitives to reuse: `clay.parse.serverRegisterParseHandler` JS bridge (`runtimeBridge` flag), `ParseCoordinator` background scheduling + `next_update`, `ParseHandlerMeta`/`JsParseHandlerRegistration`, `clay.syntax.serverRegisterSyntaxGrammar` validation, `SyntaxGrammarContributionDescriptor` (grammar kind/path/query/styleMap), `decoration_color()` token-family mapping, `EditorSurface::apply_decoration_set`, `bounded_utf8_prefix` parse window, and the persistent JS runtime worker (`RuntimeCommand::Parse`).
    - Performance: The review must preserve hot-path policy: no JS/wasm/filesystem/network work in editor paint/pointer/text/scroll hot paths; parse stays on the background worker with existing timeout/heap budgets; the host wasm runtime is instantiated once, not per document open.
    - Code Quality: The review must reject native `tree-sitter-*` Rust crates, per-language Rust branches, `wasmtime` native deps, hard-coded grammar names in Rust, and any relaxation of `StaticSduiState` validation or authority boundaries. New primitives must be generic across future modes/packages.
    - Security: Confirm the adapter grants no new filesystem/network/shell/AI/raw-op/native-widget authority to packages; grammar artifacts are package-root-confined `.wasm` paths validated by `validate_package_asset_path`; third-party grammar loading stays out of scope.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/primitives/index.md`, `docs/reference/primitives/registry.md`, `docs/wiki/modules/primitive-architecture.md`, `docs/wiki/modules/parse-coordinator.md`, `docs/wiki/modules/decoration-transport.md`, `docs/wiki/modules/markdown-parser.md`, `docs/wiki/modules/syntax-grammar-registry.md`, `docs/wiki/modules/server-ipc-skeleton.md`.
      - `.agents/skills/project-patterns/references/mode-primitive-first.md` (parse handler bridge, no mode-specific Rust branches), `language-capability-sequencing.md` (grammar contributions via generic package primitives; do not bundle language ownership into core), `authority-boundaries.md`, `protocol-and-performance.md`.
      - `packages/markdown/dist/load.js` and `packages/markdown/dist/parser.js` — current JS parse handler registration shape to mirror for the generic adapter.
    - Options Considered:
      - Implement highlighting per package in JS: duplicates work across languages.
      - Add a generic host Rust adapter: rejected (native crates / wasmtime) per the decision log.
      - Add a generic host **JS** adapter module that every package delegates to: one engine, package-owned grammar/query, matches the existing `serverRegisterParseHandler` path.
    - Chosen Approach:
      - A single generic host JS module (bundled with the runtime) wraps `web-tree-sitter`, loads a package-declared `.wasm` grammar + `.scm` query at that package's load time, and registers a parse handler on the package's behalf through the existing `clay.parse.serverRegisterParseHandler` bridge. No per-language Rust; no language names in Rust.
    - API Notes and Examples:
      ```text
      // package load entry delegates:
      import { registerTreeSitterGrammarHandler } from "clay:syntax";
      await registerTreeSitterGrammarHandler({
        packageManifest, mode, languageId,
        grammarWasm: await readGrammarBytes("./grammars/rust.wasm"),
        highlightsQuery: await readQueryText("./queries/highlights.scm"),
        styleMap, budgets
      });
      ```
    - Files to Create/Edit:
      - `docs/wiki/modules/syntax-highlighting-host-adapter.md` (new): maps the generic adapter onto existing primitives and records rejected approaches (native crates, wasmtime, per-language Rust, third-party grammars).
      - `docs/wiki/modules/primitive-architecture.md`: cross-reference the new host-adapter page and the open-time non-blocking parse change.
    - References:
      - `src/server/ops/parse.rs::op_clay_parse_register_parse_handler`
      - `src/server/js_runtime.rs::evaluate_js_parse_handler` and `register_parse_handlers`
      - `src/server/syntax.rs::TreeSitterSyntaxHandler` (reference decoration shape, test-only today)
      - `src/editor/surface.rs::apply_decoration_set` and `decoration_color`
  - Test Cases to Write:
    - `host_adapter_module_loads_into_persistent_runtime`: the generic adapter module is resolvable and executable inside the persistent JS runtime (verifies the host-side wiring exists before package work).

- [ ] Vendor `web-tree-sitter` and real first-party `grammars/*.wasm` + query artifacts
  - Acceptance Criteria:
    - Functional: Add `web-tree-sitter` as a host-bundled JS dependency (npm package, committed vendored copy acceptable in-tree), and commit real `tree-sitter-rust`, `tree-sitter-typescript` (`typescript` + `tsx`), `tree-sitter-javascript`, and `tree-sitter-markdown`/`tree-sitter-md` `*.wasm` grammar artifacts under the respective `packages/<lang>/grammars/` directories, plus a `queries/highlights.scm` for each (Rust/TS/JS already present; add Markdown).
    - Performance: Grammar artifact sizes stay bounded (document each artifact's bytes in its `grammars/README.md`); artifacts are loaded once per package load and cached by the adapter, not reloaded per parse.
    - Code Quality: Each artifact records provenance (upstream release tag/version and the command used to produce/vendor it) in `grammars/README.md`; artifacts are first-party only and package-root-confined; no network fetch at runtime.
    - Security: Artifacts come only from upstream tree-sitter release tags; no untrusted third-party grammars; an integrity note (size/source) is recorded per artifact; packages cannot load grammar paths outside their root.
  - Approach:
    - Documentation Reviewed:
      - `web-tree-sitter` npm package documentation (via `find-docs`/ctx7) for the browser/wasm load + `Language.load` + `Parser.setLanguage` + `query` API.
      - Upstream `tree-sitter-rust`, `tree-sitter-typescript`, `tree-sitter-javascript`, `tree-sitter-markdown`/`tree-sitter-md` release artifacts for `*.wasm` grammar outputs compatible with the bundled `web-tree-sitter` version.
      - `packages/rust/grammars/README.md`, `packages/typescript/grammars/README.md`, `packages/javascript/grammars/README.md` (existing contract narrative).
    - Options Considered:
      - Build grammars to `.wasm` from source at Clay build time: adds a build dependency (emcc/clang) divergence across platforms.
      - Vendor upstream-released `*.wasm` artifacts (commit binaries in-tree): reproducible, no build toolchain requirement, matches first-party-only contract; chosen.
    - Chosen Approach:
      - Vendored upstream `*.wasm` grammar artifacts + a host-bundled `web-tree-sitter` JS runtime; record provenance per artifact. Verify the selected `web-tree-sitter` version is ABI-compatible with every vendored grammar wasm.
    - API Notes and Examples:
      ```text
      npx ctx7@latest library "web-tree-sitter" "load wasm grammar and run highlight query"
      ```
    - Files to Create/Edit:
      - `packages/rust/grammars/rust.wasm` (vendored binary), `packages/rust/grammars/README.md` (provenance/size).
      - `packages/typescript/grammars/typescript.wasm`, `packages/typescript/grammars/tsx.wasm` (as needed), `packages/typescript/grammars/README.md`.
      - `packages/javascript/grammars/javascript.wasm`, `packages/javascript/grammars/README.md`.
      - `packages/markdown/grammars/markdown.wasm`, `packages/markdown/queries/highlights.scm`, `packages/markdown/grammars/README.md`.
      - Host bundled `web-tree-sitter` runtime location (vendored JS + `tree-sitter.wasm` core) under a `runtime/vendor/` or `src/server/` asset path resolved by the runtime loader; exact path fixed in the adapter task.
    - References:
      - `packages/*/package.json` `contributes.syntaxGrammars[*].grammar.path`
      - `src/packages/record.rs::validate_package_asset_path` (`.wasm` suffix already accepted)
  - Test Cases to Write:
    - `first_party_grammar_artifacts_exist_with_provenance`: each first-party language package's declared `grammar.path` resolves to a committed `.wasm` file with a documented provenance line in `grammars/README.md`.
    - `web_tree_sitter_runtime_is_bundled_and_loadable`: the host `web-tree-sitter` runtime is present at the expected asset path and loadable by the persistent runtime (no runtime network fetch).

- [ ] Implement the generic host `web-tree-sitter` syntax adapter
  - Acceptance Criteria:
    - Functional: One generic host JS module exposes a registration entry that any first-party package can call at load time. It loads the package's declared `.wasm` grammar + `highlights.scm` query, instantiates a `Parser` + `Query` once per grammar, and registers a parse handler through `clay.parse.serverRegisterParseHandler` (`runtimeBridge: true`) emitting `DecorationSpan`s keyed by the package's `styleMap`. The adapter covers Rust, TypeScript, JavaScript, and Markdown with no per-language Rust and no language names in Rust.
    - Performance: Parse runs only on the background `ParseCoordinator` lane via the existing JS worker; the wasm runtime and compiled `Language`/`Query` are cached per grammar and reused across parses (no re-instantiate per document); parse honors existing `timeoutMs`/`maxWindowBytes`/`memoryBudgetBytes` budgets; editor paint/pointer/text/scroll hot paths perform no wasm/JS work.
    - Code Quality: The adapter is generic over `languageId`/`styleMap`/query/budgets; no Rust branches keyed by language; decorations flow through the existing `EditorSurface::apply_decoration_set` + `decoration_color()` mapping unchanged; `TreeSitterSyntaxHandler` remains for tests and as the reference shape.
    - Security: The adapter grants no new package authority; grammar bytes are read from package-root-confined paths validated upstream; no `Deno.core.ops` raw routes are exposed to packages beyond the existing denied surface; no filesystem/network/shell access from the wasm runtime.
  - Approach:
    - Documentation Reviewed:
      - `web-tree-sitter` docs for `Language.load(bytes)`, `Parser.setLanguage`, `Query.new(language, source)`, `QueryMatch`/captures iteration, and incremental `Parser.parse(text, oldTree)`.
      - `runtime/js/parse.ts` (`serverRegisterParseHandler`, `runtimeBridge`, `__clayParseHandlers` registry) and `src/server/js_runtime.rs::evaluate_js_parse_handler` (the worker dispatch path the adapter will populate).
      - `src/packages/record.rs::SyntaxGrammarContributionDescriptor` and `src/server/ops/syntax.rs::op_clay_syntax_register_syntax_grammar` for the grammar metadata already validated at package load.
      - `src/server/syntax.rs::TreeSitterSyntaxHandler::decorations_for_window` for the viewport-bounded span emission and `MAX_SYNTAX_HIGHLIGHT_SPANS` clamp to mirror in JS.
    - Options Considered:
      - Per-package JS wrapper around `web-tree-sitter` duplicated in each package: rejected (duplicates the engine plumbing).
      - One generic host module imported by every package: chosen (one engine, package-owned assets).
    - Chosen Approach:
      - Add a host `clay:syntax` (or analogous existing facade) JS module that exposes `registerTreeSitterGrammarHandler({packageManifest, mode, languageId, grammarWasm, highlightsQuery, styleMap, budgets})`. It builds a `Language`/`Parser`/`Query` once, registers a `parseDecorations(notification)` function via `serverRegisterParseHandler({module, exportName, ...})`, and emits `DecorationSpan`s clamped to the notification viewport with a span cap mirroring `MAX_SYNTAX_HIGHLIGHT_SPANS`.
    - API Notes and Examples:
      ```js
      // packages/rust/dist/load.js delegates:
      import { registerTreeSitterGrammarHandler } from "clay:syntax";
      await registerTreeSitterGrammarHandler({
        packageManifest: rustPackageManifest(),
        mode: modeId,
        languageId: "rust",
        grammarWasm: new Uint8Array(await import("./grammars/rust.wasm?arraybuffer")),
        highlightsQuery: await import("./queries/highlights.scm?raw"),
        styleMap: { keyword: "keyword.control", string: "string.quoted", comment: "comment.line", punctuation: "punctuation.definition" },
        budgets: { timeoutMs: 5000, maxWindowBytes: 64 * 1024 }
      });
      ```
    - Files to Create/Edit:
      - `runtime/js/syntax.ts` (new): generic `registerTreeSitterGrammarHandler` facade delegating to `web-tree-sitter`.
      - `src/server/js_runtime.rs`: register the new facade in the inline runtime module set (mirroring `CLAY_FACADE_*` constants) and ensure the bundled `web-tree-sitter` runtime is resolvable by the persistent worker loader.
      - `packages/rust/dist/load.js`, `packages/typescript/dist/load.js`, `packages/javascript/dist/load.js`: replace grammar-metadata-only load with a call to the generic adapter (keep mode pattern, commands, completion, status item registration unchanged).
      - `packages/markdown/dist/parser.js` and `packages/markdown/dist/load.js`: replace the decoration-producing path with a delegation to the generic adapter; keep `sdui.js` preview panel registration unchanged.
    - References:
      - `runtime/js/parse.ts`
      - `src/server/js_runtime.rs::evaluate_js_parse_handler`
      - `src/server/syntax.rs::TreeSitterSyntaxHandler` (reference shape)
      - `src/editor/surface.rs::decoration_color`
  - Test Cases to Write:
    - `generic_adapter_registers_parse_handler_for_rust_grammar`: loading `@clay/rust` registers a `(rust, rust)` parse handler in `ParseCoordinator` via the JS bridge.
    - `generic_adapter_emits_decoration_set_for_rust_fixture`: parsing a Rust fixture through the adapter produces a `DecorationSet` with `keyword.control`, `string.quoted`, and `comment.line` spans within the viewport cap.
    - `generic_adapter_reuses_parser_across_parses`: two consecutive parses reuse the same compiled `Language`/`Query` (assert via adapter invariants; no re-instantiation observable in op records).
    - `markdown_decorations_route_through_generic_adapter`: opening a `.md` file yields a `DecorationSet` from the markdown grammar's query (no longer from a hand-written `parser.js` decoration path), with preview SDUI still registered.

- [ ] Fix `ParseCoordinator::finish_task` to surface handler errors as diagnostics
  - Acceptance Criteria:
    - Functional: When a registered parse handler returns `Err` (or the JS worker returns a `ClayRuntimeError`), `finish_task`/the coordinator produces a `RuntimeDiagnostic` (not an update) and the open-time path forwards it to the client instead of silently incrementing `failed_tasks` and returning; the stale-generation and stale-version rejection cases still drop silently as before (they are expected, not errors).
    - Performance: No extra IPC on the success path; error diagnostics follow the existing `ServerMessage::RuntimeDiagnostic` channel; no synchronous waiting added.
    - Code Quality: Pass error metadata through existing structures; keep `failed_tasks` accounting for stats; do not change success/stale validation order.
    - Security: Diagnostics must not leak package source text, filesystem paths beyond what existing diagnostics already carry, or credentials; error messages are bounded and redacted like existing `RuntimeDiagnostic`s.
  - Approach:
    - Documentation Reviewed:
      - `src/server/parse_coordinator.rs::finish_task`, `next_update`, `IncrementalParseUpdate`, `ParseCoordinatorError`.
      - `src/server/connection.rs::schedule_open_parse` (loop reading `next_update()` to map to `ServerMessage::DecorationSet` / `RuntimeDiagnostic`).
      - `src/protocol.rs` (or equivalent) `RuntimeDiagnostic`/`ServerMessage::RuntimeDiagnostic` shapes.
    - Options Considered:
      - Publish an `IncrementalParseUpdate` with an error field: changes the success contract.
      - Add an error channel alongside `updates_tx` (e.g. a small `diagnostics_tx`) consumed by the open-time loop: chosen, isolates errors from the update stream.
    - Chosen Approach:
      - Add a coordinator diagnostic output channel; `finish_task` sends a `RuntimeDiagnostic`-shaped message on handler `Err` (distinct from the stale-generation/stale-version expected drops); `schedule_open_parse` reads from both `next_update` and the diagnostic channel using `tokio::select!` so a handler error terminates the wait with a diagnostic instead of starving to the 6s timeout. The 6s timeout remains as a backstop but is no longer the primary failure signal.
    - API Notes and Examples:
      ```rust
      tokio::select! {
          update = parse_coordinator.next_update() => { /* existing decoration-set path */ }
          diag = parse_coordinator.next_diagnostic() => { return Err(diag); }
      }
      ```
    - Files to Create/Edit:
      - `src/server/parse_coordinator.rs`: add diagnostic sender/receiver and send on handler error in `finish_task`; expose `next_diagnostic()`.
      - `src/server/connection.rs::schedule_open_parse`: `select!` between `next_update` and `next_diagnostic`; convert diagnostic into `RuntimeDiagnostic` and return `Err(diagnostic)`.
    - References:
      - `src/server/parse_coordinator.rs::finish_task`
      - `src/server/connection.rs::schedule_open_parse`
  - Test Cases to Write:
    - `finish_task_publishes_diagnostic_on_handler_error`: a handler returning `Err` produces a coordinator diagnostic (assert it is read via `next_diagnostic`) instead of silently dropping.
    - `finish_task_still_silently_drops_stale_generation`: a stale-generation result is still dropped without a diagnostic (preserves existing behavior).
    - `schedule_open_parse_returns_diagnostic_on_handler_error`: open-time returns `Err(RuntimeDiagnostic)` promptly on handler failure rather than waiting out the full 6s.

- [ ] Make open-time parse non-blocking so the document renders immediately
  - Acceptance Criteria:
    - Functional: Opening a workspace file renders the document text immediately (`ServerMessage::InitialDocument` path) and does not gate the open handshake on parse completion; `DecorationSet` is delivered as a separate `ServerMessage::DecorationSet` whenever the background parse finishes; a `RuntimeDiagnostic` from a failed/timeout parse is delivered as a separate message but does not block the open.
    - Performance: Perceived open latency (text visible) is independent of parse duration; parse remains bounded by existing timeout/heap budgets and runs on the background lane; the file browser and editor remain responsive during a slow parse.
    - Code Quality: Reuse existing `schedule_parse_with_windows` for scheduling; do not duplicate parse logic; ensure document/version validation in `apply_decoration_set` still rejects stale sets.
    - Security: No new authority; background parse confined to the existing first-party package workspace grant and parse budgets.
  - Approach:
    - Documentation Reviewed:
      - `src/server/connection.rs::open_document_followup_messages` (currently returns decoration-set in the message vector synchronously after blocking on `schedule_open_parse`).
      - `src/server/connection.rs` open handler / `DocumentOpened` result path — where the initial snapshot is sent before follow-ups.
      - `src/client/mod.rs` `ServerMessage::DecorationSet` handling and `src/masonry_editor.rs::apply_decoration_set`.
    - Options Considered:
      - Keep blocking open with a short timeout: still hangs on slow parses; rejected.
      - Spawn the parse as a fire-and-forget task that pushes `DecorationSet`/`RuntimeDiagnostic` onto the connection writer when ready: chosen — document open returns the snapshot immediately.
    - Chosen Approach:
      - In the open path, schedule the parse (via `schedule_parse_with_windows`) and spawn a bounded background task that awaits `next_update()`/`next_diagnostic()` and writes `ServerMessage::DecorationSet`/`ServerMessage::RuntimeDiagnostic` to the client writer when ready. `open_document_followup_messages` returns the behavior manifest immediately without the decoration set (or a sentinel indicating deferred parse), so the editor renders text at once.
    - API Notes and Examples:
      ```text
      // open returns manifest + snapshot; decorations delivered asynchronously
      ServerMessage::InitialDocument(...) ; ServerMessage::BehaviorManifest(...)
      ...later...
      ServerMessage::DecorationSet(set) | ServerMessage::RuntimeDiagnostic(diag)
      ```
    - Files to Create/Edit:
      - `src/server/connection.rs`: split open-time decoration delivery from the open handshake; spawn the deferred parse waiter; ensure the writer handle is shared/cloned safely across the background task.
      - `docs/wiki/modules/server-ipc-skeleton.md`: document the deferred-decoration open flow and the diagnostic channel.
    - References:
      - `src/server/connection.rs::open_document_followup_messages` and `schedule_open_parse`
      - `src/client/mod.rs` and `src/masonry_editor.rs` client handling
  - Test Cases to Write:
    - `open_renders_document_before_parse_completes`: opening a document sends the initial snapshot/manifest before any `DecorationSet` is produced (assert ordering in a synthetic slow-parse scenario).
    - `deferred_decoration_set_applies_after_open`: a `ServerMessage::DecorationSet` arriving after open applies via `EditorSurface::apply_decoration_set` without reopening.
    - `deferred_parse_diagnostic_does_not_block_open`: a parse error after open produces a `RuntimeDiagnostic` message without the editor hanging or losing the document.

- [ ] Define and verify the package default `init.js` loading experience
  - Acceptance Criteria:
    - Functional: After `loadPackage("@clay/markdown")`, `loadPackage("@clay/rust")`, `loadPackage("@clay/typescript")`, and `loadPackage("@clay/javascript")` in `~/.config/clay/init.js`, the real `cargo run` workflow shows syntax highlighting for the corresponding file types with no additional low-level facade plumbing or manual handler registration in user config.
    - Performance: One adapter registration per package at load time; no per-open JS evaluation beyond the background parse; grammar/query loaded once per package.
    - Code Quality: User config stays declarative one-line loads; customization (if any) uses documented Clay/package JS APIs only.
    - Security: Packages do not gain new authority through the adapter; loading is still gated by the first-party-only resolver and existing permissions.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/configuration-system.md` and the Clay Configuration plan requirement (`~/.config/clay/init.js` one-line default loads).
      - `tests/fixtures/configuration/file-browser-workflow/init.js` (existing first-party load fixture).
    - Options Considered:
      - Require explicit `registerTreeSitterGrammarHandler` calls in user `init.js`: violates one-line default convention.
      - Packages self-register the generic adapter inside their own `load.js`: chosen — user only calls `loadPackage`.
    - Chosen Approach:
      - Each first-party language package's `load.js` calls the generic adapter as part of its standard load, so the one-line `loadPackage("@clay/rust")` default yields highlighting.
    - Files to Create/Edit:
      - `tests/fixtures/configuration/file-browser-workflow/init.js`: add `loadPackage("@clay/markdown")` and confirm rust/typescript/javascript loads still present.
      - `docs/development/launch-and-gui-smoke.md`: document the one-line default load for highlighting on the product `cargo run` path.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `tests/fixtures/configuration/file-browser-workflow/init.js`
  - Test Cases to Write:
    - `file_browser_workflow_config_fixture_loads_packages_and_bindings` (existing) keeps passing with markdown added; add an assertion that the generic adapter handler is registered for rust/typescript/javascript/markdown.

- [ ] Review package-provided grammar primitives before adjusting language packages
  - Acceptance Criteria:
    - Functional: Confirm grammar resolution (`SyntaxGrammarContributionDescriptor`), asset-path validation (`validate_package_asset_path` accepting `.wasm`), style-token mapping (`styleMap`), and parse handler registration stay generic; the generic adapter uses only documented primitive surfaces (`clay.syntax`/`clay.parse`).
    - Performance: Grammar/query loading bounded and cached; no per-open re-load; budgets enforced.
    - Code Quality: No language-specific Rust server/client branches; third-party grammar loading remains out of scope; tests cover package-provided grammar resolution, disabled/invalid package fallback, payload bounds, no hot-path JS/parser from packages, and no language-specific Rust.
    - Security: No new package authority; only first-party-resolver-validated packages register handlers.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/language-capability-sequencing.md` and `mode-primitive-first.md`.
      - `src/server/ops/syntax.rs`, `src/packages/record.rs`, `tests/syntax_grammar.rs`.
    - Chosen Approach:
      - Reuse existing grammar validation; the adapter adds a JS handler registration using the already-validated grammar descriptor. Add tests under `tests/syntax_grammar.rs` for the new vendor artifacts.
    - Files to Create/Edit:
      - `tests/syntax_grammar.rs`: extend the existing language-package fixture assertions to cover the generic adapter's decoration output for rust/typescript/javascript/markdown.
    - References:
      - `.agents/skills/project-patterns/references/language-capability-sequencing.md`
  - Test Cases to Write:
    - `first_party_grammar_packages_register_and_parse_via_generic_adapter` (replaces/extends the existing grammar-only test): each first-party package registers a parse handler and produces bounded `DecorationSet`s.

- [ ] Update the package UI/layout authoring contract and package guide
  - Acceptance Criteria:
    - Functional: `docs/reference/packages/creating-packages.md` documents how a package declares a `tree-sitter-wasm` grammar + query + `styleMap` and delegates to the generic host adapter via its `load.js`, plus the non-tree-sitter package-JS parse-handler fallback, with examples, limitations, permissions, and testing guidance.
    - Performance: Document that grammar/query load once per package load and parse runs on the background lane.
    - Code Quality: Document no raw `Deno.core.ops`, no per-language Rust, no third-party grammars.
    - Security: Document package-root confinement, first-party-only resolver, and denied authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/reference/packages/creating-packages.md` (current).
      - `.agents/skills/project-patterns/references/package-ui-layout.md` and `clay-js-api-boundary.md`.
    - Chosen Approach:
      - Add a "Syntax highlighting" section to the package guide with both tree-sitter and JS-fallback routes.
    - Files to Create/Edit:
      - `docs/reference/packages/creating-packages.md`: add the syntax-highlighting authoring section.
    - References:
      - `.agents/skills/project-patterns/references/package-ui-layout.md`
  - Test Cases to Write:
      - `package_guide_documents_tree_sitter_and_js_fallback_highlighting`: `tests/primitives_docs.rs` or `tests/manual_smoke_docs.rs` asserts the guide contains both route descriptions, the `styleMap`/query contract, first-party-only language, and denied-authority language.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Verify that all behavior introduced by the plan is expressible through existing Clay JS APIs (`loadPackage`, `clay.syntax.serverRegisterSyntaxGrammar`, `clay.parse.serverRegisterParseHandler`, `bindKey`); no new undocumented config keys for highlighter engine, budgets, scrollbars, or diagnostics. Any new tunable (e.g. parse budgets) must be a documented Clay JS API.
    - Performance: No hidden config keys affecting hot paths.
    - Code Quality: Configuration is JS-API-only and under `~/.config/clay/init.js`.
    - Security: No implicit authority via configuration.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/configuration-system.md` and the Clay Configuration plan requirement.
      - `docs/reference/clay-js-api/` inventory.
    - Chosen Approach:
      - Verify only; if a new budget-bearing API surface is added for parse budget overrides, document it under `docs/reference/clay-js-api/parse/` with frontmatter and registry coverage.
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/` as needed; `tests/clay_js_api_inventory.rs`/`tests/clay_js_doc_registry.rs` to cover any new doc.
    - References:
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `docs/reference/clay-js-api/keybindings/bind-key.md`
  - Test Cases to Write:
    - `highlighting_config_uses_documented_js_apis_only`: assert no new raw config keys; budget overrides (if added) are documented Clay JS APIs.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Any new server-side Rust public function exposed to JavaScript (e.g. an op wrapper to read package-root-confined grammar/query bytes for the adapter, if needed) is exposed through an explicit `deno_core` op + stable Clay JS/TS facade with documentation. Raw `Deno.core.ops.op_*` is not the user-facing API. Rust helpers that should not be JS-exposed are `pub(crate)`/private.
    - Performance: Adapter op for reading grammar bytes is invoked once per package load, not per parse.
    - Code Quality: New APIs have full frontmatter (stable id, name, key bindings/custom properties, usage, examples, permissions, backing Rust path, op wrapper, facade path, lookup tags); master-index linked; registry-generated; `cargo test` fails on missing entries.
    - Security: New APIs grant no filesystem/network/shell/raw-op authority beyond package-root-confined grammar/query byte reads; documented denials.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`, `clay-js-api-naming.md`, `clay-js-api-schema.md`.
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`, `2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`.
    - Chosen Approach:
      - Expose a minimal `clay.syntax.registerTreeSitterGrammarHandler` facade (and, if required, a package-root-confined `clay.syntax.readGrammarBytes` op) over the new `runtime/js/syntax.ts`; document and register them.
    - Files to Create/Edit:
      - `runtime/js/syntax.ts`, `src/server/js_runtime.rs` facade registration, `src/server/ops/syntax.rs` op wrapper if needed.
      - `docs/reference/clay-js-api/syntax/register-tree-sitter-grammar-handler.md` (and any companion op doc).
      - `docs/index.md` and generated registry entries.
    - References:
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
  - Test Cases to Write:
    - `clay_js_api_inventory`/`clay_js_doc_registry`/`clay_js_facade_layout` pass for any new API; a dedicated test asserting `registerTreeSitterGrammarHandler` resolves by stable id and tag.

- [ ] Update end-to-end manual smoke docs and fixtures for real cargo run highlighting
  - Acceptance Criteria:
    - Functional: `docs/development/launch-and-gui-smoke.md` documents the product `cargo run` + `~/.config/clay/init.js` workflow producing visible token-family highlighting for Rust/TypeScript/JavaScript/Markdown, the non-blocking open behavior (text first, decorations async), the diagnostic-on-error behavior, and the manual GNOME/Linux verification steps; the smoke fixture (`tests/fixtures/configuration/file-browser-workflow/init.js`) loads all four language packages.
    - Performance: Docs note parse runs background/async and budgets.
    - Code Quality: Distinguish the product cargo run path from the smoke fixture path.
    - Security: Docs repeat first-party-only grammar authority.
  - Approach:
    - Documentation Reviewed:
      - `docs/development/launch-and-gui-smoke.md` (Plan 044 "End-to-end file browser workflow smoke" and "Product cargo run configuration path" sections).
    - Chosen Approach:
      - Add a "Syntax highlighting" subsection to the product cargo run path; update the regression checklist to include rust/ts/js/markdown highlighting, non-blocking open, and diagnostic-on-error.
    - Files to Create/Edit:
      - `docs/development/launch-and-gui-smoke.md`, `tests/fixtures/configuration/file-browser-workflow/init.js`, `tests/manual_smoke_docs.rs` (extend the cargo-run-config-path assertion to include highlighting markers and the non-blocking open note).
    - References:
      - `docs/development/launch-and-gui-smoke.md`
  - Test Cases to Write:
    - `end_to_end_file_browser_workflow_smoke_covers_cargo_run_config_path` (existing) extended to assert highlighting + non-blocking-open markers.

- [ ] Run focused and full verification
  - Acceptance Criteria:
    - Functional: Linux gate passes: `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`. Focused suites: `parse_coordinator`, `connection`, `js_runtime`, `syntax_grammar`, `editor::surface`, `manual_smoke_docs`, `primitives_docs`, `clay_js_*`.
    - Performance: No new full-document IPC on the edit/render hot path; parse remains background/bounded; document open latency independent of parse.
    - Code Quality: No new clippy warnings; no fmt drift.
    - Security: No new package authority; first-party-only grammars; no raw ops user-facing.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`, `protocol-and-performance.md`.
    - Chosen Approach:
      - Run the Linux gate and focused suites; record counts.
    - Files to Create/Edit:
      - `plans/045-...md`: record execution notes and counts.
    - References:
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - No new tests; verification-only task.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete. New/changed areas documented: generic `web-tree-sitter` host adapter, grammar/query vendoring contract, non-blocking open parse, `finish_task` diagnostic channel, markdown decoration folding (preview untouched), and the non-tree-sitter package-JS fallback.
    - Performance: Wiki updates add no runtime work and document perf-relevant details (cached grammar/query, background parse, non-blocking open).
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, and links from the master wiki index.
    - Security: Wiki pages document the touched trust boundary (first-party-only grammar loader, package-root confinement, denied authority).
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`.
    - Chosen Approach:
      - After implementation and verification pass, update the Markdown code wiki once using `project-wiki`, including the master index and relevant pages (`syntax-highlighting-host-adapter.md`, `parse-coordinator.md`, `decoration-transport.md`, `server-ipc-skeleton.md`, `markdown-parser.md`, `syntax-grammar-registry.md`, `masonry-editor.md`).
    - Files to Create/Edit:
      - `docs/wiki/index.md`, `docs/wiki/modules/*.md`.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Manual wiki review: confirm the master index links relevant pages and updated pages explain the generic adapter, non-blocking open, and diagnostic channel.

## Compromises Made
- To be filled after tasks are completed and tests pass.

## Further Actions
- To be filled after task completion with improvements, rationale, and priority.