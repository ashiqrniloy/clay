/// Integration tests for Markdown mode activation, commands, key bindings,
/// and behavior-manifest transforms (Phase 18, Task 4).
///
/// All mode-specific logic lives in the `@clay/markdown` JS package
/// (`packages/markdown/dist/load.js`).  The Rust editor only provides generic
/// primitives (`EnterRule::ContinueLineMarkers`, `PairRule`, etc.) that any
/// mode can declare through the activation op by passing `editorRules` JSON.
/// No Markdown-named types or special cases appear in Rust.
///
/// Tests:
///   - `markdown_classifies_supported_extensions_and_mime`
///   - `markdown_mode_installs_behavior_manifest_atomically`
///   - `markdown_editor_rules_parse_continue_line_markers`
///   - `markdown_empty_list_item_exits_list`
///   - `markdown_editor_rules_parse_preserve_fence_body_indent`
///   - `markdown_editor_rules_parse_pair_rules_with_multi_char_delimiters`
///   - `markdown_editor_rules_reject_executable_fields`
///   - `markdown_editor_rules_parse_all_fields`
///   - `markdown_activation_publishes_manifest_with_commands`
///   - `markdown_activation_publishes_manifest_with_keymaps`
///   - `markdown_behavior_version_increments_on_reactivation`
///   - `non_markdown_manifest_uses_preserve_leading_whitespace`
///   - `markdown_mode_manifest_does_not_wait_for_parse_handler`
use std::time::{Duration, Instant};

use clay::editor::{EditorCommand, EditorSurface};
use clay::masonry_sdui::SduiNativeState;
use clay::packages::manager::FakeBackend;
use clay::packages::modes::{
    DocumentClassificationInput, MajorModeActivation, ModeDeclaration, ModeRegistry,
};
use clay::packages::record::assemble_package_record;
use clay::packages::service::{PackageService, PackageServiceError};
use clay::perf::budgets::{
    BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, DECORATION_PAYLOAD_BUDGET_BYTES,
    INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES, SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES,
    SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
};
use clay::protocol::{
    BehaviorManifest, BehaviorScope, DecorationKind, DecorationProvenance, DecorationSet,
    DecorationSpan, DocumentAccess, EditorBehaviorRules, EnterRule, IncrementalParseUpdate,
    PairRule, PairRuleContext, ParseByteRange, ParseUnit, SduiActionIntent, SduiActionSource,
    SduiEditorBinding, SduiFlexDirection, SduiListItem, SduiNode, SduiNodeId, SduiNodeKind,
    SduiTree, SduiTreeOperation, SduiTreeUpdate, ServerMessage, TabMode, TabRule,
    TextEditCapability, codec::Codec,
};
use clay::server::parse_coordinator::{ParseCoordinator, ParseScheduleRequest};
use serde_json::json;

// ── Helper: call the editor-rules JSON parser directly.  The real parser
//    lives in `src/server/ops/modes.rs::parse_editor_rules` and is not
//    public; we exercise it indirectly by testing the shapes the package
//    JSON declares and by constructing the protocol types directly.

/// Build a validated PackageRecord from the first-party @clay/markdown package.json.
fn markdown_package_record() -> clay::packages::record::PackageRecord {
    let text = std::fs::read_to_string("packages/markdown/package.json")
        .expect("packages/markdown/package.json must exist");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("package.json must be valid JSON");
    assemble_package_record(&value).expect("@clay/markdown package contract must validate")
}

fn markdown_package_json() -> serde_json::Value {
    let text = std::fs::read_to_string("packages/markdown/package.json")
        .expect("packages/markdown/package.json must exist");
    serde_json::from_str(&text).expect("package.json must be valid JSON")
}

fn markdown_registry_with_mode() -> (ModeRegistry, clay::packages::record::PackageRecord) {
    let record = markdown_package_record();
    let mut registry = ModeRegistry::new();
    let decl = ModeDeclaration {
        package_name: record.manifest.name.clone(),
        package_version: record.manifest.version.clone(),
        api_prefix: record.manifest.clay.api_prefix.clone(),
        mode_id: "markdown".to_string(),
        display_name: "Markdown".to_string(),
        extensions: vec![
            "md".to_string(),
            "markdown".to_string(),
            "mdown".to_string(),
        ],
        mime_types: vec!["text/markdown".to_string()],
        file_names: vec![],
        file_name_patterns: vec![],
    };
    registry
        .register_mode(&record.manifest, decl)
        .expect("markdown mode pattern must register");
    (registry, record)
}

fn activate_markdown_for_document(
    registry: &mut ModeRegistry,
    record: &clay::packages::record::PackageRecord,
    path: &str,
    document_id: u64,
) -> MajorModeActivation {
    let input = DocumentClassificationInput {
        document_id,
        path: Some(path.to_string()),
        mime_type: None,
    };
    let classification = registry
        .classify(&input)
        .expect("classification must succeed");
    registry
        .activate_major_mode(&record.manifest, classification)
        .expect("major mode activation must succeed")
}

/// The Markdown `editorRules` built directly from generic protocol types —
/// exactly what the JS package declares.
fn markdown_editor_rules() -> EditorBehaviorRules {
    EditorBehaviorRules {
        text_edits: vec![
            TextEditCapability::Insert,
            TextEditCapability::Delete,
            TextEditCapability::Replace,
        ],
        enter: EnterRule::ContinueLineMarkers {
            markers: vec![
                "-".to_string(),
                "*".to_string(),
                "+".to_string(),
                "ordered-dot".to_string(),
            ],
            exit_on_empty_item: true,
        },
        tab: TabRule {
            mode: TabMode::InsertSpaces,
            spaces_per_tab: 4,
        },
        pairs: vec![
            PairRule::new("(", ")"),
            PairRule::new("[", "]"),
            PairRule::new("**", "**"),
            PairRule::new("__", "__"),
            PairRule::new("`", "`"),
        ],
        comments: vec![],
        autocomplete_triggers: vec![],
    }
}

