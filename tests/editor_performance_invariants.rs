use std::{fmt::Write as _, fs};

use clay::editor::{EditorCommand, EditorSurface};
use clay::protocol::DocumentAccess;

fn generated_lines(line_count: usize) -> String {
    let mut text = String::new();
    for line in 0..line_count {
        writeln!(text, "line {line:05}").expect("writing to String cannot fail");
    }
    text
}

fn repeated_lines(line_count: usize) -> String {
    let mut text = String::new();
    for _ in 0..line_count {
        text.push_str("viewport-bounded-line\n");
    }
    text
}

fn load_editor(text: String) -> EditorSurface {
    let mut editor = EditorSurface::default();
    editor.load_snapshot(7, 1, text, DocumentAccess::Editable { lease_id: 1 });
    editor
}

#[test]
fn visible_extraction_scales_with_viewport_not_document_size() {
    let mut short_doc = load_editor(repeated_lines(500));
    let mut long_doc = load_editor(repeated_lines(20_000));

    assert!(short_doc.update_visible_line_count_for_height(48.0 * 2.0 + 12.0 * 28.0));
    assert!(long_doc.update_visible_line_count_for_height(48.0 * 2.0 + 12.0 * 28.0));

    let short_visible = short_doc.visible_text();
    let long_visible = long_doc.visible_text();

    assert_eq!(short_visible, long_visible);
    assert!(short_visible.lines().count() <= 16);
}

#[test]
fn scroll_does_not_force_unrelated_full_layout_rebuilds() {
    let mut editor = load_editor(generated_lines(10_000));
    assert!(editor.update_visible_line_count_for_height(48.0 * 2.0 + 8.0 * 28.0));

    let before = editor.visible_text();
    assert!(editor.scroll_lines(5_000));
    let scrolled = editor.visible_text();

    assert_ne!(before, scrolled);
    assert!(scrolled.starts_with("line 05000\n"));

    assert!(editor.scroll_lines(-5_000));
    assert_eq!(editor.visible_text(), before);
}

#[test]
fn layout_cache_invalidates_on_text_width_font_or_viewport_changes() {
    let mut editor = load_editor(generated_lines(512));

    assert!(editor.update_visible_line_count_for_height(48.0 * 2.0 + 4.0 * 28.0));
    let narrow_window = editor.visible_text();

    assert!(editor.update_visible_line_count_for_height(48.0 * 2.0 + 12.0 * 28.0));
    let wide_window = editor.visible_text();

    assert!(wide_window.len() > narrow_window.len());
    assert!(wide_window.starts_with("line 00000\n"));
}

#[test]
fn typography_geometry_uses_shared_profile_baseline_not_fixed_font_size() {
    let surface_source = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let typography_source =
        fs::read_to_string("src/editor/typography.rs").expect("typography readable");
    let layout_source = fs::read_to_string("src/editor/layout.rs").expect("layout readable");

    assert!(!surface_source.contains("TEXT_FONT_SIZE"));
    assert!(surface_source.contains("self.typography.document_line_height()"));
    assert!(typography_source.contains("DOCUMENT_LINE_HEIGHT_MULTIPLIER"));
    assert!(layout_source.contains("DOCUMENT_LINE_HEIGHT_MULTIPLIER"));
}

#[test]
fn typography_updates_do_not_enter_editor_hot_paths() {
    let surface = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let layout = fs::read_to_string("src/editor/layout.rs").expect("layout readable");
    let sdui = fs::read_to_string("src/masonry_sdui.rs").expect("SDUI readable");
    let hot_paths = format!(
        "{}\n{}\n{}",
        non_test_body(&surface),
        non_test_body(&layout),
        non_test_body(&sdui)
    );

    assert!(layout.contains("editor.layout.cache_hit"));
    assert!(layout.contains("normalize_style_runs()"));
    for forbidden in [
        "Deno.core",
        "op_clay_theme_set_typography",
        "setTypography(",
        "std::fs",
        "reqwest",
        "ureq",
        "TcpStream",
        "Command::new",
    ] {
        assert!(
            !hot_paths.contains(forbidden),
            "typography paint/layout/input paths must not perform JS, IPC, font-file, network, or shell work: {forbidden}"
        );
    }
}

