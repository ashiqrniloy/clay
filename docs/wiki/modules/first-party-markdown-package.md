# First-Party Markdown Package

## Source

- `packages/markdown/package.json`
- `packages/markdown/dist/index.js`
- `packages/markdown/dist/load.js`
- `packages/markdown/dist/parser.js`
- `packages/markdown/src/parser.js`
- `packages/markdown/dist/sdui.js`
- `packages/markdown/src/sdui.js`
- `packages/markdown/docs/index.md`
- `docs/reference/packages/markdown.md`
- `src/packages/record.rs`
- `src/server/connection.rs`
- `src/server/js_runtime.rs`
- `src/server/ops/sdui.rs`
- `src/server/sdui.rs`
- `tests/package_loading.rs`
- `tests/fixtures/markdown/sample.md`
- `tests/fixtures/configuration/markdown-mode/init.js`
- `tests/fixtures/configuration/markdown-mode/workspace/sample.md`
- `tests/fixtures/configuration/windows-markdown-open/init.js`
- `tests/fixtures/configuration/windows-markdown-open/workspace/sample.md`

## Overview

The Phase 18 Markdown POC now has a real first-party package scaffold at `packages/markdown/`. The package validates as `@clay/markdown` with API prefix `markdown`, declares the `markdown` major mode, lists `.md`, `.markdown`, `.mdown`, and `text/markdown` classification metadata, and exposes load/runtime JavaScript entry files without adding Markdown-specific Rust package-loading paths.

Phase 18.18 makes the compiled `tree-sitter-md-025` descriptor the default decoration engine. `packages/markdown/queries/highlights.scm` captures headings, standalone strong/emphasis/code/link forms, blocks, lists, and quotes; the vocabulary styleMap produces scope-less `TokenType` + `Modifiers` spans through the generic `TreeSitterSyntaxHandler` and `ParseCoordinator`. The manifest's default `decorations` contribution is now empty rather than pointing at `parser.js`. The package-owned Markdown parser/decorator adapter at `./dist/parser.js` remains registered as Tier 3 fallback data and retains its bounded markdown-it/scanner behavior, but native selection replaces that same package/mode handler before open-time parse. The unchanged package-owned SDUI preview/status adapter at `./dist/sdui.js` remains a separate package-JS path that builds inert Clay SDUI nodes without raw Deno ops or client-side script hooks.

Phase 18.5 (`plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md`) replanned Markdown end-user loading on top of generic shell/package/configuration primitives promoted in Phases 18.1–18.4. The package load path publishes no default side panel: the optional Markdown preview is a package-owned `clay:ui` `PanelContribution` targeting the `right` slot with `defaultVisibility: "hidden"`, shown only when the user enables it through documented configuration APIs (`setPackageOption` / `serverSetLayoutOverride`), never as a hard-coded left-sidebar fixture. The default Markdown editor is placed through the mandatory `main` slot of `PaneSlotLayout`. Mode activation, commands/key routing, parse handler registration, decoration publication, and user overrides all flow through generic primitives (`MajorModeActivation`, `CommandDeclaration`, behavior manifests, `serverRegisterParseHandler`, `serverPublishDecorations`, `PackageOwnedConfiguration`, `PackageLayoutOverride`) with no Markdown-specific Rust editor/parser/render/shell branch. The preferred one-line end-user setup remains `loadPackage("@clay/markdown")`, but that generic specifier resolver is deferred: the controlled server-side runtime is deny-by-default (`ClayModuleLoader`) and confines loadable modules to the configuration root (`canonical_local_file`), so a working one-line loader requires a security-critical module-loader bridge that warrants its own focused phase. The deferral is recorded in `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md`, and the package now ships the fallback entry shape the future resolver will invoke.

## How It Works

`packages/markdown/package.json` stays below `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES` so the existing manifest validator can accept it at enable/load time. The retained Clay metadata includes:

- Required entries: `./dist/index.js`, `./dist/load.js`, parser export `./dist/parser.js`, and SDUI export `./dist/sdui.js`.
- Required docs path: `./docs/index.md`.
- Required permissions: `mode-registration`, `mode-activation`, `command-registration`, `parse-document`, and `render-decorations`.
- API dependencies for mode registration/activation, command registration, parse handler registration, and decoration publication.
- Inert contribution descriptors for Markdown commands, client-first transform sketches, SDUI status metadata with an adapter path, Tier 1 native syntax grammar/query/styleMap metadata, mode patterns, and Tier 3 fallback parse-handler metadata; no default parser-backed decoration contribution.
- A package dependency on `markdown-it`; the adapter also accepts injected markdown-it tokens/parser instances so parser execution can be tested without changing Clay's decoration protocol shape.

`src/packages/record.rs` now validates that permission-bearing Clay JS API dependencies cannot be declared unless the package also declares their matching permission. This is generic validation, not a Markdown special case. For example, a package depending on `clay.parse.serverRegisterParseHandler` must declare `parse-document`, and one depending on `clay.decorations.serverPublishDecorations` must declare `render-decorations`.

`packages/markdown/dist/index.js` exports static package contract data, `markdownLargeFilePolicy`, `markdownPolicyForDocument()`, and `markdownPackageManifest()` so runtime loaders and tests share the same manifest shape. The policy is package-owned and deterministic: `<= 1 MiB` is `full`, `> 1 MiB` through `5 MiB` is `windowed`, `> 5 MiB` is large/windowed, timeout pressure is `degraded`, and budget exhaustion is `plain-text-fallback`; the Tier 3 fixed defaults remain `64 KiB` parse windows, `4 KiB` guard bytes, `30 MiB` syntax memory budget, and `5000 ms` timeout; the Tier 1 grammar contribution caps selected native windows at `4 KiB`. `packages/markdown/dist/load.js` exports `markdownPackageContract()` and `loadMarkdownPackage(clay, options)` for load-time registration through documented Clay JS facades: it validates the package, registers mode/command/completion/status metadata, and retains the markdown-it handler as `tier3-javascript-fallback`. `ClayRuntimeEvaluation` carries registered grammar metadata plus engine preferences. During generic open classification, `register_native_syntax_handler` selects the contribution by path, builds its query from compile-time-bundled package query text, and installs it under the selected grammar ID before scheduling background parse; the package/mode Tier 3 fallback remains separately keyed. The same module exports `markdownLoadMode(options = {})` as the package-owned alias for advanced/per-load options: it imports the `clay:packages`, `clay:modes`, `clay:commands`, and `clay:parse` facades directly (no caller-supplied `clay` object, no inline manifest), constructs the facade object, and reuses `loadMarkdownPackage`. It is re-exported from `./dist/index.js` so `import { markdownLoadMode } from "@clay/markdown"; await markdownLoadMode();` still resolves for advanced callers. The default user path is now `import { loadPackage } from "clay:packages"; await loadPackage("@clay/markdown");`, which invokes the same setup once on the persistent server runtime and reuses the registered mode/parse state for selected-file open. `packages/markdown/dist/parser.js` parses markdown-it block tokens and inline child-token streams, uses package-owned source/line indexes to derive viewport-bounded spans for ATX headings, strong/emphasis, inline code, fenced code blocks, and ordered/unordered list markers, then can publish them via `clay.decorations.serverPublishDecorations`. The adapter calls `markdownIt.parse(text, {})` on the supplied window text when the package dependency is installed, falls back to a tiny built-in token scanner for headings, fences, list markers, emphasis, strong, and inline code when `node_modules/markdown-it` is absent, or accepts injected token streams/parser objects for tests; it never calls `render` and never exposes markdown-it tokens, HTML, CSS, callbacks, or renderer state through Clay protocol shapes. Inline decoration ranges are recovered by walking markdown-it inline children against `token.content`; UTF-16 source offsets are converted to UTF-8 byte offsets through the package-owned source index and then offset by `absoluteByteStart` before inert spans are published. `parseMarkdownDecorations()` accepts `parseWindows` arrays from generic scheduler notifications, deduplicates overlapping window spans, and returns only Clay decoration shapes. When `fallbackMode`, `syntaxBudgetExceeded`, `memoryBudgetExceeded`, or an over-budget parse-window payload indicates budget pressure, `parseMarkdownDecorationUpdate()` returns an empty span list plus a `plain-text-fallback` status without invoking markdown-it. `packages/markdown/dist/sdui.js` exposes `markdownStatusForPolicy()`, `buildMarkdownPreviewStatusTree()`, and `publishMarkdownPreviewStatus()` helpers that compose a `Markdown Preview` panel, sanitized parse/decorations/highlighting status, a document-bound editor view, and a `markdown.togglePreview` button action from inert `clay:sdui` node helpers. The client still receives only validated inert data; no package JavaScript is needed for keypress, paint, scroll, layout, or text-event handlers.