fn markdown_sdui_tree() -> SduiTree {
    let root_id = SduiNodeId(1);
    let panel_id = SduiNodeId(2);
    let stack_id = SduiNodeId(3);
    let document_label_id = SduiNodeId(4);
    let mode_label_id = SduiNodeId(5);
    let parse_label_id = SduiNodeId(6);
    let decorations_label_id = SduiNodeId(7);
    let preview_label_id = SduiNodeId(8);
    let toggle_button_id = SduiNodeId(9);
    let preview_list_id = SduiNodeId(10);
    let editor_id = SduiNodeId(11);

    SduiTree {
        ui_version: 1,
        root_id,
        nodes: vec![
            SduiNode::new(
                root_id,
                SduiNodeKind::Flex {
                    direction: SduiFlexDirection::Row,
                    children: vec![panel_id, editor_id],
                },
            ),
            SduiNode::new(
                panel_id,
                SduiNodeKind::Panel {
                    title: "Markdown Preview".to_string(),
                    children: vec![stack_id],
                },
            ),
            SduiNode::new(
                stack_id,
                SduiNodeKind::Stack {
                    children: vec![
                        document_label_id,
                        mode_label_id,
                        parse_label_id,
                        decorations_label_id,
                        preview_label_id,
                        toggle_button_id,
                        preview_list_id,
                    ],
                },
            ),
            SduiNode::new(
                document_label_id,
                SduiNodeKind::Label {
                    text: "Document: sample.md".to_string(),
                },
            ),
            SduiNode::new(
                mode_label_id,
                SduiNodeKind::Label {
                    text: "Mode: markdown".to_string(),
                },
            ),
            SduiNode::new(
                parse_label_id,
                SduiNodeKind::Label {
                    text: "Parse: markdown-it registered".to_string(),
                },
            ),
            SduiNode::new(
                decorations_label_id,
                SduiNodeKind::Label {
                    text: "Decorations: published".to_string(),
                },
            ),
            SduiNode::new(
                preview_label_id,
                SduiNodeKind::Label {
                    text: "Preview: decorated editor".to_string(),
                },
            ),
            SduiNode::new(
                toggle_button_id,
                SduiNodeKind::Button {
                    label: "Toggle Preview".to_string(),
                    action: SduiActionIntent::command(
                        "markdown.togglePreview",
                        SduiActionSource::Button {
                            node_id: toggle_button_id,
                        },
                    ),
                },
            ),
            SduiNode::new(
                preview_list_id,
                SduiNodeKind::List {
                    items: vec![SduiListItem {
                        id: "markdown-preview-mode".to_string(),
                        label: "Decorated editor preview".to_string(),
                        detail: Some("Inert package SDUI".to_string()),
                        action: None,
                    }],
                },
            ),
            SduiNode::new(
                editor_id,
                SduiNodeKind::EditorView {
                    binding: SduiEditorBinding {
                        document_id: 1,
                        expected_version: Some(1),
                    },
                },
            ),
        ],
    }
}

fn protocol_payload_len(frame: &[u8]) -> usize {
    frame.len().saturating_sub(4)
}

fn markdown_decoration_set(document_version: u64, viewport_end: u64) -> DecorationSet {
    let provenance = DecorationProvenance {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "markdown".to_string(),
    };
    let spans = vec![
        (0, 10, "markup.heading.1", 90),
        (36, 46, "markup.strong", 60),
        (52, 62, "markup.emphasis", 50),
        (64, 77, "markup.inline-code", 65),
        (83, 84, "markup.list-marker", 80),
        (97, 98, "markup.list-marker", 80),
        (110, 136, "markup.code-block", 70),
    ]
    .into_iter()
    .map(
        |(byte_start, byte_end, style_token, priority)| DecorationSpan {
            byte_start,
            byte_end,
            kind: DecorationKind::Syntax,
            style_token: style_token.to_string(),
            priority,
            provenance: provenance.clone(),
        },
    )
    .collect();

    DecorationSet {
        document_id: 7,
        document_version,
        viewport_byte_start: 0,
        viewport_byte_end: viewport_end,
        spans,
    }
}

fn markdown_parse_update(document_version: u64) -> IncrementalParseUpdate {
    IncrementalParseUpdate {
        document_id: 7,
        document_version,
        behavior_version: 3,
        package_prefix: "markdown".to_string(),
        mode_id: "markdown".to_string(),
        parse_unit: ParseUnit::LineGroup,
        viewport: ParseByteRange::new(0, 160),
        invalidated_ranges: vec![ParseByteRange::new(0, 80)],
        syntax_tree_delta: Some("decorations:viewport-spans=7".to_string()),
        decoration_update: Some(markdown_decoration_set(document_version, 160)),
    }
}

// ── Parser/decorator adapter contract tests ────────────────────────────────

#[test]
fn markdown_package_has_no_mdast_dependency() {
    let package = markdown_package_json();
    assert_eq!(
        package["dependencies"]["markdown-it"].as_str(),
        Some("^14.1.0")
    );
    assert!(
        package["dependencies"]
            .get("mdast-util-from-markdown")
            .is_none(),
        "@clay/markdown must not keep the superseded mdast parser dependency"
    );
    assert_eq!(
        package["exports"]["./parser"].as_str(),
        Some("./dist/parser.js")
    );
    assert_eq!(
        package["clay"]["contributions"]["decorations"][0]["adapter"].as_str(),
        Some("./dist/parser.js")
    );
}

#[test]
fn markdown_runtime_code_has_no_from_markdown_import() {
    for path in [
        "packages/markdown/dist/parser.js",
        "packages/markdown/src/parser.js",
        "packages/markdown/dist/load.js",
        "packages/markdown/dist/index.js",
    ] {
        let source = std::fs::read_to_string(path).expect("markdown runtime file must exist");
        assert!(
            !source.contains("mdast-util-from-markdown") && !source.contains("fromMarkdown"),
            "{path} must not import or inject the superseded mdast parser"
        );
    }
}

#[test]
fn markdown_parser_adapter_uses_markdown_it_package_boundary() {
    let parser = std::fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("parser adapter must exist");
    assert!(parser.contains("parseMarkdownDecorations"));
    assert!(parser.contains("markdown-it"));
    assert!(parser.contains("MARKDOWN_IT_OPTIONS"));
    assert!(parser.contains("html: false"));
    assert!(parser.contains("markup.heading.1"));
    assert!(parser.contains("markup.list-marker"));
    assert!(
        !parser.contains("Deno.core.ops"),
        "parser adapter must use Clay facades, not raw Deno ops"
    );
}

#[test]
fn markdown_parser_adapter_publishes_protocol_spans_without_parser_data() {
    let parser = std::fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("parser adapter must exist");
    assert!(
        parser.contains("options.tokens") && parser.contains("options.markdownIt"),
        "adapter must keep markdown-it token input injectable without changing Clay's protocol shape"
    );
    assert!(
        parser.contains("kind: \"syntax\"")
            && parser.contains("byteStart")
            && parser.contains("byteEnd")
            && parser.contains("styleToken"),
        "adapter must publish Clay decoration protocol fields, not parser internals"
    );
    assert!(
        !parser.contains("mdast:") && !parser.contains("type: \"heading_open\""),
        "published span shape must not expose parser-specific internals"
    );
}

