use clay::perf::budgets::{
    AGENT_DAEMON_SPAWN_P95_BUDGET_MS, AGENT_DELTA_IPC_P95_BUDGET_MS,
    AGENT_PROMPT_TO_FIRST_DELTA_P95_BUDGET_MS, BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES,
    CLIENT_EDIT_PAYLOAD_BUDGET_BYTES, COMMAND_CENTRE_FILTER_UPDATE_P95_BUDGET_MS,
    COMMAND_CENTRE_OPEN_P95_BUDGET_MS, COMPLETION_MAX_VISIBLE_ROWS, COMPLETION_MAX_WIDTH_PX,
    COMPLETION_RESULT_MAX_ITEMS, COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES,
    DECORATION_PAYLOAD_BUDGET_BYTES, EDIT_ACK_P95_BUDGET_MS, EDIT_ACK_PAYLOAD_BUDGET_BYTES,
    FOLDING_RANGE_PAYLOAD_BUDGET_BYTES, INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES,
    INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES, KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS,
    LANGUAGE_INTELLIGENCE_MAX_HOVER_MARKDOWN_CHARS, LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB,
    MULTI_PANE_DECORATION_AGGREGATE_BUDGET_BYTES, PANE_PAINT_P95_BUDGET_MS,
    RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS, SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS,
    SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES, SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
    SYNTAX_CACHE_BUDGET_BYTES, TAB_SWITCH_P95_BUDGET_MS,
};

fn performance_doc() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/development/performance.md"
    ))
    .expect("read docs/development/performance.md")
}

fn ui_observability_doc() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/development/ui-observability.md"
    ))
    .expect("read docs/development/ui-observability.md")
}

fn phase18_plan_doc() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/plans/020-Phase18-Markdown-Mode-Package-Proof-of-Concept.md"
    ))
    .expect("read Phase 18 Markdown mode plan")
}

fn phase18_5_plan_doc() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/plans/021-Phase18.5-Large-File-Markdown-Performance-and-Memory.md"
    ))
    .expect("read Phase 18.5 large-file Markdown plan")
}

fn performance_fixtures_wiki_doc() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/wiki/modules/performance-fixtures.md"
    ))
    .expect("read performance fixtures wiki")
}

#[test]
fn performance_docs_list_all_supported_benchmark_commands() {
    let doc = performance_doc();

    for command in [
        "cargo bench",
        "cargo bench --no-run",
        "cargo bench --bench editor_baselines editor_visible_extraction -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "cargo bench --bench protocol_server_baselines -- --save-baseline phase14-baseline",
        "cargo bench --bench protocol_server_baselines -- --baseline phase14-baseline",
        "cargo bench --bench protocol_server_baselines -- --baseline-lenient phase14-baseline",
        "cargo bench --bench markdown_baselines markdown_activation_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "cargo bench --bench markdown_baselines markdown_parse_and_decoration_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "node --check tools/bench/markdown-parser.mjs",
        "node tools/bench/markdown-parser.mjs --dry-run --sizes 1MiB --source-limit 8",
        "node --expose-gc tools/bench/markdown-parser.mjs --sizes 64KiB,256KiB,1MiB,5MiB,16MiB --parser markdown-it,adapter,windowed-adapter --iterations 1 --warmup 0 --json",
        "cargo test --test protocol performance_protocol::",
        "cargo test --test runtime lsp_bridge::",
        "cargo test --test security language_server_authority::",
        "cargo bench --bench first_party_language_baselines -- --save-baseline pre-lsp",
        "cargo bench --bench window_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "cargo bench --bench window_baselines responsive_layout_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "cargo bench --bench window_baselines completion_open_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "cargo bench --bench window_baselines completion_filter_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "cargo bench --bench window_baselines command_centre_open_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "cargo bench --bench window_baselines completion_selection_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "cargo bench --bench window_baselines accessibility_tree_update_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "cargo bench --bench window_baselines completion_layout_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
    ] {
        assert!(
            doc.contains(command),
            "performance guide must document benchmark/profiling command: {command}"
        );
    }
}

