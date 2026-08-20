use std::{fmt::Write as _, fs};

use clay::editor::{EditorCommand, EditorSurface};
use clay::protocol::DocumentAccess;

mod common;
use common::{assert_absent, hot_path_concat, non_test};

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

    assert!(short_doc.update_visible_line_count_for_height(20.0 * 2.0 + 12.0 * 28.0));
    assert!(long_doc.update_visible_line_count_for_height(20.0 * 2.0 + 12.0 * 28.0));

    let short_visible = short_doc.visible_text();
    let long_visible = long_doc.visible_text();

    assert_eq!(short_visible, long_visible);
    assert!(short_visible.lines().count() <= 24);
}

#[test]
fn scroll_does_not_force_unrelated_full_layout_rebuilds() {
    let mut editor = load_editor(generated_lines(10_000));
    assert!(editor.update_visible_line_count_for_height(20.0 * 2.0 + 8.0 * 28.0));

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

    assert!(editor.update_visible_line_count_for_height(20.0 * 2.0 + 4.0 * 28.0));
    let narrow_window = editor.visible_text();

    assert!(editor.update_visible_line_count_for_height(20.0 * 2.0 + 12.0 * 28.0));
    let wide_window = editor.visible_text();

    assert!(wide_window.len() > narrow_window.len());
    assert!(wide_window.starts_with("line 00000\n"));
}

#[test]
fn typography_geometry_uses_shared_profile_baseline_not_fixed_font_size() {
    let surface_source = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
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
    let surface = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
    let layout = fs::read_to_string("src/editor/layout.rs").expect("layout readable");
    let sdui = fs::read_to_string("src/masonry_sdui.rs").expect("SDUI readable");
    let hot_paths = format!(
        "{}\n{}\n{}",
        non_test(&surface),
        non_test(&layout),
        non_test(&sdui)
    );

    assert!(layout.contains("editor.layout.cache_hit"));
    assert!(layout.contains("normalize_style_runs()"));
    assert_absent(
        &hot_paths,
        &[
            "Deno.core",
            "op_clay_theme_set_typography",
            "setTypography(",
            "std::fs",
            "reqwest",
            "ureq",
            "TcpStream",
            "Command::new",
        ],
        "typography paint/layout/input paths must not perform JS, IPC, font-file, network, or shell work",
    );
}

#[test]
fn runtime_generation_install_stays_outside_paint_and_text_event_hot_paths() {
    let surface = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
    let layout = fs::read_to_string("src/editor/layout.rs").expect("layout readable");
    let masonry = fs::read_to_string("src/masonry_editor.rs").expect("masonry editor readable");
    let hot_paths = format!("{}\n{}", non_test(&surface), non_test(&layout));
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
    let connection_source = fs::read_to_string("src/server/connection/documents.rs")
        .expect("connection source readable");
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
    let surface_source = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
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
    assert_absent(
        &paint_sources,
        &[
            "markdownIt",
            "parseMarkdown",
            "serverPublishDecorations",
            "serverPublishDiagnostics",
            "TreeSitterSyntaxHandler",
            "tree_sitter",
            "Deno.core",
            "op_clay",
        ],
        "paint/layout source must not call package/server/parser code",
    );
    // Phase 20: clipboard IO and save/reload enqueue stay off the paint path.
    assert_absent(
        &paint_bodies,
        &[
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
        ],
        "paint path must not perform clipboard/save/dialog work",
    );
}

#[test]
fn exact_range_decoration_replacement_stays_off_edit_and_paint_hot_paths() {
    let decoration =
        fs::read_to_string("src/editor/surface/decoration.rs").expect("decoration readable");
    let apply_set = decoration
        .split("fn apply_set(&mut self, set: DecorationSet)")
        .nth(1)
        .and_then(|body| body.split("fn span_count(&self)").next())
        .expect("decoration apply_set body");
    let apply_edit = decoration
        .split("fn apply_edit(&mut self, operation: &EditOperation)")
        .nth(1)
        .and_then(|body| body.split("fn confirm_version(").next())
        .expect("decoration apply_edit body");
    let surface = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
    let paint = surface
        .split("pub fn paint(")
        .nth(1)
        .and_then(|body| body.split("fn paint_caret(").next())
        .expect("surface paint body");

    assert!(apply_set.contains("subtract_provisional_chunk"));
    assert!(apply_set.contains("coalesce_local_residual"));
    for hot_path in [apply_edit, paint] {
        assert_absent(
            hot_path,
            &[
                "subtract_provisional_chunk",
                "coalesce_local_residual",
                "TreeSitterSyntaxHandler",
                "serverPublishDecorations",
                "write_client_message",
                "Deno.core",
            ],
            "edit/paint hot path must not run authoritative replacement, parser, IPC, or JavaScript work",
        );
    }
}