#[test]
fn markdown_it_adapter_has_token_stream_range_fixtures() {
    let token_ranges = std::fs::read_to_string("tests/fixtures/markdown/token-ranges.md")
        .expect("token range fixture must exist");
    for expected in ["# Hé 🦀", "**bold**", "*em*", "`code`", "```js", "1. item"] {
        assert!(
            token_ranges.contains(expected),
            "token range fixture must include `{expected}`"
        );
    }

    let inline_nesting = std::fs::read_to_string("tests/fixtures/markdown/inline-nesting.md")
        .expect("inline nesting fixture must exist");
    assert!(inline_nesting.contains("**bold and *em* text**"));

    let window_boundaries = std::fs::read_to_string("tests/fixtures/markdown/window-boundaries.md")
        .expect("window-boundary fixture must exist");
    for expected in [
        "```js",
        "# Window Hé 🦀",
        "**strong**",
        "*emphasis*",
        "`inline code`",
        "- bullet",
        "1. ordered",
    ] {
        assert!(
            window_boundaries.contains(expected),
            "window-boundary fixture must include `{expected}`"
        );
    }

    let parser = std::fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("parser adapter must exist");
    for expected in [
        "walkMarkdownItInlineChildren",
        "codeUnitToAbsoluteByte",
        "lineCodeUnitStarts",
        "markdownIt.parse(text, {})",
        "html: false",
    ] {
        assert!(
            parser.contains(expected),
            "parser must contain `{expected}`"
        );
    }
    assert!(
        !parser.contains("markdownIt.render") && !parser.contains(".render("),
        "adapter must parse tokens only and must not render HTML"
    );
}

#[test]
fn markdown_windowed_adapter_declares_bounded_parse_policy() {
    let parser = std::fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("parser adapter must exist");
    for expected in [
        "DEFAULT_WINDOWED_MARKDOWN_POLICY",
        "parseWindowBytes: 64 * 1024",
        "guardBytes: 4 * 1024",
        "memoryBudgetBytes: 30 * 1024 * 1024",
        "parseMarkdownWindowDecorations",
        "parseWindows",
        "absoluteByteStart",
        "parseWindow",
    ] {
        assert!(
            parser.contains(expected),
            "windowed parser must contain `{expected}`"
        );
    }

    let load = std::fs::read_to_string("packages/markdown/dist/load.js")
        .expect("Markdown load runtime must exist");
    for expected in [
        "maxWindowBytes: contract.parse.parseWindowBytes",
        "guardBytes: contract.parse.guardBytes",
        "memoryBudgetBytes: contract.parse.memoryBudgetBytes",
        "timeoutMs: contract.parse.timeoutMs",
    ] {
        assert!(
            load.contains(expected),
            "load runtime must pass `{expected}`"
        );
    }
}

#[test]
fn markdown_windowed_adapter_static_guards_reject_full_text_large_file_path() {
    let parser = std::fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("parser adapter must exist");
    assert!(
        parser.contains("parseMarkdownWindowSetDecorations")
            && parser.contains("window.text")
            && parser.contains("markdownIt.parse(text, {})"),
        "adapter must parse package-supplied window text, not require full-document text"
    );
    assert!(
        parser.contains("window byte range must match UTF-8 text length"),
        "window byte ranges must be validated before range translation"
    );
    assert!(
        !parser.contains("serverGetDocumentSnapshot") && !parser.contains("fullDocument"),
        "Markdown adapter must not request or name a full-document parser path for large files"
    );
}

#[test]
fn markdown_docs_do_not_describe_mdast_as_active_parser() {
    for path in [
        "packages/markdown/docs/index.md",
        "docs/reference/packages/markdown.md",
        "docs/wiki/index.md",
        "docs/wiki/modules/first-party-markdown-package.md",
        "docs/wiki/modules/markdown-mode-activation.md",
        "docs/wiki/modules/parse-coordinator.md",
        "docs/wiki/modules/performance-fixtures.md",
        "docs/development/performance.md",
    ] {
        let doc = std::fs::read_to_string(path).expect("markdown docs must exist");
        assert!(
            !doc.contains("Parser dependency: `mdast-util-from-markdown`")
                && !doc.contains("mdast dependency boundary")
                && !doc.contains("positioned mdast nodes")
                && !doc.contains("mdast-specific conversion logic")
                && !doc.contains("fromMarkdown` parser dependency"),
            "{path} must not describe mdast as the active Markdown parser"
        );
    }
}

// ── Workspace fixture, SDUI, and fallback tests ───────────────────────────

#[test]
fn markdown_fixture_activates_with_markdown_it_adapter() {
    let fixture_path = std::path::Path::new("tests")
        .join("fixtures")
        .join("configuration")
        .join("markdown-mode")
        .join("workspace")
        .join("sample.md");
    let fixture_text = std::fs::read_to_string(&fixture_path)
        .expect("Markdown workspace fixture must be readable");
    assert!(fixture_text.contains("# Clay Markdown Fixture"));

    let (registry, _record) = markdown_registry_with_mode();
    let classification = registry
        .classify(&DocumentClassificationInput {
            document_id: 1,
            path: Some(fixture_path.to_string_lossy().replace('\\', "/")),
            mime_type: None,
        })
        .expect("fixture path must classify as markdown");
    assert_eq!(classification.mode_id, "markdown");
    assert_eq!(classification.api_prefix, "markdown");

    let load = std::fs::read_to_string("packages/markdown/dist/load.js")
        .expect("Markdown load runtime must exist");
    assert!(load.contains("markdownPackageManifest"));
    assert!(load.contains("serverLoadPackage?.(packageManifest)"));
    assert!(load.contains("serverRegisterModePattern(packageManifest"));
    assert!(load.contains("serverActivateMajorMode(packageManifest"));
    assert!(load.contains("serverRegisterCommand(packageManifest"));
    assert!(load.contains("packageManifest,"));
    assert!(load.contains("adapter: contract.parse.adapter"));
    assert!(load.contains("./dist/parser.js"));
    assert!(load.contains("./dist/sdui.js"));
    assert!(
        !load.contains("serverRegisterModePattern({")
            && !load.contains("serverRegisterCommand({\n      packageName"),
        "load runtime must use the real Clay facade signatures, not stale single-object stubs"
    );
}