#[test]
fn performance_budget_payload_constants_match_docs() {
    let doc = performance_doc();

    for expected in [
        format!("<= {} bytes", CLIENT_EDIT_PAYLOAD_BUDGET_BYTES),
        format!("<= {} bytes", EDIT_ACK_PAYLOAD_BUDGET_BYTES),
        format!("<= {} bytes", BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES),
        format!("<= {} bytes", SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES),
        format!("<= {} bytes", SDUI_UPDATE_PAYLOAD_BUDGET_BYTES),
        format!(
            "<= {} ms (P95, advisory)",
            KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS
        ),
        format!("<= {} ms (P95, advisory)", EDIT_ACK_P95_BUDGET_MS),
        format!(
            "<= {} ms (P95, advisory)",
            SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS
        ),
        format!(
            "<= {} ms (P95, advisory)",
            RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS
        ),
        format!(
            "<= {} MiB (advisory)",
            LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB
        ),
    ] {
        assert!(
            doc.contains(&expected),
            "performance guide is missing budget marker `{expected}`"
        );
    }

    for security_requirement in [
        "must not expose document contents",
        "must not expose secrets",
        "must not open network listeners",
        "must not grant shell authority",
        "must not execute arbitrary JavaScript in the client",
    ] {
        assert!(
            doc.contains(security_requirement),
            "performance guide must preserve security boundary: {security_requirement}"
        );
    }
}

#[test]
fn markdown_performance_verification_is_documented() {
    let doc = performance_doc();
    for expected in [
        "Markdown mode verification",
        "markdown_baselines",
        "`markdown-it` token-stream adapter",
        "markdown_behavior_manifest_fits_budget",
        "markdown_parse_and_decoration_payloads_fit_budgets",
        "markdown_typing_does_not_wait_for_markdown_it_parse",
        "markdown_it_adapter_large_fixture_span_counts_are_stable",
        "# Hé 🦀",
        "Large-file parser recommendation",
        "Active markdown-it benchmark verification (2026-06-04)",
    ] {
        assert!(
            doc.contains(expected),
            "performance guide must record Markdown verification evidence marker `{expected}`"
        );
    }
}

#[test]
fn markdown_plan_references_markdown_it_rewrite_decision() {
    let plan = phase18_plan_doc();

    for expected in [
        "# Phase 18: Markdown Mode Package Proof of Concept — markdown-it Rewrite",
        "decision-logs/2026-06-04-1923-replace-markdown-parser-with-markdown-it-and-primitive-first-mode-planning.md",
        "Superseded mdast start decision",
        "Rewrite Phase 18 around markdown-it and primitive-first package work: selected",
        "Clean up mdast implementation, dependencies, tests, docs, and stale references",
        "markdown-it is the active implementation target and mdast is cleanup scope",
    ] {
        assert!(
            plan.contains(expected),
            "Phase 18 plan must record markdown-it rewrite evidence marker `{expected}`"
        );
    }

    for rejected_active_mdast_marker in [
        "Treat markdown-it as a follow-up spike only: selected",
        "Keep mdast for small documents and add markdown-it for large documents: selected",
        "mdast is the active implementation target",
    ] {
        assert!(
            !plan.contains(rejected_active_mdast_marker),
            "Phase 18 plan must not describe mdast as active parser via `{rejected_active_mdast_marker}`"
        );
    }
}

#[test]
fn markdown_performance_docs_record_parser_replacement_reason() {
    let doc = performance_doc();

    for expected in [
        "Large-file parser recommendation",
        "do **not** treat full-document `mdast-util-from-markdown` parsing as proven",
        "`markdown-it` parse",
        "66.528 ms",
        "397.630 ms",
        "849.659 ms",
        "Do not add full-document parser IPC or client-side JavaScript to compensate.",
    ] {
        assert!(
            doc.contains(expected),
            "performance guide must retain parser replacement evidence marker `{expected}`"
        );
    }
}

#[test]
fn markdown_benchmark_docs_record_markdown_it_results() {
    let doc = performance_doc();

    for expected in [
        "Active markdown-it benchmark verification (2026-06-04)",
        "Node v26.2.0",
        "largest committed repository Markdown files repeated to requested sizes",
        "1.01 MiB",
        "127.234 ms",
        "190.213 ms",
        "5.02 MiB",
        "428.597 ms",
        "608.680 ms",
        "16.01 MiB",
        "1007.381 ms",
        "1577.844 ms",
        "231,008 spans",
    ] {
        assert!(
            doc.contains(expected),
            "performance guide must record active markdown-it benchmark marker `{expected}`"
        );
    }
}