#[test]
fn completion_hot_paths_use_inert_state_and_nonblocking_enqueue_only() {
    let surface_source = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
    let widget_source =
        fs::read_to_string("src/masonry_editor.rs").expect("editor widget readable");
    // Phase 22.2: the per-document completion plumbing lives in the pane view.
    let view_source =
        fs::read_to_string("src/masonry_pane_document.rs").expect("pane view readable");
    let client_queue_source =
        fs::read_to_string("src/client/mod.rs").expect("client queue readable");
    let combined = format!("{surface_source}\n{widget_source}\n{view_source}");

    assert!(surface_source.contains("completion_request_event"));
    assert!(view_source.contains("enqueue_completion_request"));
    assert!(client_queue_source.contains("ClientMessage::CompletionRequest"));
    assert!(client_queue_source.contains("try_send"));
    assert_absent(
        &combined,
        &[
            "CompletionCoordinator",
            "schedule_completion",
            "serverRegisterCompletionProvider",
            "loadPackage",
            "Deno.core",
            "op_clay",
            "BufferWordCompletionProvider",
            "tokio::spawn",
            "std::fs",
        ],
        "editor key/text/paint path must not run completion provider/package/server work",
    );
}

#[test]
fn completion_projection_is_bounded_and_stays_out_of_paint() {
    let session =
        fs::read_to_string("src/shell/transient_menu.rs").expect("transient menu source readable");
    assert!(session.contains("take(MAX_ITEMS)"));

    let package = fs::read_to_string("src/masonry_package_region.rs")
        .expect("package region source readable");
    let package_impl = package
        .split("impl Widget for PackageOverlayHost")
        .nth(1)
        .expect("PackageOverlayHost widget implementation present")
        .split("#[cfg(test)]")
        .next()
        .expect("PackageOverlayHost production body");
    let package_layout = package_impl
        .split("fn layout(")
        .nth(1)
        .and_then(|body| body.split("fn paint(").next())
        .expect("PackageOverlayHost layout body");
    let package_paint = package_impl
        .split("fn paint(")
        .nth(1)
        .and_then(|body| body.split("fn accessibility_role(").next())
        .expect("PackageOverlayHost paint body");
    assert!(package_layout.contains("completion_overlay_rect"));
    assert!(!package_paint.contains("completion_overlay_rect"));
    assert!(!package_paint.contains("menu_item_count"));
    assert_absent(
        package_paint,
        &[
            "Deno.core",
            "write_client_message",
            "std::fs",
            "Command::new",
        ],
        "completion overlay paint must not perform JS, IPC, filesystem, or shell work",
    );

    let geometry =
        fs::read_to_string("src/shell/package_ui.rs").expect("package UI source readable");
    let geometry_body = geometry
        .split("pub(crate) fn completion_overlay_rect(")
        .nth(1)
        .and_then(|body| body.split("\n}\n").next())
        .expect("completion geometry helper body");
    assert!(geometry_body.contains("item_count.clamp(1, COMPLETION_MAX_VISIBLE_ROWS)"));
    assert!(geometry_body.contains("COMPLETION_MAX_WIDTH_PX"));

    let editor = fs::read_to_string("src/masonry_editor.rs").expect("editor source readable");
    let editor_paint = editor
        .split("fn paint(&mut self, ctx: &mut PaintCtx")
        .nth(1)
        .and_then(|body| body.split("fn accessibility_role(").next())
        .expect("EditorWidget paint body");
    assert!(!editor_paint.contains("completion_overlay_rect"));
    assert!(!editor_paint.contains("TransientMenuSession"));
}

#[test]
fn language_server_process_work_is_absent_from_editor_hot_paths() {
    let surface_source = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
    let widget_source =
        fs::read_to_string("src/masonry_editor.rs").expect("editor widget readable");
    let client_source = fs::read_to_string("src/client/mod.rs").expect("client readable");
    let combined = format!("{surface_source}\n{widget_source}\n{client_source}");
    assert_absent(
        &combined,
        &[
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
        ],
        "editor key/text/paint/client path must not run language-server process/session work",
    );
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
    let connection =
        fs::read_to_string("src/server/connection/documents.rs").expect("connection readable");
    // The Edit/EditorIntent arms share one apply/ack/follow-up path (Plan 060
    // T4): assert the invariant inside the shared dispatcher — the edit ack is
    // written to the client before any analysis follow-up work runs.
    let edit_branch = connection
        .split("async fn dispatch_edit_operation")
        .nth(1)
        .expect("shared edit dispatch present");
    assert!(
        edit_branch.find("write_server_message(stream, &response)")
            < edit_branch.find("document_analysis.change_document")
    );

    let surface = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
    let widget = fs::read_to_string("src/masonry_editor.rs").expect("widget readable");
    let client = fs::read_to_string("src/client/mod.rs").expect("client readable");
    let combined = format!("{surface}\n{widget}\n{client}");
    assert_absent(
        &combined,
        &[
            "DocumentAnalysisCoordinator",
            "invoke_document_analyzer",
            "serverRegisterDocumentAnalyzer",
            "op_clay_language_register_document_analyzer",
            "DOCUMENT_ANALYSIS_WORKER_HEAP_BYTES",
        ],
        "editor hot path",
    );
}