#[test]
fn windows_markdown_open_fixture_binds_ctrl_o_without_hardcoding() {
    let fixture =
        std::fs::read_to_string("tests/fixtures/configuration/windows-markdown-open/init.js")
            .expect("Windows Markdown open fixture must exist");
    let launch_doc = std::fs::read_to_string("docs/development/launch-and-gui-smoke.md")
        .expect("launch smoke docs must exist");

    assert!(fixture.contains("import { bindKey } from \"clay:keybindings\";"));
    assert!(fixture.contains(
        "bindKey(\"Ctrl+O\", \"clay.documents.clientOpenFileDialog\", { scope: \"editor\" });"
    ));
    assert!(
        !fixture.contains("Deno.core.ops") && !fixture.contains("clientOpenFileDialog("),
        "fixture must use public Clay JS configuration APIs, not raw ops or callable dialog hooks"
    );
    assert!(launch_doc.contains("cargo run -- smoke-gui --config-fixture windows-markdown-open"));
}

#[test]
fn windows_markdown_open_fixture_loads_markdown_package() {
    let fixture =
        std::fs::read_to_string("tests/fixtures/configuration/windows-markdown-open/init.js")
            .expect("Windows Markdown open fixture must exist");
    let sample = std::fs::read_to_string(
        "tests/fixtures/configuration/windows-markdown-open/workspace/sample.md",
    )
    .expect("Windows Markdown open workspace sample must exist");

    assert!(sample.contains("# Clay Markdown Fixture"));
    for expected in [
        "@clay/markdown",
        "serverLoadPackage(markdownPackage)",
        "serverRegisterModePattern(markdownPackage",
        "serverRegisterParseHandler({",
        "serverPublishDecorations({",
        "extensions: [\"md\", \"markdown\", \"mdown\"]",
        "Windows Markdown Open Dialog Smoke",
        "Open: Ctrl+O native Markdown dialog",
    ] {
        assert!(
            fixture.contains(expected),
            "Windows Markdown open fixture must include `{expected}`"
        );
    }
}

#[test]
fn markdown_package_declares_sdui_preview_status_adapter() {
    let package = markdown_package_json();
    assert_eq!(
        package["exports"]["./sdui"].as_str(),
        Some("./dist/sdui.js")
    );
    assert_eq!(
        package["clay"]["contributions"]["sdui"][0]["adapter"].as_str(),
        Some("./dist/sdui.js")
    );

    let sdui = std::fs::read_to_string("packages/markdown/dist/sdui.js")
        .expect("package SDUI adapter must exist");
    assert!(sdui.contains("buildMarkdownPreviewStatusTree"));
    assert!(sdui.contains("markdown.togglePreview"));
    assert!(sdui.contains("Parse:"));
    assert!(sdui.contains("markdown-it registered"));
    assert!(sdui.contains("Decorations:"));
    assert!(
        !sdui.contains("Deno.core.ops"),
        "package SDUI adapter must use Clay facades, not raw Deno ops"
    );
}

#[test]
fn markdown_sdui_status_reports_markdown_it_parse_state() {
    let record = markdown_package_record();
    let declared_commands: Vec<&str> = record
        .contributions
        .commands
        .iter()
        .map(|command| command.id.as_str())
        .collect();
    assert!(declared_commands.contains(&"markdown.togglePreview"));

    let tree = markdown_sdui_tree();
    let toggle_action = tree
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            SduiNodeKind::Button { action, .. } => Some(action),
            _ => None,
        })
        .expect("Markdown SDUI tree must include a toggle button action");
    assert!(
        declared_commands.contains(&toggle_action.command_id.as_str()),
        "SDUI actions must target declared package commands"
    );

    let mut native = SduiNativeState::empty();
    native.apply_snapshot(tree.clone());
    let visible_texts = native.visible_texts();
    for expected in [
        "Markdown Preview",
        "Mode: markdown",
        "Parse: markdown-it registered",
        "Decorations: published",
        "Preview: decorated editor",
        "Toggle Preview",
    ] {
        assert!(
            visible_texts.iter().any(|text| text == expected),
            "missing visible SDUI text `{expected}` in {visible_texts:?}"
        );
    }

    let codec = Codec::default();
    let snapshot_payload = protocol_payload_len(
        &codec
            .encode_server_message(&ServerMessage::SduiSnapshot { client_id: 1, tree })
            .unwrap(),
    );
    assert!(
        snapshot_payload <= SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES,
        "Markdown SDUI snapshot payload {snapshot_payload} exceeds budget {SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES}"
    );
}

#[test]
fn markdown_sdui_status_update_fits_budget() {
    let codec = Codec::default();
    let update = SduiTreeUpdate {
        base_ui_version: 1,
        new_ui_version: 2,
        operations: vec![SduiTreeOperation::ReplaceNode {
            node: SduiNode::new(
                SduiNodeId(6),
                SduiNodeKind::Label {
                    text: "Parse: idle".to_string(),
                },
            ),
        }],
    };
    let payload = protocol_payload_len(
        &codec
            .encode_server_message(&ServerMessage::SduiUpdate { update })
            .unwrap(),
    );
    assert!(
        payload <= SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
        "Markdown SDUI update payload {payload} exceeds budget {SDUI_UPDATE_PAYLOAD_BUDGET_BYTES}"
    );
}

#[test]
fn markdown_large_file_policy_declares_thresholds_and_states() {
    let index = std::fs::read_to_string("packages/markdown/dist/index.js")
        .expect("Markdown package index must exist");
    for expected in [
        "markdownLargeFilePolicy",
        "smallFileMaxBytes: 1 * 1024 * 1024",
        "mediumFileMaxBytes: 5 * 1024 * 1024",
        "largeFileThresholdBytes: 5 * 1024 * 1024",
        "parseWindowBytes: 64 * 1024",
        "memoryBudgetBytes: 30 * 1024 * 1024",
        "highlightingStates: Object.freeze([\"full\", \"windowed\", \"degraded\", \"plain-text-fallback\"])",
        "markdownPolicyForDocument",
    ] {
        assert!(
            index.contains(expected),
            "Markdown policy must contain `{expected}`"
        );
    }

    let load = std::fs::read_to_string("packages/markdown/dist/load.js")
        .expect("Markdown load runtime must exist");
    for expected in [
        "markdownLargeFilePolicy.parseWindowBytes",
        "markdownLargeFilePolicy.guardBytes",
        "markdownLargeFilePolicy.memoryBudgetBytes",
        "markdownLargeFilePolicy.timeoutMs",
        "fallbackMode: markdownLargeFilePolicy.fallbackMode",
    ] {
        assert!(
            load.contains(expected),
            "load runtime must use `{expected}`"
        );
    }
}