#[test]
fn markdown_large_file_performance_contract_documents_overhead_budget() {
    let doc = performance_doc();
    let wiki = performance_fixtures_wiki_doc();
    let plan = phase18_5_plan_doc();

    for expected in [
        "Large-file Markdown editor-parity contract (Phase 18.5)",
        "The 30 MiB target applies to **Markdown-specific overhead only**, not total process RSS",
        "markdown_overhead <= 30 MiB",
        "baseline_rss",
        "document_memory",
        "markdown_parser_temporary_allocations",
        "retained_decoration_cache_memory",
        "total_rss",
    ] {
        assert!(
            doc.contains(expected),
            "performance guide must document large-file memory contract marker `{expected}`"
        );
    }

    for expected in [
        "markdown_overhead <= 30 MiB",
        "whole-process 30 MiB cap",
        "baseline_rss",
        "document_memory",
        "retained_decoration_cache_memory",
    ] {
        assert!(
            wiki.contains(expected),
            "performance fixture wiki must document large-file memory accounting marker `{expected}`"
        );
    }

    for expected in [
        "not total process RSS below 30 MiB",
        "Markdown parsing/decoration overhead at or below 30 MiB",
        "What is possible and should be implemented",
    ] {
        assert!(
            plan.contains(expected),
            "Phase 18.5 plan must preserve feasibility marker `{expected}`"
        );
    }
}

#[test]
fn markdown_large_file_contract_rejects_full_document_hot_path() {
    let doc = performance_doc();
    let wiki = performance_fixtures_wiki_doc();

    for expected in [
        "ordinary open, edit, and scroll paths must not run full-document parse/decorate",
        "Markdown parser delay may only affect decoration freshness",
        "Full-document work is allowed only as cancellable idle/background validation",
        "Large Markdown files (`> 5 MiB`, including the 16 MiB target)",
    ] {
        assert!(
            doc.contains(expected),
            "performance guide must reject full-document large-file hot path marker `{expected}`"
        );
    }

    for expected in [
        "large files (`> 5 MiB`) must not use full-document parse/decorate on ordinary open, edit, or scroll paths",
        "typing/local paint",
        "visible decoration refresh",
        "parser cancellation",
    ] {
        assert!(
            wiki.contains(expected),
            "performance fixture wiki must record editor-comparison target marker `{expected}`"
        );
    }
}

#[test]
fn markdown_benchmark_reports_baseline_document_and_markdown_overhead() {
    let doc = performance_doc();
    let wiki = performance_fixtures_wiki_doc();

    for expected in [
        "Benchmark JSON for large-file Markdown work must expose separate",
        "`total_rss`",
        "`baseline_rss`",
        "`document_memory`",
        "`markdown_parser_temporary_allocations`",
        "`retained_decoration_cache_memory`",
        "`markdown_overhead`",
    ] {
        assert!(
            doc.contains(expected),
            "performance guide must require benchmark JSON memory category marker `{expected}`"
        );
    }

    for expected in [
        "Benchmark JSON for future large-file Markdown runs must separate",
        "`total_rss`",
        "`baseline_rss`",
        "`document_memory`",
        "`markdown_parser_temporary_allocations`",
        "`retained_decoration_cache_memory`",
        "`markdown_overhead`",
    ] {
        assert!(
            wiki.contains(expected),
            "performance fixture wiki must require benchmark JSON memory category marker `{expected}`"
        );
    }
}

#[test]
fn markdown_windowed_benchmark_uses_real_parser_and_repo_corpus() {
    let script = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/bench/markdown-parser.mjs"
    ))
    .expect("read tools/bench/markdown-parser.mjs");
    for expected in [
        "markdownIt.parse(corpus.text, {})",
        "DEFAULT_SIZES = ['64KiB', '256KiB', '1MiB', '5MiB', '16MiB']",
        "DEFAULT_PARSERS = ['markdown-it', 'adapter', 'windowed-adapter']",
        "parseMarkdownDecorations",
        "parseWindows: [parseWindow]",
        "largest committed repository Markdown files",
        "no dummy prose generated",
        "EXCLUDED_DIRS",
    ] {
        assert!(
            script.contains(expected),
            "Markdown parser benchmark script must include marker `{expected}`"
        );
    }
}