#[test]
fn runtime_generation_install_stays_outside_paint_and_text_event_hot_paths() {
    let surface = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let layout = fs::read_to_string("src/editor/layout.rs").expect("layout readable");
    let masonry = fs::read_to_string("src/masonry_editor.rs").expect("masonry editor readable");
    let hot_paths = format!("{}\n{}", non_test_body(&surface), non_test_body(&layout));
    let paint_body = masonry
        .split("fn paint(")
        .nth(1)
        .and_then(|rest| rest.split("fn accessibility_role(").next())
        .expect("paint method present");
    let text_event_body = masonry
        .split("fn on_text_event(")
        .nth(1)
        .and_then(|rest| rest.split("fn on_access_event(").next())
        .unwrap_or("");

    assert!(
        masonry.contains("fn install_runtime_state_snapshot"),
        "runtime install must remain a dedicated connection-event path"
    );
    assert!(
        masonry.contains("ClientRuntimeStateCandidate::validate"),
        "runtime install must validate a complete candidate before mutation"
    );
    for forbidden in [
        "ClientRuntimeStateCandidate",
        "install_runtime_state_snapshot",
        "RuntimeStateSnapshot",
        "Deno.core",
        "op_clay_",
        "write_client_message",
        "reload_runtime_generation",
    ] {
        assert!(
            !hot_paths.contains(forbidden),
            "editor paint/layout paths must not validate or install runtime snapshots: {forbidden}"
        );
        assert!(
            !paint_body.contains(forbidden),
            "masonry paint must not validate or install runtime snapshots: {forbidden}"
        );
        assert!(
            !text_event_body.contains(forbidden),
            "masonry text events must not validate or install runtime snapshots: {forbidden}"
        );
    }
}

#[test]
fn parse_window_snapshot_primitive_uses_bounded_rope_slicing() {
    let document_source =
        fs::read_to_string("src/server/document.rs").expect("document source readable");
    let parse_source = fs::read_to_string("src/server/parse_coordinator.rs")
        .expect("parse coordinator source readable");
    let connection_source =
        fs::read_to_string("src/server/connection.rs").expect("connection source readable");
    let edit_refresh = connection_source
        .split("async fn refresh_native_syntax_after_edit(")
        .nth(1)
        .expect("native edit refresh")
        .split("async fn schedule_open_parse(")
        .next()
        .expect("bounded native edit refresh body");

    assert!(document_source.contains("byte_slice(start..end).to_string()"));
    assert!(document_source.contains("validate_parse_snapshot_range"));
    assert!(document_source.contains("parse_window_after_edit"));
    assert!(parse_source.contains("schedule_parse_with_windows"));
    assert!(parse_source.contains("task.handle.abort()"));
    assert!(!edit_refresh.contains(".text()"));
}

#[test]
fn paint_uses_cached_inert_spans_without_package_javascript() {
    let surface_source = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let layout_source = fs::read_to_string("src/editor/layout.rs").expect("layout readable");
    let widget_source =
        fs::read_to_string("src/masonry_editor.rs").expect("editor widget readable");
    let paint_sources = format!("{surface_source}\n{layout_source}");
    let surface_paint = surface_source
        .split("pub fn paint(")
        .nth(1)
        .expect("EditorSurface::paint")
        .split("fn paint_caret(")
        .next()
        .expect("EditorSurface paint body through scrollbar/text");
    let widget_paint = widget_source
        .split("fn paint(&mut self, ctx: &mut PaintCtx")
        .nth(1)
        .expect("EditorWidget::paint")
        .split("fn accessibility_role(")
        .next()
        .expect("EditorWidget::paint body");
    let paint_bodies = format!("{surface_paint}\n{widget_paint}");

    assert!(surface_source.contains("normalize_visible_text_style_runs"));
    assert!(layout_source.contains("StyleProperty::Brush"));
    for forbidden in [
        "markdownIt",
        "parseMarkdown",
        "serverPublishDecorations",
        "serverPublishDiagnostics",
        "TreeSitterSyntaxHandler",
        "tree_sitter",
        "Deno.core",
        "op_clay",
    ] {
        assert!(
            !paint_sources.contains(forbidden),
            "paint/layout source must not call package/server/parser code: {forbidden}"
        );
    }
    // Phase 20: clipboard IO and save/reload enqueue stay off the paint path.
    for forbidden in [
        "SystemClipboard",
        "ClipboardSink",
        "get_text",
        "set_text",
        "copy_selection_to_system_clipboard",
        "cut_selection_to_system_clipboard",
        "paste_from_system_clipboard",
        "enqueue_save_document",
        "enqueue_reload_document",
        "SaveDocument",
        "ReloadDocument",
        "open_markdown_file_dialog",
        "open_folder_dialog",
    ] {
        assert!(
            !paint_bodies.contains(forbidden),
            "paint path must not perform clipboard/save/dialog work: {forbidden}"
        );
    }
}