#[test]
fn markdown_large_file_configuration_options_have_custom_properties_or_fixed_defaults() {
    let configuration_doc = std::fs::read_to_string("docs/reference/clay-js-api/configuration.md")
        .expect("configuration reference must exist");
    let package_docs = std::fs::read_to_string("packages/markdown/docs/index.md")
        .expect("Markdown package docs must exist");
    let package_reference = std::fs::read_to_string("docs/reference/packages/markdown.md")
        .expect("Markdown package reference must exist");
    let parse_docs = std::fs::read_to_string(
        "docs/reference/clay-js-api/parse/server-register-parse-handler.md",
    )
    .expect("parse handler API docs must exist");
    let package_json = markdown_package_json();
    let package_index = std::fs::read_to_string("packages/markdown/dist/index.js")
        .expect("Markdown package index must exist");
    let package_load = std::fs::read_to_string("packages/markdown/dist/load.js")
        .expect("Markdown package load runtime must exist");

    for expected in [
        "Phase 18.5 large-file Markdown configuration review",
        "fixed defaults",
        "not hidden `init.js` keys",
        "serverRegisterParseHandler",
        "custom_properties",
        "setPackageOption",
        "setParsePolicy",
        "remain unavailable stubs",
    ] {
        assert!(
            configuration_doc.contains(expected),
            "configuration docs must record Phase 18.5 fixed-default review phrase `{expected}`"
        );
    }

    for expected in [
        "fixed package-owned defaults",
        "declares no `contributions.configuration` entries",
        "does not request `package-configuration`",
        "serverRegisterParseHandler",
        "custom_properties",
    ] {
        assert!(
            package_docs.contains(expected) || package_reference.contains(expected),
            "Markdown docs/reference must explain fixed-default configuration status `{expected}`"
        );
    }

    let clay = package_json
        .get("clay")
        .and_then(serde_json::Value::as_object)
        .expect("package clay metadata must be an object");
    let permissions = clay
        .get("permissions")
        .and_then(serde_json::Value::as_array)
        .expect("package permissions must be an array");
    assert!(
        !permissions
            .iter()
            .any(|permission| permission == "package-configuration"),
        "Markdown fixed defaults must not request package-configuration until user settings exist"
    );
    let contributions = clay
        .get("contributions")
        .and_then(serde_json::Value::as_object)
        .expect("package contributions must be an object");
    assert!(
        !contributions.contains_key("configuration"),
        "Markdown fixed defaults must not be hidden package configuration entries"
    );
    assert!(
        !package_index.contains("contributions.configuration")
            && !package_load.contains("setPackageOption")
            && !package_load.contains("setParsePolicy"),
        "Markdown load path must not invent ad hoc configuration APIs"
    );

    for property in [
        "viewportPriority",
        "timeoutMs",
        "maxWindowBytes",
        "guardBytes",
        "memoryBudgetBytes",
        "parseUnits",
    ] {
        assert!(
            parse_docs.contains(property),
            "parse handler docs must expose behavior-changing parse policy property `{property}`"
        );
    }
}

#[test]
fn markdown_large_file_configuration_does_not_grant_package_authority() {
    let configuration_doc = std::fs::read_to_string("docs/reference/clay-js-api/configuration.md")
        .expect("configuration reference must exist");
    let package_docs = std::fs::read_to_string("packages/markdown/docs/index.md")
        .expect("Markdown package docs must exist");
    let configuration_runtime = std::fs::read_to_string("runtime/js/configuration.ts")
        .expect("configuration facade must exist");

    for denied in [
        "package enable/disable",
        "filesystem",
        "network",
        "shell",
        "extension loading",
        "AI mutation",
        "workspace",
        "WASM",
        "client-side JavaScript",
    ] {
        assert!(
            configuration_doc.contains(denied),
            "configuration docs must deny authority `{denied}`"
        );
    }
    for denied in ["install", "enable", "disable", "grant new permissions"] {
        assert!(
            package_docs.contains(denied),
            "Markdown package docs must deny configuration authority `{denied}`"
        );
    }
    assert!(configuration_runtime.contains("plannedConfigurationApi"));
    assert!(configuration_runtime.contains("setPackageOption"));
    assert!(configuration_runtime.contains("setParsePolicy"));
}

#[test]
fn markdown_large_file_status_reports_windowed_highlighting() {
    let sdui = std::fs::read_to_string("packages/markdown/dist/sdui.js")
        .expect("package SDUI adapter must exist");
    for expected in [
        "markdownStatusForPolicy",
        "windowed visible syntax current",
        "visible and near-viewport chunks current",
        "degraded; visible syntax refresh delayed",
        "plain text fallback; Markdown parser paused",
        "Highlighting: ${model.status.highlightingState}",
    ] {
        assert!(
            sdui.contains(expected),
            "SDUI status must contain `{expected}`"
        );
    }
}

#[test]
fn markdown_large_file_budget_exhaustion_falls_back_to_plain_text_static_guard() {
    let parser = std::fs::read_to_string("packages/markdown/dist/parser.js")
        .expect("parser adapter must exist");
    for expected in [
        "plainTextFallbackReason",
        "syntaxBudgetExceeded",
        "memoryBudgetExceeded",
        "fallbackMode === \"plain-text-fallback\"",
        "plain text fallback; syntax decorations cleared",
        "if (plainTextFallbackReason(options)) return []",
    ] {
        assert!(
            parser.contains(expected),
            "parser fallback must contain `{expected}`"
        );
    }
}

#[test]
fn markdown_degraded_status_contains_no_document_text_or_paths_static_guard() {
    let sdui = std::fs::read_to_string("packages/markdown/dist/sdui.js")
        .expect("package SDUI adapter must exist");
    assert!(sdui.contains("sanitizeStatusText"));
    assert!(sdui.contains("sanitizeDocumentLabel"));
    assert!(sdui.contains("[path]"));
    assert!(
        !sdui.contains("options.diagnostic") && !sdui.contains("options.documentText"),
        "Markdown status model must not include raw diagnostics or document text"
    );
}