The deterministic configuration fixture in `tests/fixtures/configuration/markdown-mode/init.js` exercises the full workflow without a package-manager process: it validates the package manifest, opens `workspace/sample.md` when a test supplies the workspace root, registers/activates Markdown mode, registers package commands and parse/decorations providers, publishes representative decorations, and publishes the Markdown preview/status SDUI tree. The fixture falls back to document `1` when no workspace root exists so manual `cargo run -- smoke-gui --config-fixture markdown-mode` stays deterministic and does not grant broader filesystem authority.

The Phase 19 development fixture in `tests/fixtures/configuration/windows-markdown-open/init.js` reuses the same first-party Markdown load/activation/decorations/status path and adds `bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" })`. Manual `cargo run -- smoke-gui --config-fixture windows-markdown-open` therefore exercises the selected-file dialog path with normal configuration APIs: the native command is inert manifest data until the user presses `Ctrl+O`, package JavaScript remains server-side, and the selected file still receives only a server-validated single-file grant.

Selected-file open now follows one generic path. `src/server/connection.rs::classify_open_document` loads/classifies package metadata, asks `ClayJsRuntimeService::register_native_syntax_handler` for the path-selected grammar, then registers package JS handlers only as fallback. `schedule_open_parse` returns immediately after enqueue; the coordinator later publishes the native `DecorationSet`. The optional preview is not coupled to this path: default package load publishes no panel, while explicit `registerMarkdownPreview()` and `packages/markdown/dist/sdui.js` continue through validated package UI/SDUI primitives.

## Invariants and Constraints