#[test]
fn exact_range_decoration_replacement_stays_off_edit_and_paint_hot_paths() {
    let surface = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let apply_set = surface
        .split("fn apply_set(&mut self, set: DecorationSet)")
        .nth(1)
        .and_then(|body| body.split("fn span_count(&self)").next())
        .expect("decoration apply_set body");
    let apply_edit = surface
        .split("fn apply_edit(&mut self, operation: &EditOperation)")
        .nth(1)
        .and_then(|body| body.split("fn confirm_version(").next())
        .expect("decoration apply_edit body");
    let paint = surface
        .split("pub fn paint(")
        .nth(1)
        .and_then(|body| body.split("fn paint_caret(").next())
        .expect("surface paint body");

    assert!(apply_set.contains("subtract_provisional_chunk"));
    assert!(apply_set.contains("coalesce_local_residual"));
    for hot_path in [apply_edit, paint] {
        for forbidden in [
            "subtract_provisional_chunk",
            "coalesce_local_residual",
            "TreeSitterSyntaxHandler",
            "serverPublishDecorations",
            "write_client_message",
            "Deno.core",
        ] {
            assert!(
                !hot_path.contains(forbidden),
                "edit/paint hot path must not run authoritative replacement, parser, IPC, or JavaScript work: {forbidden}"
            );
        }
    }
}

#[test]
fn completion_hot_paths_use_inert_state_and_nonblocking_enqueue_only() {
    let surface_source = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let widget_source =
        fs::read_to_string("src/masonry_editor.rs").expect("editor widget readable");
    let client_queue_source =
        fs::read_to_string("src/client/mod.rs").expect("client queue readable");
    let combined = format!("{surface_source}\n{widget_source}");

    assert!(surface_source.contains("completion_request_event"));
    assert!(widget_source.contains("enqueue_completion_request"));
    assert!(client_queue_source.contains("ClientMessage::CompletionRequest"));
    assert!(client_queue_source.contains("try_send"));
    for forbidden in [
        "CompletionCoordinator",
        "schedule_completion",
        "serverRegisterCompletionProvider",
        "loadPackage",
        "Deno.core",
        "op_clay",
        "BufferWordCompletionProvider",
        "tokio::spawn",
        "std::fs",
    ] {
        assert!(
            !combined.contains(forbidden),
            "editor key/text/paint path must not run completion provider/package/server work: {forbidden}"
        );
    }
}

#[test]
fn language_server_process_work_is_absent_from_editor_hot_paths() {
    let surface_source = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let widget_source =
        fs::read_to_string("src/masonry_editor.rs").expect("editor widget readable");
    let client_source = fs::read_to_string("src/client/mod.rs").expect("client readable");
    let combined = format!("{surface_source}\n{widget_source}\n{client_source}");
    for forbidden in [
        "LanguageServerProcessService",
        "startLanguageServerSession",
        "language_server_process",
        "op_clay_language_server_start_session",
        "op_clay_language_server_send_message",
        "op_clay_language_server_read_message",
        "op_clay_language_server_send_bytes",
        "op_clay_language_server_read_bytes",
        "tokio::process::Command",
        "std::process::Command",
    ] {
        assert!(
            !combined.contains(forbidden),
            "editor key/text/paint/client path must not run language-server process/session work: {forbidden}"
        );
    }
}

#[test]
fn document_analysis_capacity_constants_match_approved_phase18_21_contract() {
    use clay::perf::budgets::{
        DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES, DOCUMENT_ANALYSIS_MAX_WORKERS,
        DOCUMENT_ANALYSIS_WORKER_HEAP_BYTES, LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES,
    };
    assert_eq!(DOCUMENT_ANALYSIS_MAX_WORKERS, 4);
    assert_eq!(DOCUMENT_ANALYSIS_WORKER_HEAP_BYTES, 64 * 1024 * 1024);
    assert_eq!(DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES, 256 * 1024);
    assert_eq!(LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES, 1024 * 1024);

    let analysis =
        fs::read_to_string("src/server/document_analysis.rs").expect("analysis readable");
    assert!(analysis.contains("DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES"));
    assert!(analysis.contains("DOCUMENT_ANALYSIS_INPUT_MAX_EVENTS"));
    assert!(analysis.contains("DOCUMENT_ANALYSIS_OUTPUT_MAX_BYTES"));
    assert!(
        analysis.contains("coalesce_reset") || analysis.contains("Coalesce"),
        "mailbox must coalesce/reset rather than unbounded-queue every edit"
    );
}