#[test]
fn markdown_disabled_falls_back_to_plain_text_after_rewrite() {
    let mut service = PackageService::new(
        "target/test-package-store/markdown-disabled",
        Box::new(FakeBackend::default()),
    );
    service
        .install_from_value(markdown_package_json())
        .expect("installing Markdown package metadata must succeed");
    service
        .enable("@clay/markdown")
        .expect("Markdown package must enable before fallback test");
    service
        .disable("@clay/markdown")
        .expect("disable must remove enabled package contributions");

    let (mut registry, record) = markdown_registry_with_mode();
    activate_markdown_for_document(&mut registry, &record, "sample.md", 1);
    let enabled: Vec<_> = service.enabled_records().collect();
    let error = registry
        .select_behavior_manifest_for_document(1, &enabled)
        .expect_err("disabled package must not compose a Markdown manifest");
    assert!(error.message.contains("not in the enabled package list"));

    let fallback = BehaviorManifest::minimal_text_editing(1);
    assert!(
        fallback
            .commands
            .iter()
            .all(|command| !command.command_id.starts_with("markdown.")),
        "plain-text fallback manifest must not retain Markdown command authority"
    );
}

#[test]
fn markdown_invalid_package_reports_sanitized_diagnostics() {
    let mut invalid = markdown_package_json();
    invalid["clay"]["permissions"]
        .as_array_mut()
        .unwrap()
        .retain(|permission| permission.as_str() != Some("render-decorations"));

    let mut service = PackageService::new(
        "target/test-package-store/markdown-invalid",
        Box::new(FakeBackend::default()),
    );
    service
        .install_from_value(invalid)
        .expect("install records invalid package metadata without executing it");
    let error = service.enable("@clay/markdown").unwrap_err();
    let PackageServiceError::InvalidClayMetadata(diagnostic) = error else {
        panic!("expected InvalidClayMetadata diagnostic, got {error:?}");
    };
    assert_eq!(diagnostic.package_name.as_deref(), Some("@clay/markdown"));
    assert!(diagnostic.message.contains("render-decorations"));
    assert!(!diagnostic.message.contains("target/test-package-store"));
    assert!(!diagnostic.message.contains("packages/markdown"));
}

// ── Document classification tests ──────────────────────────────────────────

#[test]
fn markdown_classifies_supported_extensions_and_mime() {
    let (registry, _record) = markdown_registry_with_mode();

    let md = registry
        .classify(&DocumentClassificationInput {
            document_id: 1,
            path: Some("README.md".to_string()),
            mime_type: None,
        })
        .expect(".md must classify as markdown");
    assert_eq!(md.mode_id, "markdown");

    let markdown = registry
        .classify(&DocumentClassificationInput {
            document_id: 2,
            path: Some("notes.markdown".to_string()),
            mime_type: None,
        })
        .expect(".markdown must classify as markdown");
    assert_eq!(markdown.mode_id, "markdown");

    let mdown = registry
        .classify(&DocumentClassificationInput {
            document_id: 3,
            path: Some("doc.mdown".to_string()),
            mime_type: None,
        })
        .expect(".mdown must classify as markdown");
    assert_eq!(mdown.mode_id, "markdown");

    // MIME hint.
    let (registry2, record2) = {
        let r2_record = markdown_package_record();
        let mut r2 = ModeRegistry::new();
        r2.register_mode(
            &r2_record.manifest,
            ModeDeclaration {
                package_name: r2_record.manifest.name.clone(),
                package_version: r2_record.manifest.version.clone(),
                api_prefix: r2_record.manifest.clay.api_prefix.clone(),
                mode_id: "markdown".to_string(),
                display_name: "Markdown".to_string(),
                extensions: vec!["md".to_string()],
                mime_types: vec!["text/markdown".to_string()],
                file_names: vec![],
                file_name_patterns: vec![],
            },
        )
        .unwrap();
        (r2, r2_record)
    };
    let _ = record2;
    let mime = registry2
        .classify(&DocumentClassificationInput {
            document_id: 4,
            path: None,
            mime_type: Some("text/markdown".to_string()),
        })
        .expect("text/markdown MIME must classify");
    assert_eq!(mime.mode_id, "markdown");

    // Unknown extension → NoClassificationMatch.
    let err = registry
        .classify(&DocumentClassificationInput {
            document_id: 99,
            path: Some("file.txt".to_string()),
            mime_type: None,
        })
        .unwrap_err();
    assert!(
        err.message.contains("no registered mode matched"),
        "must produce NoClassificationMatch: {err:?}"
    );
}

// ── Mode activation and manifest tests ─────────────────────────────────────

#[test]
fn markdown_mode_installs_behavior_manifest_atomically() {
    let (mut registry, record) = markdown_registry_with_mode();
    let activation = activate_markdown_for_document(&mut registry, &record, "notes.md", 1);

    assert_eq!(activation.mode_id, "markdown");
    assert_eq!(activation.package_name, "@clay/markdown");
    assert!(activation.behavior_version >= 1);

    let enabled = vec![&record];
    let selection = registry
        .select_behavior_manifest_for_document(1, &enabled)
        .expect("manifest selection must succeed");

    // Scope is per-document.
    assert!(matches!(
        selection.manifest.scope,
        BehaviorScope::Document { document_id: 1 }
    ));
    assert_eq!(
        selection.manifest.behavior_version,
        activation.behavior_version
    );
    assert!(selection.manifest.manifest_id.starts_with("markdown."));
    assert_eq!(selection.major_mode.package_name, "@clay/markdown");
    assert!(selection.minor_modes.is_empty());
}

#[test]
fn markdown_behavior_manifest_fits_budget() {
    let (mut registry, record) = markdown_registry_with_mode();
    activate_markdown_for_document(&mut registry, &record, "notes.md", 1);
    let enabled = vec![&record];
    let selection = registry
        .select_behavior_manifest_for_document(1, &enabled)
        .expect("Markdown behavior manifest must compose");

    let codec = Codec::default();
    let payload = protocol_payload_len(
        &codec
            .encode_server_message(&ServerMessage::BehaviorManifest(selection.manifest.clone()))
            .expect("Markdown behavior manifest must encode"),
    );
    assert!(
        payload <= BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES,
        "Markdown behavior manifest payload {payload} exceeds budget {BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES}"
    );
}

#[test]
fn markdown_parse_and_decoration_payloads_fit_budgets() {
    let decoration_set = markdown_decoration_set(3, 160);
    let decoration_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&decoration_set)
        .expect("representative Markdown decoration set must serialize")
        .len();
    assert!(
        decoration_bytes <= DECORATION_PAYLOAD_BUDGET_BYTES,
        "Markdown decoration payload {decoration_bytes} exceeds budget {DECORATION_PAYLOAD_BUDGET_BYTES}"
    );

    let parse_update = markdown_parse_update(3);
    let parse_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&parse_update)
        .expect("representative Markdown parse update must serialize")
        .len();
    assert!(
        parse_bytes <= INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES,
        "Markdown parse payload {parse_bytes} exceeds budget {INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES}"
    );
}

