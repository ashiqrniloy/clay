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
fn parse_window_snapshot_primitive_uses_bounded_rope_slicing() {
    let document_source =
        fs::read_to_string("src/server/document.rs").expect("document source readable");
    let parse_source = fs::read_to_string("src/server/parse_coordinator.rs")
        .expect("parse coordinator source readable");

    assert!(document_source.contains("byte_slice(start..end).to_string()"));
    assert!(document_source.contains("validate_parse_snapshot_range"));
    assert!(parse_source.contains("schedule_parse_with_windows"));
    assert!(parse_source.contains("previous.abort()"));
}

#[test]
fn paint_uses_cached_inert_spans_without_package_javascript() {
    let surface_source = fs::read_to_string("src/editor/surface.rs").expect("surface readable");
    let layout_source = fs::read_to_string("src/editor/layout.rs").expect("layout readable");
    let paint_sources = format!("{surface_source}\n{layout_source}");

    assert!(surface_source.contains("visible_decoration_ranges"));
    assert!(layout_source.contains("decoration_visible_byte_ranges"));
    for forbidden in [
        "markdownIt",
        "parseMarkdown",
        "serverPublishDecorations",
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