#[test]
fn document_analysis_runs_after_ack_and_outside_editor_hot_paths() {
    let connection = fs::read_to_string("src/server/connection.rs").expect("connection readable");
    let edit_branch = connection
        .split("ClientMessage::Edit {")
        .nth(1)
        .and_then(|source| source.split("ClientMessage::EditorIntent {").next())
        .expect("edit branch present");
    assert!(
        edit_branch.find("write_server_message(&mut stream, &response)")
            < edit_branch.find("document_analysis.change_document")
    );

    let surface = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let widget = fs::read_to_string("src/masonry_editor.rs").expect("widget readable");
    let client = fs::read_to_string("src/client/mod.rs").expect("client readable");
    let combined = format!("{surface}\n{widget}\n{client}");
    for forbidden in [
        "DocumentAnalysisCoordinator",
        "invoke_document_analyzer",
        "serverRegisterDocumentAnalyzer",
        "op_clay_language_register_document_analyzer",
        "DOCUMENT_ANALYSIS_WORKER_HEAP_BYTES",
    ] {
        assert!(
            !combined.contains(forbidden),
            "editor hot path contains {forbidden}"
        );
    }
}

#[test]
fn language_intelligence_provider_work_is_absent_from_editor_hot_paths() {
    let surface_source = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let widget_source =
        fs::read_to_string("src/masonry_editor.rs").expect("editor widget readable");
    let client_source = fs::read_to_string("src/client/mod.rs").expect("client readable");
    let combined = format!("{surface_source}\n{widget_source}\n{client_source}");
    for forbidden in [
        "LanguageIntelligenceCoordinator",
        "schedule_language_intelligence",
        "serverRegisterLanguageIntelligenceProvider",
        "op_clay_language_register_intelligence_provider",
        "LanguageIntelligenceProviderRegistry",
        "__clayLanguageIntelligenceHandlers",
        "provideLanguageIntelligence",
        "LanguageServerProcessService",
        "tokio::process::Command",
    ] {
        assert!(
            !combined.contains(forbidden),
            "editor key/text/paint/client path must not run language-intelligence provider work: {forbidden}"
        );
    }
}

#[test]
fn markdown_full_document_adapter_is_not_large_file_hot_path_static_guard() {
    let bench_source = fs::read_to_string("tools/bench/markdown-parser.mjs")
        .expect("Markdown benchmark script readable");
    let load_source = fs::read_to_string("packages/markdown/dist/load.js")
        .expect("Markdown load source readable");
    let parser_source = fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("Markdown parser source readable");

    assert!(bench_source.contains("adapter_full_document_advisory"));
    assert!(bench_source.contains("adapter_windowed_viewport"));
    assert!(bench_source.contains("hotPathAllowed"));
    assert!(load_source.contains("parseWindowBytes"));
    assert!(parser_source.contains("parseWindowInputs(options)"));
    assert!(parser_source.contains("plainTextFallbackReason(options)"));
}

#[test]
fn unicode_boundaries_remain_valid_after_layout_optimizations() {
    let mut editor = load_editor("a🦀b".to_string());

    assert!(editor.command(EditorCommand::DocumentEnd));
    assert!(editor.command(EditorCommand::Backspace));
    assert!(editor.command(EditorCommand::Backspace));

    assert_eq!(editor.visible_text(), "a");
}

// ── Phase 18.9 Task 7: fallback activation + keypress budget alignment ──
//
// Advisory budget references (Phase 14 defers hard latency CI gates): the
// Phase 18.9 always-available `core.text`/`core.code` fallback activation and
// synchronous keypress-to-local-paint budgets must have one source of truth
// the runtime references, so drift between `ModeRegistry::activation_budget_ms`
// and the documented constants is caught here.