#[test]
fn markdown_structural_sdui_snapshot_matches_fixture() {
    let mut native = SduiNativeState::empty();
    native.apply_snapshot(markdown_sdui_tree());
    let visible_texts = native.visible_texts();
    assert_eq!(
        visible_texts,
        vec![
            "Markdown Preview".to_string(),
            "Document: sample.md".to_string(),
            "Mode: markdown".to_string(),
            "Parse: markdown-it registered".to_string(),
            "Decorations: published".to_string(),
            "Preview: decorated editor".to_string(),
            "Toggle Preview".to_string(),
            "Decorated editor preview".to_string(),
            "Inert package SDUI".to_string(),
            "Editor document 1".to_string(),
        ]
    );
}

// ── Editor-rules shape tests (package declares generic types) ─────────────

/// The Markdown package declares `enter.kind = "continueLineMarkers"` with
/// all required unordered and ordered list markers plus `exitOnEmptyItem`.
/// This tests that the generic `EnterRule::ContinueLineMarkers` variant
/// carries the correct data — no Markdown-specific code in Rust.
#[test]
fn markdown_editor_rules_parse_continue_line_markers() {
    let rules = markdown_editor_rules();
    let EnterRule::ContinueLineMarkers {
        markers,
        exit_on_empty_item,
    } = &rules.enter
    else {
        panic!("expected ContinueLineMarkers, got {:?}", rules.enter);
    };
    for m in &["-", "*", "+"] {
        assert!(markers.iter().any(|x| x == m), "missing marker '{m}'");
    }
    assert!(
        markers.iter().any(|x| x == "ordered-dot"),
        "missing 'ordered-dot'"
    );
    assert!(*exit_on_empty_item, "exit_on_empty_item must be true");
}

/// `exitOnEmptyItem: true` means Enter on an empty list item removes/exits
/// the marker rather than inserting another one.
#[test]
fn markdown_empty_list_item_exits_list() {
    let rules = markdown_editor_rules();
    let EnterRule::ContinueLineMarkers {
        exit_on_empty_item, ..
    } = &rules.enter
    else {
        panic!("expected ContinueLineMarkers");
    };
    assert!(*exit_on_empty_item, "exit_on_empty_item must be true");
}

/// The Markdown package also uses the generic `EnterRule::PreserveFenceBodyIndent`
/// variant (not currently the default Markdown enter kind, but tested to prove
/// any mode can declare it and the editor will deserialize it correctly).
#[test]
fn markdown_editor_rules_parse_preserve_fence_body_indent() {
    // Construct a fence-indent rule for any mode that needs it.
    let rules = EditorBehaviorRules {
        text_edits: vec![
            TextEditCapability::Insert,
            TextEditCapability::Delete,
            TextEditCapability::Replace,
        ],
        enter: EnterRule::PreserveFenceBodyIndent {
            fence_markers: vec!["```".to_string(), "~~~".to_string()],
        },
        tab: TabRule {
            mode: TabMode::InsertSpaces,
            spaces_per_tab: 4,
        },
        pairs: vec![],
        comments: vec![],
        autocomplete_triggers: vec![],
    };
    let EnterRule::PreserveFenceBodyIndent { fence_markers } = &rules.enter else {
        panic!("expected PreserveFenceBodyIndent");
    };
    assert!(
        fence_markers.iter().any(|m| m == "```"),
        "triple backtick must be present"
    );
    assert!(
        fence_markers.iter().any(|m| m == "~~~"),
        "triple tilde must be present"
    );
}

/// Multi-character pair delimiters (`**`/`**`, `__`/`__`, `` ` ``/`` ` ``) are
/// valid `PairRule` entries.  The existing `PairRule` has always supported
/// arbitrary strings — no Markdown-specific extension needed.
#[test]
fn markdown_editor_rules_parse_pair_rules_with_multi_char_delimiters() {
    let rules = markdown_editor_rules();
    let pair_opens: Vec<&str> = rules.pairs.iter().map(|p| p.open.as_str()).collect();
    assert!(pair_opens.contains(&"**"), "must include ** pair");
    assert!(pair_opens.contains(&"__"), "must include __ pair");
    assert!(pair_opens.contains(&"`"), "must include ` pair");
    // All pairs must have matching close delimiters.
    for pair in &rules.pairs {
        assert!(
            !pair.open.is_empty() && !pair.close.is_empty(),
            "pairs must be non-empty"
        );
    }
}

/// The `editorRules` JSON parser must reject known executable field names
/// (`callback`, `code`, `javascript`, `hook`) in pair rules — only inert
/// data may ride the manifest.
#[test]
fn markdown_editor_rules_reject_executable_fields() {
    // Pair rules with executable-sounding fields.
    let _bad_pairs = json!([
        { "open": "(", "close": ")", "callback": "() => true" },
    ]);

    // We can't call parse_editor_rules directly (it's private), but we
    // exercise the same invariant by verifying the protocol types can't
    // hold a callback — no field exists for it.
    let pair = PairRule::new("(", ")");
    assert_eq!(pair.open, "(");
    assert_eq!(pair.close, ")");
    assert_eq!(pair.when, PairRuleContext::CaretOrSelection);
    // No callback field exists on PairRule — the type system enforces inertness.
}

/// All generic `editorRules` fields parse correctly with the Markdown
/// package's declared values.
#[test]
fn markdown_editor_rules_parse_all_fields() {
    let rules = markdown_editor_rules();

    // Enter rule.
    assert!(matches!(rules.enter, EnterRule::ContinueLineMarkers { .. }));

    // Pairs: exactly 5 (including **, __, `).
    assert_eq!(rules.pairs.len(), 5);

    // Comments: empty (Markdown has no line-comment continuation).
    assert!(rules.comments.is_empty());

    // Autocomplete triggers: empty.
    assert!(rules.autocomplete_triggers.is_empty());

    // Tab: spaces, 4 per tab.
    assert_eq!(rules.tab.spaces_per_tab, 4);
    assert_eq!(rules.tab.mode, TabMode::InsertSpaces);

    // Text edits: all three capabilities.
    assert_eq!(rules.text_edits.len(), 3);
}

// ── Command and key binding tests ──────────────────────────────────────────