#[test]
fn markdown_benchmark_json_reports_editor_parity_categories() {
    let script = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/bench/markdown-parser.mjs"
    ))
    .expect("read tools/bench/markdown-parser.mjs");
    let doc = performance_doc();
    let wiki = performance_fixtures_wiki_doc();

    for expected in [
        "parser_full_document_advisory",
        "adapter_windowed_viewport",
        "status_fallback_policy",
        "parserInputBytes",
        "hotPathAllowed",
        "total_rss",
        "baseline_rss",
        "document_memory",
        "markdown_parser_temporary_allocations",
        "retained_decoration_cache_memory",
        "markdown_overhead",
        "markdown_overhead_budget_met",
    ] {
        assert!(
            script.contains(expected),
            "Markdown benchmark JSON must include editor-parity category marker `{expected}`"
        );
    }

    for expected in [
        "windowed-adapter",
        "64KiB,256KiB,1MiB,5MiB,16MiB",
        "parser, adapter, transport, render-adjacent, status/fallback, and memory categories",
    ] {
        assert!(
            doc.contains(expected) || wiki.contains(expected),
            "docs/wiki must record benchmark extension marker `{expected}`"
        );
    }
}

#[test]
fn markdown_large_file_memory_overhead_fits_budget() {
    let script = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/bench/markdown-parser.mjs"
    ))
    .expect("read tools/bench/markdown-parser.mjs");
    assert_eq!(SYNTAX_CACHE_BUDGET_BYTES, 30 * 1024 * 1024);
    assert!(script.contains("MARKDOWN_OVERHEAD_BUDGET_BYTES = 30 * 1024 * 1024"));
    assert!(script.contains("markdownOverhead <= MARKDOWN_OVERHEAD_BUDGET_BYTES"));
    assert!(performance_doc().contains("windowed-adapter markdown_overhead"));
}

#[test]
fn markdown_full_document_adapter_is_not_large_file_hot_path() {
    let script = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/bench/markdown-parser.mjs"
    ))
    .expect("read tools/bench/markdown-parser.mjs");
    for expected in [
        "adapter_full_document_advisory",
        "large-file full-document adapter is advisory only and not an ordinary hot path",
        "hotPathAllowed",
        "corpus.bytes <= SMALL_FILE_THRESHOLD_BYTES",
    ] {
        assert!(
            script.contains(expected),
            "benchmark script must reject full-document large-file hot paths via marker `{expected}`"
        );
    }
}

#[test]
fn markdown_ui_observability_uses_structural_snapshots() {
    let doc = ui_observability_doc();
    for expected in [
        "Markdown Preview",
        "markdown_structural_sdui_snapshot_matches_fixture",
        "GPU-backed pixel snapshots remain deferred",
    ] {
        assert!(
            doc.contains(expected),
            "UI observability guide must document Markdown structural regression marker `{expected}`"
        );
    }
}

/// Confirms that Phase 14 profiling/benchmark activation is developer-only
/// and does not require a public Clay JS configuration API.
/// This test acts as a policy guard: if profiling is promoted to a stable
/// user-facing feature in a future phase, remove this test and add a
/// Clay JS API doc, inventory entry, and registry entry instead.
#[test]
fn no_public_configuration_needed_for_internal_perf_hooks() {
    // The profiling activation is CLAY_PERF_PROFILE env var and --profile-perf
    // CLI flag, which are developer-only paths documented in
    // docs/development/performance.md. They are NOT Clay JS APIs.
    //
    // Verify: performance doc describes the activation mechanism as
    // developer-only, not as user configuration.
    let doc = performance_doc();
    assert!(
        doc.contains("CLAY_PERF_PROFILE") && doc.contains("--profile-perf"),
        "performance guide must document the developer-only activation paths"
    );
    assert!(
        doc.contains("internal") && (doc.contains("developer") || doc.contains("opt-in")),
        "performance guide must describe profiling hooks as internal/developer-only"
    );
}
/// reference so that docs and code stay aligned at compile time.
#[test]
fn performance_budget_constants_are_exported() {
    // If a constant is removed or renamed the import at the top of this file
    // will fail to compile, making this an implicit compile-time guard.
    assert_eq!(CLIENT_EDIT_PAYLOAD_BUDGET_BYTES, 512);
    assert_eq!(EDIT_ACK_PAYLOAD_BUDGET_BYTES, 128);
    // Plan 046: raised from 2048 to 4096 to fit first-party theme packages'
    // full inert `textStyles` mappings (~2.3 KB) alongside their manifest.
    // Plan 061 task 8: raised to 8192 to also fit versioned
    // `clay.extensionPoints` declarations (largest: @clay/markdown ~5.5 KB).
    assert_eq!(BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, 8192);
    assert_eq!(SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES, 4096);
    assert_eq!(SDUI_UPDATE_PAYLOAD_BUDGET_BYTES, 1024);
    assert_eq!(KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS, 16);
    assert_eq!(EDIT_ACK_P95_BUDGET_MS, 40);
    assert_eq!(SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS, 16);
    assert_eq!(RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS, 25);
    assert_eq!(LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB, 256);
    assert_eq!(SYNTAX_CACHE_BUDGET_BYTES, 30 * 1024 * 1024);
    assert_eq!(COMPLETION_MAX_VISIBLE_ROWS, 8);
    assert_eq!(COMPLETION_MAX_WIDTH_PX, 480.0);
}