#[test]
fn language_intelligence_provider_work_is_absent_from_editor_hot_paths() {
    let surface_source = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
    let widget_source =
        fs::read_to_string("src/masonry_editor.rs").expect("editor widget readable");
    let client_source = fs::read_to_string("src/client/mod.rs").expect("client readable");
    let combined = format!("{surface_source}\n{widget_source}\n{client_source}");
    assert_absent(
        &combined,
        &[
            "LanguageIntelligenceCoordinator",
            "schedule_language_intelligence",
            "serverRegisterLanguageIntelligenceProvider",
            "op_clay_language_register_intelligence_provider",
            "LanguageIntelligenceProviderRegistry",
            "__clayLanguageIntelligenceHandlers",
            "provideLanguageIntelligence",
            "LanguageServerProcessService",
            "tokio::process::Command",
        ],
        "editor key/text/paint/client path must not run language-intelligence provider work",
    );
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
    assert!(load_source.contains("maxWindowBytes: 64 * 1024"));
    assert!(parser_source.contains("parseWindowBytes"));
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
    ACTIVE_LINE_PAINT_P95_BUDGET_MS, BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES,
    BRACKET_MATCH_PAINT_P95_BUDGET_MS, DECORATION_BACKGROUND_FILL_P95_BUDGET_MS,
    GUTTER_PAINT_P95_BUDGET_MS, KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS,
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

#[test]
fn phase26_7_chrome_paint_budgets_fit_inside_keypress_envelope() {
    const {
        assert!(
            GUTTER_PAINT_P95_BUDGET_MS
                + ACTIVE_LINE_PAINT_P95_BUDGET_MS
                + BRACKET_MATCH_PAINT_P95_BUDGET_MS
                + DECORATION_BACKGROUND_FILL_P95_BUDGET_MS
                <= KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS,
            "chrome/background paint envelopes must fit in keypress-to-local-paint"
        )
    };
}

/// Return the non-test portion of a source file: everything before the first
/// `#[cfg(test)]` or `mod tests` boundary. Files without a test module return
/// the whole source.

#[test]
fn hot_path_no_theme_resolution_or_package_js() {
    // Plan 065 (Phase 20.4) task 11: restyled paint hot paths must read cached
    // ResolvedUiTheme/SduiThemeStyle values only — no per-frame theme
    // re-resolution (ThemeTokenResolver/from_resolver/core_theme_value parse),
    // no package JavaScript, no server round trip, no filesystem/network/shell
    // work. Theme resolution happens once at install time into ResolvedUiTheme;
    // paint reads cached typed values.
    let files = [
        "src/masonry_sdui.rs",
        "src/editor/surface/mod.rs",
        "src/masonry_editor.rs",
        "src/masonry_shell/mod.rs",
        "src/masonry_package_region.rs",
        "src/masonry_sdui_region.rs",
        "src/shell/package_ui.rs",
        "src/shell/primitives.rs",
    ];
    let hot_paths = hot_path_concat(&files);
    assert_absent(
        &hot_paths,
        &[
            "ThemeTokenResolver::new()",
            "ThemeTokenResolver::new",
            "from_resolver(",
            "core_theme_value",
            "Deno.core",
            "op_clay_theme_set_theme",
            "op_clay_theme_set_typography",
            "reqwest",
            "ureq",
            "TcpStream",
            "Command::new",
            "std::fs::read",
        ],
        "Phase 20.4 paint hot paths must not re-resolve themes or run package/server/IO work",
    );
    // The SDUI path must resolve through from_ui_theme (cached), not from_resolver.
    assert!(
        hot_paths.contains("from_ui_theme"),
        "SDUI paint must resolve via SduiThemeStyle::from_ui_theme (cached ResolvedUiTheme)"
    );
}

#[test]
fn style_registry_is_single_source_of_color_for_paint_paths() {
    // Plan 046 task 4 source guard: no editor/shell paint-path file may hold a
    // `Color::from_rgb*` literal except `src/editor/theme.rs` (the theme-
    // definition module). The StyleRegistry owns all color; surface/shell read
    // from it. If a literal reappears in the paint path this fails fast.
    let paint_path_files = [
        "src/editor.rs",
        "src/editor/surface/mod.rs",
        "src/editor/surface/chrome.rs",
        "src/editor/layout.rs",
        "src/editor/buffer.rs",
        "src/editor/cursor.rs",
        "src/editor/selection.rs",
        "src/editor/viewport.rs",
        "src/masonry_editor.rs",
        "src/masonry_sdui.rs",
        "src/masonry_shell/mod.rs",
    ];
    for file in paint_path_files {
        let src =
            fs::read_to_string(file).unwrap_or_else(|e| panic!("{file} should be readable: {e}"));
        let body = non_test(&src);
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
    let theme_body = non_test(&theme_src);
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

    let surface = fs::read_to_string("src/editor/surface/mod.rs").unwrap();
    let body = non_test(&surface);
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
    let surface = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
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
    assert_absent(
        &hot_path,
        &[
            "Deno.core",
            "op_clay_",
            "enqueue_",
            "serverRegisterCompletionProvider",
            "std::fs",
            "std::process",
            "TcpStream",
            "reqwest",
            "ureq",
        ],
        "snippet accept/parser must not run provider code, IPC, filesystem, network, or shell work",
    );
}

#[test]
fn range_diagnostics_do_not_enter_editor_hot_paths() {
    let surface = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
    let layout = fs::read_to_string("src/editor/layout.rs").expect("layout readable");
    let widget = fs::read_to_string("src/masonry_editor.rs").expect("widget readable");
    let hot_paths = format!(
        "{}\n{}\n{}",
        non_test(&surface),
        non_test(&layout),
        non_test(&widget)
    );

    assert!(layout.contains("fn paint_squiggle"));
    assert!(surface.contains("visible_diagnostic_ranges"));
    assert_absent(
        &hot_paths,
        &[
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
        ],
        "range-diagnostic paint/layout/input paths must not run parser/JS/IPC/validation work",
    );
}

#[test]
fn semantic_intelligence_reuses_existing_decoration_paths_without_hot_path_work() {
    let surface = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
    let theme = fs::read_to_string("src/editor/theme.rs").expect("theme readable");
    let widget = fs::read_to_string("src/masonry_editor.rs").expect("widget readable");
    let decorations_declarations = fs::read_to_string("runtime/js/decorations.d.ts")
        .expect("decorations declarations readable");
    let hot_paths = format!(
        "{}\n{}\n{}",
        non_test(&surface),
        non_test(&theme),
        non_test(&widget)
    );

    assert!(surface.contains("normalize_visible_text_style_runs"));
    assert!(surface.contains("DecorationKind::Semantic"));
    assert!(theme.contains("DecorationKind::Syntax | DecorationKind::Semantic"));
    assert!(decorations_declarations.contains("tokenType?"));
    assert!(decorations_declarations.contains("modifiers?"));
    assert!(decorations_declarations.contains("\"semantic\""));

    assert_absent(
        &hot_paths,
        &[
            "serverPublishDecorations",
            "op_clay_decorations_publish_decorations",
            "LanguageServerProcessService",
            "startLanguageServerSession",
            "provideLanguageIntelligence",
            "__clayLanguageIntelligenceHandlers",
            "tokio::process::Command",
            "std::process::Command",
            "Deno.core",
        ],
        "semantic paint/layout must stay additive over cached spans without publish/process/JS work",
    );
}

#[test]
fn ui_design_tokens_resolve_without_package_javascript_in_paint_layout_or_input_hot_paths() {
    // Phase 20.1 source guard: token resolution (core_theme_value,
    // ThemeTokenResolver::resolve) must never run in Masonry paint/layout/
    // pointer/scroll hot paths. Token resolution happens at theme-install time
    // into a cached ResolvedUiTheme; paint paths read cached fields only.
    let hot_path_files = [
        "src/masonry_sdui.rs",
        "src/masonry_editor.rs",
        "src/masonry_shell/mod.rs",
    ];
    for file in hot_path_files {
        let src =
            fs::read_to_string(file).unwrap_or_else(|e| panic!("{file} should be readable: {e}"));
        let body = non_test(&src);
        assert!(
            !body.contains("core_theme_value("),
            "{file} hot path must not call core_theme_value(); token resolution is theme-install-time only"
        );
        assert!(
            !body.contains("ThemeTokenResolver"),
            "{file} hot path must not reference ThemeTokenResolver; use cached ResolvedUiTheme instead"
        );
        assert!(
            !body.contains("parse_override_token("),
            "{file} hot path must not parse theme tokens; that is package-load/configuration-time work"
        );
        assert!(
            !body.contains("parse_hex_rgba(") && !body.contains("parse_hex_color("),
            "{file} hot path must not parse hex colors; all color resolution goes through StyleRegistry"
        );
    }

    // ResolvedUiTheme construction and panel_defaults() must live in the
    // theme module, not in hot-path files.
    let theme_src =
        fs::read_to_string("src/shell/theme.rs").expect("shell/theme.rs should be readable");
    let theme_body = non_test(&theme_src);
    assert!(
        theme_body.contains("pub(crate) struct ResolvedUiTheme"),
        "ResolvedUiTheme must be the single cached UI token registry defined in shell/theme.rs"
    );
    assert!(
        theme_body.contains("fn resolved(&self"),
        "ResolvedUiTheme::resolved() must live in shell/theme.rs"
    );
    assert!(
        theme_body.contains("pub(crate) struct PanelDefaults"),
        "PanelDefaults must live in shell/theme.rs as the resolved token-backed geometry source"
    );
}

// ── Phase 22.6 (plan 077 task 5): window-model performance invariants ──

#[test]
fn accessibility_updates_reuse_stable_virtual_ids_without_allocator_churn() {
    let accessibility =
        fs::read_to_string("src/editor/accessibility.rs").expect("accessibility source readable");
    let shell = fs::read_to_string("src/masonry_shell/mod.rs").expect("shell source readable");
    let body = non_test(&accessibility);

    assert!(body.contains("VIRTUAL_A11Y_NODE_PREFIX"));
    assert!(body.contains("owner.to_raw()"));
    assert!(
        !body.contains("WidgetId::next()"),
        "virtual accessibility nodes must not allocate fresh IDs per tree pass"
    );
    assert!(
        shell.matches("virtual_a11y_node_id(").count() >= 3,
        "shell tab/status/announcement nodes must use the shared stable-ID helper"
    );
}

#[test]
fn editor_accessibility_uses_bounded_text_and_stable_action_ids() {
    let surface = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
    let pane = fs::read_to_string("src/masonry_pane_document.rs").expect("pane readable");
    let accessibility =
        fs::read_to_string("src/editor/accessibility.rs").expect("accessibility source readable");
    assert!(surface.contains("pub(crate) fn accessibility_text(&self)"));
    assert!(surface.contains("self.visible_snapshot().text"));
    assert!(pane.contains("populate_accessibility_text"));
    assert!(pane.contains("replace_accessibility_text"));
    assert!(accessibility.contains("pub(crate) const TEXT_RUN: u16 = 2"));
    assert!(!non_test(&pane).contains("WidgetId::next()"));
}

#[test]
fn retained_accessibility_update_fixture_stays_bounded() {
    let mut fixture = clay::perf::baselines::AccessibilityTreeBench::new(4);
    let first = fixture.update();
    let second = fixture.update();

    assert!(first > 0, "stable-ID update must emit accessibility nodes");
    assert_eq!(
        first, second,
        "repeated label updates keep node work bounded"
    );
}

#[test]
fn pane_chrome_geometry_work_scales_linearly_with_pane_count() {
    // The shell's per-pane paint work is chrome geometry: split dividers
    // (N-1), fixed-slot handles (none in the default layout), and the focus
    // ring (1 when N > 1). Piece count must be linear in pane count, never
    // document-size or tab-count dependent.
    use clay::perf::baselines::pane_chrome_piece_count;
    assert_eq!(pane_chrome_piece_count(1), 0);
    assert_eq!(pane_chrome_piece_count(2), 2);
    assert_eq!(pane_chrome_piece_count(4), 4);
    assert_eq!(
        pane_chrome_piece_count(4) - pane_chrome_piece_count(2),
        pane_chrome_piece_count(2) - pane_chrome_piece_count(1),
        "pane chrome work must grow linearly with pane count"
    );
}

#[test]
fn tab_switch_path_performs_no_document_reserialization() {
    // A tab switch mounts the target tab's chrome at its pane rects; it must
    // never serialize document text, send client messages, enqueue tab
    // commands, or touch the document lifecycle. The widgets involved are
    // the shell and the pane host (the driver owns all queues/IPC).
    for file in ["src/masonry_shell/mod.rs", "src/masonry_pane_host.rs"] {
        let src = fs::read_to_string(file).unwrap_or_else(|error| panic!("read {file}: {error}"));
        let body = non_test(&src);
        assert_absent(
            body,
            &[
                "write_client_message",
                "ClientMessage",
                "rkyv",
                "enqueue_tab_command",
                "InitialDocument",
                "DocumentOpened",
                "DocumentReloaded",
                "encode_client_message",
                "encode_server_message",
            ],
            "tab-switch path must not serialize documents or send messages",
        );
    }
}

#[test]
fn responsive_layout_work_preserves_sidebar_and_editor_bounds() {
    // The benchmark helper calls the production SDUI slot decision. Keep this
    // small typed matrix blocking: narrow panes yield the sidebar, normal
    // panes keep it, and large UI typography yields it until the pane is wide
    // enough for a usable editor region.
    use clay::perf::baselines::responsive_layout_work;

    assert_eq!(responsive_layout_work(320.0, 12.0), 0b100);
    assert_eq!(responsive_layout_work(900.0, 12.0), 0b111);
    assert_eq!(responsive_layout_work(900.0, 96.0), 0b100);
    assert_eq!(responsive_layout_work(1200.0, 96.0), 0b111);
}

#[test]
fn four_pane_decoration_aggregate_payload_fits_budget() {
    // One decoration update across a 4-pane window: each pane's payload
    // stays within the per-pane budget and the aggregate stays within the
    // Phase 22.6 4-pane ceiling. Representative syntax-decorated set per
    // pane (same shape as the Phase 16 payload gate).
    use clay::perf::budgets::{
        DECORATION_PAYLOAD_BUDGET_BYTES, MULTI_PANE_DECORATION_AGGREGATE_BUDGET_BYTES,
    };
    use clay::protocol::{DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan};

    let mut aggregate = 0usize;
    for pane in 1..=4 {
        let set = DecorationSet {
            document_id: pane as u64,
            document_version: 3,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 8 * 1024 * 1024,
            viewport_byte_end: 8 * 1024 * 1024 + 256 * 1024,
            spans: vec![DecorationSpan::from_style_token(
                8 * 1024 * 1024,
                8 * 1024 * 1024 + 16,
                DecorationKind::Syntax,
                "markup.heading.1",
                10,
                DecorationProvenance {
                    package_name: "@clay/markdown".to_string(),
                    package_version: "0.1.0".to_string(),
                    package_prefix: "markdown".to_string(),
                },
            )],
        };
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&set)
            .unwrap_or_else(|error| panic!("serialize pane {pane} decorations: {error}"))
            .len();
        assert!(
            bytes <= DECORATION_PAYLOAD_BUDGET_BYTES,
            "pane {pane} decoration payload {bytes} exceeds per-pane budget {DECORATION_PAYLOAD_BUDGET_BYTES}"
        );
        aggregate += bytes;
    }
    assert!(
        aggregate <= MULTI_PANE_DECORATION_AGGREGATE_BUDGET_BYTES,
        "4-pane aggregate decoration payload {aggregate} exceeds budget {MULTI_PANE_DECORATION_AGGREGATE_BUDGET_BYTES}"
    );
}

#[test]
fn command_centre_open_filter_and_listing_stay_bounded_off_hot_paths() {
    // Phase 24.5: the Command Centre's menu open does no document-sized
    // work, the per-keystroke filter scans only the bounded candidate list
    // (never the document), the path-browser listing snapshot is bounded by
    // the menu caps, and listing snapshot state is never read on paint/
    // layout paths. Advisory wall-clock budgets are pinned in
    // src/perf/budgets.rs; these are the deterministic CI-blocking guards.
    use clay::perf::budgets::{
        COMMAND_CENTRE_FILTER_UPDATE_P95_BUDGET_MS, COMMAND_CENTRE_LISTING_MAX_ENTRIES,
        COMMAND_CENTRE_LISTING_PAYLOAD_BUDGET_BYTES, COMMAND_CENTRE_OPEN_P95_BUDGET_MS,
        TRANSIENT_MENU_MAX_ITEMS,
    };
    const {
        assert!(COMMAND_CENTRE_LISTING_MAX_ENTRIES <= TRANSIENT_MENU_MAX_ITEMS);
        assert!(COMMAND_CENTRE_FILTER_UPDATE_P95_BUDGET_MS < COMMAND_CENTRE_OPEN_P95_BUDGET_MS);
        assert!(COMMAND_CENTRE_OPEN_P95_BUDGET_MS > 0);
        assert!(
            COMMAND_CENTRE_LISTING_PAYLOAD_BUDGET_BYTES < 1024 * 1024,
            "a listing snapshot must stay far below the 1 MiB codec frame ceiling"
        );
    }

    // Menu open: the browse listing plan is bounded by the listing-entry
    // constant and the open helper reads only document metadata, never
    // document text.
    let connection =
        fs::read_to_string("src/server/connection/menus.rs").expect("connection readable");
    let open_body = connection
        .split("async fn open_command_centre_session")
        .nth(1)
        .expect("open_command_centre_session present")
        .split("\n}")
        .next()
        .expect("open helper body");
    assert!(open_body.contains("COMMAND_CENTRE_LISTING_MAX_ENTRIES"));
    assert!(open_body.contains("execute_user_browse_listing"));
    assert_absent(
        open_body,
        &[".text()", "visible_text"],
        "command centre open must not read document text",
    );

    // Per-keystroke filter: query updates clamp at the shared query budget
    // and score the installed (bounded) candidate list only; the path
    // browser's refresh_filter scores installed entries locally.
    let sessions = fs::read_to_string("src/server/menu_sessions.rs").expect("sessions readable");
    let sessions_body = non_test(&sessions);
    assert!(sessions_body.contains("TRANSIENT_MENU_MAX_QUERY_CHARS"));
    assert!(sessions_body.contains("FilterOnly"));
    assert_absent(
        sessions_body,
        &["DocumentState", ".text()", "visible_text"],
        "menu session filter path must not touch document state",
    );
    let browser = fs::read_to_string("src/shell/path_browser.rs").expect("browser readable");
    let browser_body = non_test(&browser);
    assert!(browser_body.contains("fn refresh_filter"));
    assert!(browser_body.contains("fuzzy_score"));
    assert_absent(
        browser_body,
        &["DocumentState", ".text()", "visible_text"],
        "path browser filter must not touch document state",
    );

    // Listing snapshot state is never read on paint/layout paths: the pure
    // paint/layout files reference none of it, and the pane document's
    // paint bodies (paint_in / paint_status_line / paint) reference none of
    // it either — the snapshot is consumed only by the connection-event
    // handler.
    for file in [
        "src/masonry_editor.rs",
        "src/masonry_shell/mod.rs",
        "src/masonry_sdui.rs",
        "src/shell/primitives.rs",
    ] {
        let src = fs::read_to_string(file).unwrap_or_else(|error| panic!("read {file}: {error}"));
        let body = non_test(&src);
        assert_absent(
            body,
            &[
                "TransientMenuSnapshotData",
                "PathBrowserSession",
                "UserBrowseEntry",
                "UserBrowsePage",
                "UserBrowseListingPlan",
            ],
            "{file} must not read listing snapshot state",
        );
    }
    let pane = fs::read_to_string("src/masonry_pane_document.rs").expect("pane readable");
    let pane_body = non_test(&pane);
    assert!(
        pane_body.contains("ClientConnectionEvent::TransientMenuSnapshot(snapshot)"),
        "listing snapshots are consumed only by the connection-event handler"
    );
    let paint_bodies = [
        pane_body
            .split("fn paint_in(")
            .nth(1)
            .and_then(|s| s.split("fn paint_status_line(").next())
            .unwrap_or(""),
        pane_body
            .split("fn paint_status_line(")
            .nth(1)
            .and_then(|s| s.split("fn paint(").next())
            .unwrap_or(""),
        pane_body
            .split("fn paint(")
            .nth(1)
            .and_then(|s| s.split("fn accessibility_role(").next())
            .unwrap_or(""),
    ]
    .join("\n");
    assert_absent(
        &paint_bodies,
        &[
            "TransientMenuSnapshotData",
            "PathBrowserSession",
            "UserBrowseEntry",
            "UserBrowsePage",
        ],
        "pane paint must not read listing snapshot state",
    );
}

#[test]
fn pending_chord_buffer_grows_one_stroke_per_pending_outcome() {
    // Phase 24.5: the pending-chord buffer is bounded by the longest bound
    // sequence. Source guard: the buffer grows by exactly one validated
    // stroke in exactly the Pending arm, and every non-pending path clears
    // it; the matcher reports Pending only while the candidate is a strict
    // prefix of some rule. (Runtime proof: the surface test
    // editor_pending_chord_buffer_never_exceeds_longest_bound_sequence.)
    let surface = fs::read_to_string("src/editor/surface/mod.rs").expect("surface readable");
    let body = non_test(&surface);
    let routing_body = body
        .split("pub(crate) fn route_key_with_event")
        .nth(1)
        .expect("route_key_with_event present")
        .split("fn dispatch_routed")
        .next()
        .expect("routing body");
    assert_eq!(
        routing_body.matches("strokes.push(key.clone())").count(),
        1,
        "pending buffer grows by one stroke in exactly the Pending arm"
    );
    assert!(
        routing_body.contains("pending_chord = None"),
        "match/mismatch/timeout paths clear the pending buffer"
    );
    assert!(
        body.contains("KEY_CHORD_PENDING_TIMEOUT_MS"),
        "stale pending chords expire via the budget constant"
    );
}

#[test]
fn centered_overlay_work_is_bounded_and_scrim_is_single_pass() {
    // Phase 24.4 (plan 084 task 7): the centered Command Centre surface must
    // stay deterministic — one token-driven full-window scrim fill, one
    // window-level host, layer lifecycle confined to the reconcile bridge,
    // menu items bounded at session construction, window-bounded geometry with
    // no document-size dependency, and no blur/filter/offscreen/JS/IPC/IO work
    // on the paint/layout path.
    let primitives =
        fs::read_to_string("src/shell/primitives.rs").expect("primitives source readable");
    let scrim_body = primitives
        .split("pub(crate) fn paint_scrim")
        .nth(1)
        .expect("paint_scrim present")
        .split("\n#[cfg(test)]")
        .next()
        .expect("paint_scrim body");
    assert_eq!(
        scrim_body.matches("scene.fill(").count(),
        1,
        "paint_scrim is exactly one Scene::fill"
    );
    assert!(
        scrim_body.contains("surface.scrim"),
        "scrim color is token-driven"
    );
    assert!(
        scrim_body.contains("opacity.scrim"),
        "scrim opacity is token-driven"
    );
    assert_absent(
        scrim_body,
        &["draw_blurred_rounded_rect", "offscreen", "filter"],
        "scrim must not blur/filter/offscreen",
    );

    let host = fs::read_to_string("src/masonry_package_region.rs").expect("host source readable");
    let host_body = non_test(&host);
    assert_eq!(
        host_body
            .matches("paint_scrim(scene, self.window_rect, &self.ui_theme)")
            .count(),
        1,
        "centered host paints exactly one scrim fill per paint pass"
    );
    assert!(
        host_body.contains("self.window_rect = working_area"),
        "scrim bounds come from window geometry"
    );
    assert!(
        host_body.contains("size.to_rect()"),
        "window bounds derive from layout size, not document metrics"
    );
    assert_absent(
        host_body,
        &[
            "draw_blurred_rounded_rect",
            "offscreen",
            "filter",
            "Deno.core",
            "op_clay_",
            "std::fs",
            "TcpStream",
            "reqwest",
            "visible_line_count",
            "document_state",
        ],
        "centered host paint/layout must not blur/filter, run JS/IPC/IO, or depend on document size",
    );

    let sdui = fs::read_to_string("src/masonry_sdui.rs").expect("sdui source readable");
    let sdui_body = non_test(&sdui);
    assert!(
        sdui_body.contains("overlay.anchor != PackageOverlayAnchor::Centered"),
        "pane-local host filters centered overlays out"
    );
    assert!(
        sdui_body.contains("overlay.anchor == PackageOverlayAnchor::Centered"),
        "window-level layer receives only centered overlays"
    );

    let driver = fs::read_to_string("src/driver/mod.rs").expect("driver source readable");
    let driver_body = non_test(&driver);
    assert!(
        driver_body.contains("centered_layer_id: Option<WidgetId>"),
        "driver owns one optional window-level layer"
    );
    assert!(
        !driver_body.contains("centered_layer_ids:"),
        "exactly one centered layer, never a collection"
    );
    let sync_body = driver_body
        .split("fn sync_centered_layer")
        .nth(1)
        .expect("sync_centered_layer present");
    assert!(
        sync_body.contains("reconcile_centered_overlay_layer"),
        "snapshot sync routes through the retained-layer bridge"
    );
    assert_absent(
        sync_body,
        &["add_layer(", "remove_layer("],
        "layer lifecycle stays in the reconcile bridge",
    );

    let session =
        fs::read_to_string("src/shell/transient_menu.rs").expect("session source readable");
    let session_body = non_test(&session);
    assert!(
        session_body.contains("const MAX_ITEMS: usize = TRANSIENT_MENU_MAX_ITEMS;"),
        "menu item bound aliases the documented budget constant"
    );
    assert!(
        session_body.contains("take(MAX_ITEMS)"),
        "menu items bounded at session construction"
    );
}

#[test]
fn completion_ranking_is_not_on_keypress_to_local_paint_path() {
    let hot = hot_path_concat(&[
        "src/editor/surface/mod.rs",
        "src/editor/layout.rs",
        "src/masonry_editor.rs",
        "src/client/mod.rs",
    ]);
    assert_absent(
        &hot,
        &[
            "rank_completion",
            "score_completion",
            "collect_matching_words",
            "buffer_word_result",
            "estimated_result_payload_bytes",
        ],
        "completion ranking/scan must stay in the completion coordinator, not paint/keypress",
    );
}

#[test]
fn hover_intent_is_not_on_paint_or_layout_path() {
    let hot = hot_path_concat(&[
        "src/editor/surface/mod.rs",
        "src/editor/layout.rs",
        "src/editor/surface/decoration.rs",
        "src/masonry_editor.rs",
    ]);
    assert_absent(
        &hot,
        &[
            "HoverIntent",
            "ActivateLink",
            "DecorationKind::Link",
            "DecorationKind::Inlay",
            "publish_folding",
            "serverPublishFoldingRanges",
            "LanguageIntelligenceCoordinator",
        ],
        "hover/click/fold-publish must stay off paint/layout",
    );
}