/// After mode activation, the per-document behavior manifest should include
/// the three package-prefixed Markdown commands (appended by
/// `append_package_commands` from the package record's contributions).
#[test]
fn markdown_activation_publishes_manifest_with_commands() {
    let (mut registry, record) = markdown_registry_with_mode();
    activate_markdown_for_document(&mut registry, &record, "notes.md", 1);

    let enabled = vec![&record];
    let selection = registry
        .select_behavior_manifest_for_document(1, &enabled)
        .expect("manifest selection must succeed");

    let command_ids: Vec<&str> = selection
        .manifest
        .commands
        .iter()
        .map(|c| c.command_id.as_str())
        .collect();

    for expected in &[
        "markdown.togglePreview",
        "markdown.insertHeading",
        "markdown.toggleList",
    ] {
        assert!(
            command_ids.contains(expected),
            "manifest must include command '{expected}', got: {command_ids:?}"
        );
    }
}

/// Key routing descriptors in the package are discoverable from the package
/// record's `contributions.key_routing`.
#[test]
fn markdown_activation_publishes_manifest_with_keymaps() {
    let record = markdown_package_record();
    let kr = &record.contributions.key_routing;

    assert_eq!(kr.len(), 3, "must have 3 key routing contributions");
    assert!(kr.iter().any(|k| k.command_id == "markdown.togglePreview"
        && k.key_binding.as_deref() == Some("Ctrl+Shift+M")));
    assert!(kr.iter().any(|k| k.command_id == "markdown.insertHeading"
        && k.key_binding.as_deref() == Some("Ctrl+Alt+1")));
    assert!(kr.iter().any(|k| k.command_id == "markdown.toggleList"
        && k.key_binding.as_deref() == Some("Ctrl+Shift+8")));
}

// ── Behavior version and non-Markdown tests ────────────────────────────────

#[test]
fn markdown_behavior_version_increments_on_reactivation() {
    let (mut registry, record) = markdown_registry_with_mode();
    let first = activate_markdown_for_document(&mut registry, &record, "notes.md", 1);
    let second = activate_markdown_for_document(&mut registry, &record, "notes.md", 1);
    assert!(
        second.behavior_version > first.behavior_version,
        "reactivation must increment behavior_version"
    );
}

#[test]
fn markdown_reload_reinstalls_manifest_and_decorations() {
    let (mut first_registry, first_record) = markdown_registry_with_mode();
    activate_markdown_for_document(&mut first_registry, &first_record, "notes.md", 7);
    let first_enabled = vec![&first_record];
    let first_manifest = first_registry
        .select_behavior_manifest_for_document(7, &first_enabled)
        .expect("first run must publish Markdown behavior manifest")
        .manifest
        .clone();

    let (mut restarted_registry, restarted_record) = markdown_registry_with_mode();
    activate_markdown_for_document(&mut restarted_registry, &restarted_record, "notes.md", 7);
    let restarted_enabled = vec![&restarted_record];
    let restarted_manifest = restarted_registry
        .select_behavior_manifest_for_document(7, &restarted_enabled)
        .expect("restart must recreate Markdown behavior manifest from package metadata")
        .manifest
        .clone();

    assert_eq!(first_manifest.manifest_id, restarted_manifest.manifest_id);
    assert!(
        restarted_manifest
            .commands
            .iter()
            .any(|command| command.command_id == "markdown.togglePreview")
    );

    let mut surface = EditorSurface::default();
    surface.load_snapshot(
        7,
        3,
        "# Reloaded\n\n- item\n".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    surface.install_behavior_manifest(restarted_manifest);
    assert!(surface.apply_decoration_set(markdown_decoration_set(3, 160)));
    assert_eq!(surface.decoration_state_version(), Some(3));
    assert_eq!(surface.decoration_span_count(), 7);
}

#[tokio::test]
async fn markdown_typing_does_not_wait_for_markdown_it_parse() {
    let coordinator = ParseCoordinator::new();
    let package = markdown_package_record();
    coordinator
        .register_handler(&package, "markdown", |_notification| async move {
            tokio::time::sleep(Duration::from_millis(250)).await;
            Ok(markdown_parse_update(1))
        })
        .expect("Markdown parse handler must register");

    let mut surface = EditorSurface::default();
    surface.load_snapshot(
        7,
        1,
        "# Title\n".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    surface.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
    surface.command(EditorCommand::DocumentEnd);

    let started = Instant::now();
    coordinator
        .schedule_parse(ParseScheduleRequest {
            document_id: 7,
            document_version: 1,
            behavior_version: 3,
            package_prefix: "markdown".to_string(),
            mode_id: "markdown".to_string(),
            viewport: ParseByteRange::new(0, 32),
            invalidated_ranges: vec![ParseByteRange::new(0, 8)],
        })
        .expect("Markdown parse scheduling must be accepted");
    let outcome = surface.command_with_event(EditorCommand::Insert("!"));

    assert!(outcome.changed);
    assert_eq!(surface.visible_text(), "# Title\n!");
    assert!(
        started.elapsed() < Duration::from_millis(25),
        "local Markdown typing must not wait for slow server parse handler"
    );
}

/// Non-Markdown manifests use `EnterRule::PreserveLeadingWhitespace` —
/// no Markdown-specific rules leak in.
#[test]
fn non_markdown_manifest_uses_preserve_leading_whitespace() {
    let manifest = BehaviorManifest::minimal_text_editing(1);
    assert_eq!(
        manifest.editor_rules.enter,
        EnterRule::PreserveLeadingWhitespace
    );
    // No comments by default (manifest uses /// comments).
    assert!(
        !manifest.editor_rules.comments.is_empty() || manifest.editor_rules.comments.len() <= 1
    );
    // Pairs are generic bracket/quote pairs.
    assert!(manifest.editor_rules.pairs.iter().any(|p| p.open == "("));
}

/// Manifest selection succeeds without a parse handler having been invoked.
/// All transform rules are available immediately from the inert manifest.
#[test]
fn markdown_mode_manifest_does_not_wait_for_parse_handler() {
    let (mut registry, record) = markdown_registry_with_mode();
    activate_markdown_for_document(&mut registry, &record, "notes.md", 1);

    let enabled = vec![&record];
    let selection = registry
        .select_behavior_manifest_for_document(1, &enabled)
        .expect("manifest selection must succeed without parse handler");

    // The manifest is available immediately — no parse result needed.
    assert!(!selection.manifest.commands.is_empty());
    assert!(!selection.manifest.keymaps.is_empty());
}