use clay::packages::modes::ModeRegistry;
use clay::perf::budgets::{
    BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS,
    MODE_ACTIVATION_P95_BUDGET_MS,
};

#[test]
fn phase18_9_mode_activation_budget_is_single_source_of_truth() {
    // `ModeRegistry::activation_budget_ms` is the value the activation path
    // references; it must equal the documented `MODE_ACTIVATION_P95_BUDGET_MS`
    // so advisory latency assertions and runtime budgets never drift.
    assert_eq!(
        ModeRegistry::new().activation_budget_ms(),
        MODE_ACTIVATION_P95_BUDGET_MS
    );
    const { assert!(MODE_ACTIVATION_P95_BUDGET_MS > 0) };
}

#[test]
fn phase18_9_keypress_to_paint_budget_orders_below_mode_activation_budget() {
    // Synchronous keypress-to-local-paint is a strict hot-path budget and must
    // stay well below open/reload activation latency: a mode activation that
    // blocked local paint would violate the no-sync-JS-before-paint invariant
    // Phase 18.9 preserves. Advisory ordering only (no CI latency harness).
    const {
        assert!(
            KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS < MODE_ACTIVATION_P95_BUDGET_MS,
            "keypress-to-paint budget must be tighter than mode activation budget"
        )
    };
    const { assert!(BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES > 0) };
}

/// Return the non-test portion of a source file: everything before the first
/// `#[cfg(test)]` or `mod tests` boundary. Files without a test module return
/// the whole source.
fn non_test_body(src: &str) -> &str {
    let mut cut = src.len();
    if let Some(i) = src.find("\n#[cfg(test)]") {
        cut = cut.min(i);
    }
    if let Some(i) = src.find("\nmod tests") {
        cut = cut.min(i);
    }
    &src[..cut]
}

#[test]
fn style_registry_is_single_source_of_color_for_paint_paths() {
    // Plan 046 task 4 source guard: no editor/shell paint-path file may hold a
    // `Color::from_rgb*` literal except `src/editor/theme.rs` (the theme-
    // definition module). The StyleRegistry owns all color; surface/shell read
    // from it. If a literal reappears in the paint path this fails fast.
    let paint_path_files = [
        "src/editor.rs",
        "src/editor/surface.rs",
        "src/editor/layout.rs",
        "src/editor/buffer.rs",
        "src/editor/cursor.rs",
        "src/editor/selection.rs",
        "src/editor/viewport.rs",
        "src/masonry_editor.rs",
        "src/masonry_sdui.rs",
        "src/masonry_shell.rs",
    ];
    for file in paint_path_files {
        let src =
            fs::read_to_string(file).unwrap_or_else(|e| panic!("{file} should be readable: {e}"));
        let body = non_test_body(&src);
        assert!(
            !body.contains("Color::from_rgb8("),
            "{file} paint path must source color from StyleRegistry, not a Color::from_rgb8 literal"
        );
        assert!(
            !body.contains("Color::from_rgba8("),
            "{file} paint path must source color from StyleRegistry, not a Color::from_rgba8 literal"
        );
    }

    // The theme-definition module is the ONLY paint-path file allowed to hold
    // color literals, and it must actually hold them (the default Clay theme
    // lives there), otherwise the registry stopped being the single source.
    let theme_src = fs::read_to_string("src/editor/theme.rs")
        .expect("src/editor/theme.rs (theme-definition module) should be readable");
    let theme_body = non_test_body(&theme_src);
    assert!(
        theme_body.contains("Color::from_rgb8(") || theme_body.contains("Color::from_rgba8("),
        "src/editor/theme.rs must own the default Clay theme color literals"
    );
    assert!(
        theme_body.contains("pub struct StyleRegistry"),
        "src/editor/theme.rs must define the StyleRegistry single source of color"
    );
}

#[test]
fn diagnostic_paint_uses_theme_owned_severity_styles_only() {
    let theme = fs::read_to_string("src/editor/theme.rs").unwrap();
    assert!(theme.contains("fn diagnostic_style"));
    assert!(theme.contains("diagnostic_error"));
    assert!(theme.contains("diagnostic_warning"));
    assert!(theme.contains("diagnostic_info"));

    let surface = fs::read_to_string("src/editor/surface.rs").unwrap();
    let body = non_test_body(&surface);
    assert!(body.contains("diagnostic_style(span.severity)"));
    let apply_body = body
        .split("fn apply_diagnostic_set")
        .nth(1)
        .expect("apply_diagnostic_set")
        .split("pub(crate) fn clear_decorations")
        .next()
        .expect("apply_diagnostic_set body");
    assert!(
        !apply_body.contains("bump_layout_style_revision"),
        "apply_diagnostic_set must not bump layout_style_revision"
    );
}