#[test]
fn plan087_completion_surface_budgets_are_documented() {
    let doc = performance_doc();
    for expected in [
        "COMPLETION_MAX_VISIBLE_ROWS".to_string(),
        "COMPLETION_MAX_WIDTH_PX".to_string(),
        format!("{} visible rows", COMPLETION_MAX_VISIBLE_ROWS),
        format!("{} logical px", COMPLETION_MAX_WIDTH_PX),
    ] {
        assert!(
            doc.contains(&expected),
            "performance guide must document completion budget marker `{expected}`"
        );
    }
}

#[test]
fn plan087_focused_ui_regression_coverage_is_documented() {
    let doc = performance_doc();
    for expected in [
        "Plan 087 focused UI regression coverage",
        "completion_open_baselines",
        "completion_filter_baselines",
        "completion_layout_baselines",
        "completion_projection_is_bounded_and_stays_out_of_paint",
        "completion_result_rejects_foreign_document_and_behavior_provenance",
        "accesskit_consumer",
        "No pixel goldens",
        "wall-clock results remain local/advisory",
    ] {
        assert!(
            doc.contains(expected),
            "performance guide must document Plan 087 regression marker `{expected}`"
        );
    }
}

#[test]
fn phase22_6_window_budget_constants_are_pinned_and_documented() {
    // Pinned from `cargo bench --bench window_baselines` on 2026-08-08:
    // pane_paint_baselines 1/2/4 panes ≈ 69/382/743 ns; tab_switch_baselines
    // 1/2/4 panes ≈ 88/389/807 ns. Advisory ms ceilings with ~1000x headroom
    // (debug builds, AT overhead); hard CI promotion follows the Phase 21
    // stable-runner rule.
    assert_eq!(PANE_PAINT_P95_BUDGET_MS, 1);
    assert_eq!(TAB_SWITCH_P95_BUDGET_MS, 1);
    assert_eq!(
        MULTI_PANE_DECORATION_AGGREGATE_BUDGET_BYTES,
        4 * DECORATION_PAYLOAD_BUDGET_BYTES
    );
    const { assert!(MULTI_PANE_DECORATION_AGGREGATE_BUDGET_BYTES >= 4 * 8192) };

    let doc = performance_doc();
    for expected in [
        format!("<= {} ms (P95, advisory)", PANE_PAINT_P95_BUDGET_MS),
        format!("<= {} ms (P95, advisory)", TAB_SWITCH_P95_BUDGET_MS),
        format!("<= {} bytes", MULTI_PANE_DECORATION_AGGREGATE_BUDGET_BYTES),
        "cargo bench --bench window_baselines".to_string(),
    ] {
        assert!(
            doc.contains(&expected),
            "performance guide must document Phase 22.6 budget marker `{expected}`"
        );
    }
}

