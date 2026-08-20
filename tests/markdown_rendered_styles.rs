//! Rendered-output coverage for Markdown: grammar-produced spans fed through
//! the editor normalization seam must yield the expected visible style runs
//! (font role, attributes, color), not just emitted token types.

#![cfg(any(unix, windows))]

use clay::editor::EditorSurface;
use clay::editor::theme::StyleRegistry;
use clay::protocol::{
    DecorationKind, DocumentAccess, FontRole, Modifiers, ParseByteRange, ParseEditNotification,
    ParseWindowSnapshot, TokenType,
};
use clay::server::syntax::{SyntaxGrammarRegistry, TreeSitterSyntaxHandler};

use clay::editor::surface::VisibleTextStyleRunForTest as Run;

const DOC: &str = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n\nPlain prose here.\n\nMix **bold** and *ital* and `code` and [link](https://example.com) now.\n\n<https://example.com> and <a@b.c>\n\n> quoted text\n\n```rust\nfn main() {}\n```\n\nEmoji 🦀 tail **böld** end.\n";

fn markdown_handler() -> TreeSitterSyntaxHandler {
    let registry = SyntaxGrammarRegistry::with_first_party_native();
    let contribution = registry
        .get("markdown.markdown")
        .expect("native Markdown grammar")
        .clone();
    let mut handler = TreeSitterSyntaxHandler::new(
        contribution,
        tree_sitter_md_025::LANGUAGE.into(),
        include_str!("../packages/markdown/queries/highlights.scm"),
    )
    .expect("Markdown query compiles");
    handler
        .enable_injections(include_str!("../packages/markdown/queries/injections.scm"))
        .expect("Markdown injections query compiles");
    handler
}

fn parse_notification(version: u64, text: &str) -> ParseEditNotification {
    ParseEditNotification {
        document_id: 11,
        document_version: version,
        behavior_version: 4,
        package_prefix: "markdown".to_string(),
        mode_id: "markdown.markdown".to_string(),
        viewport: ParseByteRange::new(0, text.len() as u64),
        invalidated_ranges: vec![ParseByteRange::new(0, text.len() as u64)],
        accepted_edit: None,
        parse_windows: vec![ParseWindowSnapshot {
            document_id: 11,
            document_version: version,
            package_prefix: "markdown".to_string(),
            mode_id: "markdown.markdown".to_string(),
            window_id: 0,
            byte_start: 0,
            byte_end: text.len() as u64,
            base_line: 0,
            base_column: 0,
            incremental_edit: false,
            text: text.to_string(),
        }],
        memory_budget: None,
    }
}

fn render(text: &str, version: u64) -> EditorSurface {
    let handler = markdown_handler();
    let update = handler
        .parse_sync(parse_notification(version, text))
        .expect("Markdown parses");
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        11,
        version,
        text.to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    for set in update.decoration_updates {
        assert!(editor.apply_decoration_set(set));
    }
    editor
}

fn spec(token_type: TokenType, modifiers: Modifiers) -> clay::editor::theme::StyleSpec {
    StyleRegistry::clay_default().style_for(DecorationKind::Syntax, token_type, modifiers)
}

fn run_covering<'a>(runs: &'a [Run], source: &str, needle: &str) -> &'a Run {
    let start = source
        .find(needle)
        .unwrap_or_else(|| panic!("needle {needle:?}"));
    runs.iter()
        .find(|(range, ..)| range.start <= start && start < range.end)
        .unwrap_or_else(|| panic!("no run covers {needle:?} at byte {start}: {runs:?}"))
}

fn assert_run(
    runs: &[Run],
    source: &str,
    needle: &str,
    expected: clay::editor::theme::StyleSpec,
    font_role: FontRole,
) {
    let (_, role, [bold, italic, underline, strike], color, background, scale) =
        run_covering(runs, source, needle);
    assert_eq!(*role, font_role, "{needle:?} font role");
    assert_eq!(*color, Some(expected.color), "{needle:?} color");
    assert_eq!(*background, expected.background, "{needle:?} background");
    assert_eq!(*scale, expected.scale, "{needle:?} scale");
    assert_eq!(
        [*bold, *italic, *underline, *strike],
        [
            expected.bold,
            expected.italic,
            expected.underline,
            expected.strike
        ],
        "{needle:?} attributes"
    );
}