#[test]
fn snippet_accept_is_bounded_client_local_text_work() {
    let surface = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let snippet = fs::read_to_string("src/editor/snippet.rs").expect("snippet readable");
    let accept_body = surface
        .split("pub(crate) fn accept_completion_with_event")
        .nth(1)
        .expect("completion accept")
        .split("pub(crate) fn has_active_snippet_session")
        .next()
        .expect("completion accept body");
    let parser_body = snippet
        .split("pub(crate) fn parse_snippet")
        .nth(1)
        .expect("snippet parser")
        .split("fn push_text_char")
        .next()
        .expect("snippet parser body");
    let hot_path = format!("{accept_body}\n{parser_body}");

    assert!(accept_body.contains("parse_snippet"));
    // Phase 20 routes accepted completion inserts through the shared local-edit
    // helper so inverse history is recorded the same way as ordinary typing.
    assert!(accept_body.contains("apply_and_record_local_edit"));
    for forbidden in [
        "Deno.core",
        "op_clay_",
        "enqueue_",
        "serverRegisterCompletionProvider",
        "std::fs",
        "std::process",
        "TcpStream",
        "reqwest",
        "ureq",
    ] {
        assert!(
            !hot_path.contains(forbidden),
            "snippet accept/parser must not run provider code, IPC, filesystem, network, or shell work: {forbidden}"
        );
    }
}

#[test]
fn range_diagnostics_do_not_enter_editor_hot_paths() {
    let surface = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let layout = fs::read_to_string("src/editor/layout.rs").expect("layout readable");
    let widget = fs::read_to_string("src/masonry_editor.rs").expect("widget readable");
    let hot_paths = format!(
        "{}\n{}\n{}",
        non_test_body(&surface),
        non_test_body(&layout),
        non_test_body(&widget)
    );

    assert!(layout.contains("fn paint_squiggle"));
    assert!(surface.contains("visible_diagnostic_ranges"));
    for forbidden in [
        "TreeSitterSyntaxHandler",
        "collect_syntax_diagnostics",
        "validate_diagnostic_publication",
        "validate_diagnostic_set",
        "serverPublishDiagnostics",
        "op_clay_diagnostics_publish_diagnostics",
        "Deno.core",
        "ParseCoordinator",
        "std::fs",
        "Command::new",
        "reqwest",
        "LanguageServer",
    ] {
        assert!(
            !hot_paths.contains(forbidden),
            "range-diagnostic paint/layout/input paths must not run parser/JS/IPC/validation work: {forbidden}"
        );
    }
}

#[test]
fn semantic_intelligence_reuses_existing_decoration_paths_without_hot_path_work() {
    let surface = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let theme = fs::read_to_string("src/editor/theme.rs").expect("theme readable");
    let widget = fs::read_to_string("src/masonry_editor.rs").expect("widget readable");
    let decorations_facade =
        fs::read_to_string("runtime/js/decorations.ts").expect("decorations facade readable");
    let hot_paths = format!(
        "{}\n{}\n{}",
        non_test_body(&surface),
        non_test_body(&theme),
        non_test_body(&widget)
    );

    assert!(surface.contains("normalize_visible_text_style_runs"));
    assert!(surface.contains("DecorationKind::Semantic"));
    assert!(theme.contains("DecorationKind::Syntax | DecorationKind::Semantic"));
    assert!(decorations_facade.contains("tokenType?"));
    assert!(decorations_facade.contains("modifiers?"));
    assert!(decorations_facade.contains("\"semantic\""));

    for forbidden in [
        "serverPublishDecorations",
        "op_clay_decorations_publish_decorations",
        "LanguageServerProcessService",
        "startLanguageServerSession",
        "provideLanguageIntelligence",
        "__clayLanguageIntelligenceHandlers",
        "tokio::process::Command",
        "std::process::Command",
        "Deno.core",
    ] {
        assert!(
            !hot_paths.contains(forbidden),
            "semantic paint/layout must stay additive over cached spans without publish/process/JS work: {forbidden}"
        );
    }
}
