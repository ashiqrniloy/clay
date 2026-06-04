use clay::perf::budgets::{
    BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, CLIENT_EDIT_PAYLOAD_BUDGET_BYTES,
    EDIT_ACK_P95_BUDGET_MS, EDIT_ACK_PAYLOAD_BUDGET_BYTES, KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS,
    LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB, RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS,
    SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS, SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES,
    SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
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

#[test]
fn performance_docs_list_all_supported_benchmark_commands() {
    let doc = performance_doc();

    for command in [
        "cargo bench",
        "cargo bench --no-run",
        "cargo bench --bench editor_baselines editor_visible_extraction -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "cargo bench --benches -- --save-baseline phase14-baseline",
        "cargo bench --benches -- --baseline phase14-baseline",
        "cargo bench --benches -- --baseline-lenient phase14-baseline",
        "cargo bench --bench markdown_baselines markdown_activation_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "cargo bench --bench markdown_baselines markdown_parse_and_decoration_baselines -- --sample-size 10 --warm-up-time 1 --measurement-time 2",
        "node --check tools/bench/markdown-parser.mjs",
        "node tools/bench/markdown-parser.mjs --dry-run --sizes 1MiB --source-limit 8",
        "node --expose-gc tools/bench/markdown-parser.mjs --sizes 1MiB,5MiB,16MiB --parser markdown-it,adapter --iterations 1 --warmup 0",
        "cargo test --test performance_protocol",
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
fn markdown_it_parser_benchmark_script_uses_real_parser_and_repo_corpus() {
    let script = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tools/bench/markdown-parser.mjs"
    ))
    .expect("read tools/bench/markdown-parser.mjs");
    for expected in [
        "markdownIt.parse(text, {})",
        "DEFAULT_PARSERS = ['markdown-it', 'adapter']",
        "parseMarkdownDecorations",
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
    assert_eq!(EDIT_ACK_PAYLOAD_BUDGET_BYTES, 112);
    assert_eq!(BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, 2048);
    assert_eq!(SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES, 4096);
    assert_eq!(SDUI_UPDATE_PAYLOAD_BUDGET_BYTES, 1024);
    assert_eq!(KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS, 16);
    assert_eq!(EDIT_ACK_P95_BUDGET_MS, 40);
    assert_eq!(SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS, 16);
    assert_eq!(RUNTIME_CONFIGURATION_EVAL_P95_BUDGET_MS, 25);
    assert_eq!(LARGE_FILE_RESIDENT_MEMORY_BUDGET_MIB, 256);
}