- Installing or recording `@clay/markdown` does not execute package JavaScript.
- Enabling/loading runs the existing Clay package validators and conflict checks only during package/configuration/document activation operations.
- The package does not request prohibited filesystem, network, shell, AI, raw Deno op, WASM, native-widget, package-enable, workspace-mutation, or client-side JavaScript authority.
- Markdown-specific behavior is not hard-coded into Rust package loading; Rust only gained generic API-dependency permission validation.
- The package manifest must remain bounded by `BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`; long style-token lists belong in parser/docs code, not manifest metadata, unless package metadata transport budgets are changed by a later decision/task.
- Markdown declares `defaultFontRole: "proportional"` through generic mode metadata. Parser output marks only `markup.inline-code` and `markup.code-block` spans with semantic `fontRole: "monospace"`; the native Tree-sitter query maps its generic `code` capture the same way. No Markdown branch exists in editor/server rendering.
- Default native spans carry direct prose vocabulary (`Heading1..6`, `Paragraph + Bold/Italic`, `CodeSpan`, `CodeBlock`, `ListItem`, `Link`, `Quote`) and builtin package provenance. Tier 3 fallback spans remain inert legacy-compatible syntax spans; neither path exposes parser nodes, CSS, callbacks, HTML, or renderer functions.
- Large-file parsing uses `parseWindows`/`parseWindow` metadata supplied by generic parse primitives. Window byte ranges must match the UTF-8 length of the supplied text, spans are clipped to the current viewport before publication, and off-window document text is neither indexed nor sent to `markdown-it` by the adapter.
- Package-owned SDUI actions must target commands that are already registered in the runtime command registry; disabling or invalidating the package removes enabled package records and the plain-text fallback manifest contains no `markdown.*` command/keybinding authority.
- Large-file policy is evaluated during load/open/reload/configuration or explicit viewport/policy refresh work. Keypress and paint handlers use already-installed behavior manifests and local decoration chunks only.
- Status diagnostics are fixed or sanitized package strings. Absolute paths, raw diagnostics, and document text are not embedded in Markdown SDUI status labels.
- Selected-file Markdown activation runs only after a user-selected server-authorized file open and only when the Markdown package has already contributed active Markdown commands; it does not load Markdown for arbitrary files or execute package JavaScript from keypress, paint, scroll, layout, or text-event handlers.
- The package load path (`loadMarkdownPackage` / `markdownLoadMode`) publishes no default fixed side panel. The optional preview is a package `PanelContribution` with `defaultVisibility: "hidden"` and is enabled only through documented `setPackageOption` / `serverSetLayoutOverride` calls; panels that appear in test fixtures or the `connection.rs` selected-file-open evaluation are not part of the default end-user load path.
- No Markdown-specific Rust editor/parser/render/shell branch exists. Markdown grammar data and query captures are static descriptor/package assets; generic Rust selector/handler code contains no Markdown mode branch. Package JS owns markers, commands, Tier 3 fallback, policy/status, and preview SDUI.

## Performance, Smoke, and Tests

Phase 18 keeps Markdown verification split between hard deterministic guards and advisory local benchmarks. `benches/markdown_baselines.rs` measures package activation/manifest selection, parse-update/decorations validation, and native decorated-editor render-adjacent work without running package JavaScript on client hot paths. `tools/bench/markdown-parser.mjs` measures Tier 3 fallback parser cost by building 1 MiB, 5 MiB, and 16 MiB corpora from the largest committed repository Markdown files and timing `markdown-it` plus package-adapter calls.

`docs/development/performance.md` records both the parser replacement rationale and active rewrite verification. Historical `mdast-util-from-markdown` full-document results were too slow for ordinary large-file editing (`fromMarkdown` ~1.28 s at 1 MiB, ~16.24 s at 5 MiB, and did not complete 16 MiB within 120 s), while `markdown-it` completed the same corpora much faster. After the package rewrite, the active harness completed local 1.01 MiB, 5.02 MiB, and 16.01 MiB repository-Markdown corpora in about 127.2/190.2 ms, 428.6/608.7 ms, and 1007.4/1577.8 ms for parser/adapter paths respectively. The former adapter's repeated byte-offset scanning was also infeasible for full documents (~49.3 s at 1 MiB), so durable large-file Markdown support should optimize the markdown-it adapter and viewport/range mapping rather than add client-side JavaScript or full-document IPC.

Run focused coverage with:

```text
cargo test --test package_loading
cargo test --test markdown_mode
cargo test --test performance_budgets
cargo bench --no-run
node --check tools/bench/markdown-parser.mjs
node tools/bench/markdown-parser.mjs --dry-run --sizes 1MiB --source-limit 8
npm install --prefix packages/markdown --no-save --no-package-lock --ignore-scripts markdown-it@^14.1.0
node --expose-gc tools/bench/markdown-parser.mjs --sizes 1MiB,5MiB,16MiB --parser markdown-it,adapter --iterations 1 --warmup 0
cargo test markdown_package_runtime_loads_markdown_it_workflow --lib
cargo test markdown_parser_adapter_publishes_viewport_bounded_decorations --lib
```