#[test]
fn markdown_visible_runs_style_all_constructs() {
    let editor = render(DOC, 1);
    let runs = editor.visible_text_style_runs_for_test();
    assert!(!runs.is_empty());

    // Six heading levels: per-level color, bold.
    for (level, needle, token_type) in [
        (1usize, "H1", TokenType::Heading1),
        (2, "H2", TokenType::Heading2),
        (3, "H3", TokenType::Heading3),
        (4, "H4", TokenType::Heading4),
        (5, "H5", TokenType::Heading5),
        (6, "H6", TokenType::Heading6),
    ] {
        assert_run(
            &runs,
            DOC,
            needle,
            spec(token_type, Modifiers::NONE),
            FontRole::Proportional,
        );
        let heading_color = run_covering(&runs, DOC, needle).3;
        for other_level in 1..level {
            let other = format!("H{other_level}");
            assert_ne!(
                heading_color,
                run_covering(&runs, DOC, &other).3,
                "heading levels {other_level} and {level} must differ"
            );
        }
    }

    // Plain prose at base text color, no attributes.
    assert_run(
        &runs,
        DOC,
        "Plain prose",
        spec(TokenType::Paragraph, Modifiers::NONE),
        FontRole::Proportional,
    );

    // Mixed inline run inside one paragraph.
    assert_run(
        &runs,
        DOC,
        "bold",
        spec(TokenType::Paragraph, Modifiers::BOLD),
        FontRole::Proportional,
    );
    assert_run(
        &runs,
        DOC,
        "ital",
        spec(TokenType::Paragraph, Modifiers::ITALIC),
        FontRole::Proportional,
    );
    assert_run(
        &runs,
        DOC,
        "`code`",
        spec(TokenType::CodeSpan, Modifiers::NONE),
        FontRole::Monospace,
    );
    assert_run(
        &runs,
        DOC,
        "[link](https://example.com)",
        spec(TokenType::Link, Modifiers::NONE),
        FontRole::Proportional,
    );

    // Autolinks render like links.
    for needle in ["<https://example.com>", "<a@b.c>"] {
        assert_run(
            &runs,
            DOC,
            needle,
            spec(TokenType::Link, Modifiers::NONE),
            FontRole::Proportional,
        );
    }

    // Quote marker and fenced code block.
    assert_run(
        &runs,
        DOC,
        "> quoted",
        spec(TokenType::Quote, Modifiers::NONE),
        FontRole::Proportional,
    );
    assert_run(
        &runs,
        DOC,
        "fn main() {}",
        spec(TokenType::CodeBlock, Modifiers::NONE),
        FontRole::Monospace,
    );

    // UTF-8 offsets: multibyte emoji before a strong run must not shift styling.
    assert_run(
        &runs,
        DOC,
        "böld",
        spec(TokenType::Paragraph, Modifiers::BOLD),
        FontRole::Proportional,
    );
    let plain = run_covering(&runs, DOC, "tail");
    assert_eq!(
        plain.3,
        Some(spec(TokenType::Paragraph, Modifiers::NONE).color),
        "prose after emoji stays base-colored"
    );
}

#[test]
fn markdown_visible_runs_survive_typing_scrolling_and_authority_replacement() {
    let mut editor = render(DOC, 1);
    let handler = markdown_handler();

    // Scroll through the decorated document; normalization stays stable.
    editor.scroll_lines(4);
    editor.scroll_lines(-4);
    let runs = editor.visible_text_style_runs_for_test();
    assert_run(
        &runs,
        DOC,
        "H1",
        spec(TokenType::Heading1, Modifiers::NONE),
        FontRole::Proportional,
    );

    // Type at the document start; decorations shift provisionally.
    editor.install_behavior_manifest(clay::protocol::BehaviorManifest::minimal_text_editing(1));
    assert!(editor.paste_text_with_event("Intro\n\n").changed);
    assert!(editor.note_confirmed_version(11, 2));

    // Authority re-parse of the edited document replaces the shifted chunks.
    let edited = format!("Intro\n\n{DOC}");
    let update = handler
        .parse_sync(parse_notification(2, &edited))
        .expect("edited Markdown parses");
    for set in update.decoration_updates {
        assert!(editor.apply_decoration_set(set));
    }

    let runs = editor.visible_text_style_runs_for_test();
    assert_run(
        &runs,
        &edited,
        "H1",
        spec(TokenType::Heading1, Modifiers::NONE),
        FontRole::Proportional,
    );
    assert_run(
        &runs,
        &edited,
        "[link](https://example.com)",
        spec(TokenType::Link, Modifiers::NONE),
        FontRole::Proportional,
    );
    assert_run(
        &runs,
        &edited,
        "`code`",
        spec(TokenType::CodeSpan, Modifiers::NONE),
        FontRole::Monospace,
    );
    // The typed intro is plain prose, not a heading continuation.
    assert_run(
        &runs,
        &edited,
        "Intro",
        spec(TokenType::Paragraph, Modifiers::NONE),
        FontRole::Proportional,
    );
}