#[test]
fn phase25_agent_host_budget_constants_are_pinned_and_documented() {
    assert_eq!(AGENT_DAEMON_SPAWN_P95_BUDGET_MS, 2_000);
    assert_eq!(AGENT_PROMPT_TO_FIRST_DELTA_P95_BUDGET_MS, 2_000);
    assert_eq!(AGENT_DELTA_IPC_P95_BUDGET_MS, 4);
    const { assert!(AGENT_DELTA_IPC_P95_BUDGET_MS < KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS) };
    assert_eq!(COMMAND_CENTRE_OPEN_P95_BUDGET_MS, 50);
    assert_eq!(COMMAND_CENTRE_FILTER_UPDATE_P95_BUDGET_MS, 4);

    let doc = performance_doc();
    for expected in [
        "## Phase 25 agent-host budgets",
        "AGENT_DAEMON_SPAWN_P95_BUDGET_MS",
        "AGENT_PROMPT_TO_FIRST_DELTA_P95_BUDGET_MS",
        "AGENT_DELTA_IPC_P95_BUDGET_MS",
        "AGENT_TRANSCRIPT_SNAPSHOT_BUDGET_BYTES",
        "slow_daemon_submit_does_not_block_caller",
        "agent_io_stays_off_paint_and_keypress",
        "Deltas never block",
    ] {
        assert!(
            doc.contains(expected),
            "performance guide must document Phase 25 budget marker {expected:?}"
        );
    }
}

#[test]
fn plan088_responsive_layout_conformance_and_baseline_are_documented() {
    let doc = performance_doc();
    for expected in [
        "Plan 088 modernization conformance and responsive layout baselines",
        "responsive_layout_baselines",
        "responsive_layout_work",
        "responsive_layout_work_preserves_sidebar_and_editor_bounds",
        "Screenshot goldens remain deferred",
        "cargo bench --bench window_baselines responsive_layout_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
    ] {
        assert!(
            doc.contains(expected),
            "performance guide must document Plan 088 guard/baseline marker `{expected}`"
        );
    }
}

#[test]
fn plan089_editor_menu_completion_and_accessibility_costs_are_documented() {
    let doc = performance_doc();
    let wiki = performance_fixtures_wiki_doc();
    for expected in [
        "Plan 089 editor, menu, tab, completion, and accessibility cost guards",
        "command_centre_open_baselines",
        "completion_selection_baselines",
        "accessibility_tree_update_baselines",
        "accessibility_updates_reuse_stable_virtual_ids_without_allocator_churn",
        "retained_accessibility_update_fixture_stays_bounded",
        "Criterion's saved-target comparisons remain advisory",
        "existing `editor_baselines` `editor_render_adjacent`",
    ] {
        assert!(
            doc.contains(expected) || wiki.contains(expected),
            "performance docs/wiki must document Plan 089 cost marker `{expected}`"
        );
    }
}

#[test]
fn phase28_budget_constants_match_docs() {
    assert_eq!(FOLDING_RANGE_PAYLOAD_BUDGET_BYTES, 2048);
    assert_eq!(INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES, 4096);
    assert_eq!(
        INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES,
        INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES + FOLDING_RANGE_PAYLOAD_BUDGET_BYTES
    );
    assert_eq!(DECORATION_PAYLOAD_BUDGET_BYTES, 8192);
    assert_eq!(COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES, 16 * 1024);
    assert_eq!(COMPLETION_RESULT_MAX_ITEMS, 256);
    assert_eq!(LANGUAGE_INTELLIGENCE_MAX_HOVER_MARKDOWN_CHARS, 4096);
    assert_eq!(KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS, 16);
    const {
        assert!(KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS == 16);
        assert!(FOLDING_RANGE_PAYLOAD_BUDGET_BYTES < DECORATION_PAYLOAD_BUDGET_BYTES);
    }

    let doc = performance_doc();
    for expected in [
        "Phase 28.7".to_string(),
        "FOLDING_RANGE_PAYLOAD_BUDGET_BYTES".to_string(),
        "INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES".to_string(),
        format!("<= {} bytes", FOLDING_RANGE_PAYLOAD_BUDGET_BYTES),
        format!(
            "{} B when",
            INCREMENTAL_PARSE_UPDATE_WITH_FOLDING_BUDGET_BYTES
        ),
        format!("<= {} bytes", DECORATION_PAYLOAD_BUDGET_BYTES),
        format!("<= {} bytes", COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES),
        format!(
            "<= {} ({})",
            COMPLETION_RESULT_MAX_ITEMS, "COMPLETION_RESULT_MAX_ITEMS"
        ),
        format!(
            "<= {} chars",
            LANGUAGE_INTELLIGENCE_MAX_HOVER_MARKDOWN_CHARS
        ),
        format!(
            "<= {} ms (P95, advisory)",
            KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS
        ),
        "phase28_budget_constants_match_docs".to_string(),
        "folding_and_inlay_payloads_deny_above_cap".to_string(),
        "completion_ranking_stays_inside_existing_scan_budget".to_string(),
    ] {
        assert!(
            doc.contains(&expected),
            "performance guide must document Phase 28.7 budget marker `{expected}`"
        );
    }
}