Relevant tests:

- `markdown_decoration_renders_through_tier1_native_engine`
- `markdown_preview_sdui_panel_remains_package_js_and_unchanged`
- `markdown_decoration_and_preview_are_independently_activatable`
- `server::connection::tests::default_init_js_load_package_powers_selected_markdown_open` (asserts builtin native provenance)
- `markdown_package_contract_validates_with_required_metadata`
- `markdown_package_rejects_missing_required_permissions`
- `markdown_package_does_not_execute_on_install`
- `markdown_package_docs_path_is_required_and_resolvable`
- `markdown_package_has_no_mdast_dependency`
- `markdown_runtime_code_has_no_from_markdown_import`
- `markdown_parser_adapter_uses_markdown_it_package_boundary`
- `markdown_parser_adapter_publishes_protocol_spans_without_parser_data`
- `markdown_it_adapter_has_token_stream_range_fixtures`
- `server::js_runtime::tests::markdown_package_runtime_loads_markdown_it_workflow`
- `server::js_runtime::tests::windows_markdown_open_config_fixture_loads_markdown_and_binds_ctrl_o`
- `windows_markdown_open_fixture_binds_ctrl_o_without_hardcoding`
- `windows_markdown_open_fixture_loads_markdown_package`
- `server::connection::tests::selected_markdown_file_publishes_manifest_decorations_and_status`
- `server::connection::tests::markdown_open_runtime_uses_bounded_parse_window_for_large_file`
- `markdown_package_declares_sdui_preview_status_adapter`
- `markdown_sdui_status_reports_markdown_it_parse_state`
- `markdown_disabled_falls_back_to_plain_text_after_rewrite`
- `markdown_invalid_package_reports_sanitized_diagnostics`
- `markdown_behavior_manifest_fits_budget`
- `markdown_parse_and_decoration_payloads_fit_budgets`
- `markdown_reload_reinstalls_manifest_and_decorations`
- `markdown_typing_does_not_wait_for_markdown_it_parse`
- `markdown_it_adapter_large_fixture_span_counts_are_stable`
- `markdown_structural_sdui_snapshot_matches_fixture`
- `server::js_runtime::tests::markdown_parser_adapter_publishes_viewport_bounded_decorations`
- `server::js_runtime::tests::markdown_windowed_adapter_offsets_ranges_to_absolute_document_bytes`
- `server::js_runtime::tests::markdown_windowed_adapter_does_not_parse_full_large_document`
- `server::js_runtime::tests::markdown_windowed_adapter_preserves_fence_and_list_context`
- `server::js_runtime::tests::markdown_large_file_status_reports_windowed_highlighting`
- `server::js_runtime::tests::markdown_large_file_budget_exhaustion_falls_back_to_plain_text`
- `server::js_runtime::tests::markdown_degraded_status_contains_no_document_text_or_paths`
- `markdown_large_file_policy_declares_thresholds_and_states`
- `markdown_large_file_status_reports_windowed_highlighting`
- `markdown_large_file_budget_exhaustion_falls_back_to_plain_text_static_guard`
- `markdown_degraded_status_contains_no_document_text_or_paths_static_guard`
- `markdown_windowed_adapter_declares_bounded_parse_policy`
- `markdown_windowed_adapter_static_guards_reject_full_text_large_file_path`
- `server::js_runtime::tests::markdown_config_fixture_opens_workspace_and_publishes_status_sdui`

## Related

- [Phase 18.5 Markdown Replan Primitive Review](phase18.5-markdown-replan-primitive-review.md)
- [Package Loading](package-loading.md)
- [Package Primitive Gate](package-primitive-gate.md)
- [Mode Registry](mode-registry.md)
- [Command Registry](command-registry.md)
- [Parse Coordinator](parse-coordinator.md)
- [Decoration Transport](decoration-transport.md)
- `docs/reference/packages/markdown.md`
- `packages/markdown/docs/index.md`