#[test]
fn folding_and_inlay_payloads_deny_above_cap() {
    let decorations = production_src("src/server/decorations.rs");
    assert!(
        decorations.contains("DECORATION_PAYLOAD_BUDGET_BYTES"),
        "inlay/link reuse decoration publication; must keep the named cap"
    );
    assert!(
        decorations.contains("PayloadBudgetExceeded"),
        "oversized decoration (inlay/link) must deny, not truncate"
    );

    let intelligence = production_src("src/server/language_intelligence.rs");
    assert!(
        intelligence.contains("LANGUAGE_INTELLIGENCE_MAX_HOVER_MARKDOWN_CHARS"),
        "hover payloads must keep the named markdown cap"
    );

    for path in ["src/protocol", "src/server", "src/editor"] {
        for rust_file in rust_sources_under(path) {
            let source = std::fs::read_to_string(&rust_file)
                .unwrap_or_else(|error| panic!("read {}: {error}", rust_file.display()));
            let body = production_body(&source);
            if mentions_folding_publish(body) {
                assert!(
                    body.contains("FOLDING_RANGE_PAYLOAD_BUDGET_BYTES"),
                    "{} publishes folding ranges without the named cap",
                    rust_file.display()
                );
            }
        }
    }
}

#[test]
fn completion_ranking_stays_inside_existing_scan_budget() {
    let completion = production_src("src/server/completion.rs");
    assert!(
        completion.contains("COMPLETION_RESULT_MAX_ITEMS"),
        "buffer-word scan / ranking must keep the existing item cap"
    );
    assert!(
        completion.contains("COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES"),
        "buffer-word scan / ranking must keep the existing payload cap"
    );
    assert!(
        completion.contains("estimated_result_payload_bytes"),
        "scan must stop before exceeding the payload estimate"
    );
    for line in completion.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("fn ") && function_name_looks_like_rank(trimmed) {
            let name = trimmed.split('(').next().unwrap_or(trimmed);
            assert!(
                completion.contains("COMPLETION_RESULT_MAX_ITEMS"),
                "{name} must stay inside COMPLETION_RESULT_MAX_ITEMS"
            );
        }
    }
}

fn production_src(path: &str) -> String {
    let source = std::fs::read_to_string(concat_manifest(path))
        .unwrap_or_else(|error| panic!("read {path}: {error}"));
    production_body(&source).to_string()
}

fn concat_manifest(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn production_body(src: &str) -> &str {
    if let Some(index) = src.find("\nmod tests") {
        return &src[..index];
    }
    if let Some(index) = src.find("\n#[cfg(test)]") {
        return &src[..index];
    }
    src
}

fn mentions_folding_publish(body: &str) -> bool {
    body.contains("FoldingRange")
        || body.contains("publish_folding")
        || body.contains("serverPublishFoldingRanges")
}

fn function_name_looks_like_rank(fn_line: &str) -> bool {
    let name = fn_line
        .trim_start_matches("fn ")
        .split(['<', '('])
        .next()
        .unwrap_or("");
    name.contains("rank") || name.contains("score")
}

fn rust_sources_under(dir: &str) -> Vec<std::path::PathBuf> {
    let root = concat_manifest(dir);
    let mut stack = vec![root];
    let mut files = Vec::new();
    while let Some(path) = stack.pop() {
        let entries = std::fs::read_dir(&path)
            .unwrap_or_else(|error| panic!("read_dir {}: {error}", path.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|error| panic!("dir entry: {error}"));
            let child = entry.path();
            if child.is_dir() {
                stack.push(child);
            } else if child.extension().is_some_and(|ext| ext == "rs") {
                files.push(child);
            }
        }
    }
    files
}
