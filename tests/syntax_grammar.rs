use clay::editor::EditorSurface;
use clay::packages::{
    modes::{DocumentClassificationInput, MajorModeActivation, ModePatternKind},
    record::{PackageRecord, PackageRecordRule, assemble_package_record},
};
use clay::protocol::{
    DecorationSet, DecorationSpan, DocumentAccess, DocumentFontRole, Modifiers, ParseByteRange,
    ParseEditNotification, ParseInputEdit, ParsePoint, ParsePolicy, ParseWindowSnapshot, TokenType,
};
#[cfg(any(unix, windows))]
use clay::server::{
    parse_coordinator::{ParseCoordinator, ParseScheduleRequest},
    syntax::{
        SyntaxCapture, SyntaxEngineTier, SyntaxGrammarContribution, SyntaxGrammarPatternKind,
        SyntaxGrammarRegistry, SyntaxGrammarRegistryError, TreeSitterSyntaxError,
        TreeSitterSyntaxHandler, WebTreeSitterArtifactError, map_capture_to_vocabulary,
    },
};
use serde_json::{Value, json};

trait DecorationUpdatesExt {
    fn expect(self, message: &str) -> DecorationSet;
    fn unwrap_or_else(self, fallback: impl FnOnce() -> DecorationSet) -> DecorationSet;
    fn is_some(&self) -> bool;
}

impl DecorationUpdatesExt for Vec<DecorationSet> {
    fn expect(self, message: &str) -> DecorationSet {
        self.unwrap_or_else(|| panic!("{message}"))
    }

    fn unwrap_or_else(mut self, fallback: impl FnOnce() -> DecorationSet) -> DecorationSet {
        if self.is_empty() {
            return fallback();
        }
        let mut merged = self.remove(0);
        for set in self {
            merged.viewport_byte_start = merged.viewport_byte_start.min(set.viewport_byte_start);
            merged.viewport_byte_end = merged.viewport_byte_end.max(set.viewport_byte_end);
            merged.spans.extend(set.spans);
        }
        merged
    }

    fn is_some(&self) -> bool {
        !self.is_empty()
    }
}

fn grammar_package(prefix: &str, language_id: &str, extension: &str) -> Value {
    json!({
        "name": format!("@clay/{prefix}"),
        "version": "0.1.0",
        "type": "module",
        "clay": {
            "apiPrefix": prefix,
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "permissions": ["parse-document", "render-decorations"],
            "modes": [],
            "docs": "./docs/index.md",
            "apiDependencies": ["clay.syntax.serverRegisterSyntaxGrammar"],
            "contributions": {
                "syntaxGrammars": [{
                    "languageId": language_id,
                    "filePatterns": { "extensions": [extension] },
                    "grammar": {
                        "kind": "tree-sitter-wasm",
                        "path": format!("./grammars/{language_id}.wasm"),
                        "source": format!("tree-sitter-{language_id}")
                    },
                    "queries": {
                        "highlights": "./queries/highlights.scm",
                        "locals": "./queries/locals.scm",
                        "injections": "./queries/injections.scm"
                    },
                    "styleMap": {
                        "keyword": { "type": "Keyword" },
                        "string": { "type": "String" },
                        "comment": { "type": "Comment" },
                        "punctuation": { "type": "Operator" }
                    },
                    "budgets": { "timeoutMs": 5000, "maxWindowBytes": 4096 }
                }]
            }
        }
    })
}

#[cfg(any(unix, windows))]
fn rust_record() -> PackageRecord {
    assemble_package_record(&grammar_package("rust", "rust", "rs"))
        .expect("valid syntax grammar package")
}

#[cfg(any(unix, windows))]
fn rust_contribution(record: &PackageRecord) -> SyntaxGrammarContribution {
    let mut registry = SyntaxGrammarRegistry::new();
    registry
        .register_package(record)
        .expect("grammar registers");
    registry
        .get("rust.rust")
        .expect("registered grammar")
        .clone()
}

#[cfg(any(unix, windows))]
fn rust_language() -> tree_sitter::Language {
    tree_sitter_rust::LANGUAGE.into()
}

#[cfg(any(unix, windows))]
fn rust_highlights_query() -> &'static str {
    r#"
      "fn" @keyword
      "{" @punctuation
      (string_literal) @string
      (line_comment) @comment
    "#
}

#[cfg(any(unix, windows))]
#[test]
fn native_tree_sitter_crates_are_runtime_compatible() {
    let languages: &[(&str, tree_sitter::Language)] = &[
        ("rust", tree_sitter_rust::LANGUAGE.into()),
        (
            "typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        ),
        ("tsx", tree_sitter_typescript::LANGUAGE_TSX.into()),
        ("javascript", tree_sitter_javascript::LANGUAGE.into()),
        ("markdown", tree_sitter_md_025::LANGUAGE.into()),
        (
            "markdown-inline",
            tree_sitter_md_025::INLINE_LANGUAGE.into(),
        ),
    ];

    for (name, language) in languages {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(language).unwrap_or_else(|err| {
            panic!("{name} grammar is incompatible with host tree-sitter: {err}")
        });
    }
}

#[cfg(any(unix, windows))]
#[cfg(any(unix, windows))]
#[test]
fn tier1_native_first_party_is_default_for_known_extensions() {
    let mut registry = SyntaxGrammarRegistry::with_first_party_native();
    assert_eq!(
        SyntaxGrammarRegistry::first_party_native_descriptors().len(),
        5
    );

    for (path, contribution_id, language_id, package_prefix) in [
        ("src/main.rs", "rust.rust", "rust", "rust"),
        (
            "src/app.ts",
            "typescript.typescript",
            "typescript",
            "typescript",
        ),
        ("src/app.tsx", "typescript.tsx", "tsx", "typescript"),
        (
            "src/app.js",
            "javascript.javascript",
            "javascript",
            "javascript",
        ),
        (
            "src/app.jsx",
            "javascript.javascript",
            "javascript",
            "javascript",
        ),
        (
            "src/app.mjs",
            "javascript.javascript",
            "javascript",
            "javascript",
        ),
        (
            "src/app.cjs",
            "javascript.javascript",
            "javascript",
            "javascript",
        ),
        ("README.md", "markdown.markdown", "markdown", "markdown"),
        (
            "README.markdown",
            "markdown.markdown",
            "markdown",
            "markdown",
        ),
    ] {
        let activation = core_code_activation(91, 12);
        let selection =
            registry.select_for_document(&classification_input(91, Some(path)), &activation, 3);
        let grammar = selection
            .active_syntax_grammar
            .as_ref()
            .unwrap_or_else(|| panic!("{path} should select a native grammar"));
        let contribution = registry
            .get(contribution_id)
            .unwrap_or_else(|| panic!("{contribution_id} registered"));

        assert_eq!(grammar.contribution_id, contribution_id);
        assert_eq!(grammar.language_id, language_id);
        assert_eq!(grammar.package_prefix, package_prefix);
        assert_eq!(contribution.engine_tier, SyntaxEngineTier::Native);
        assert_eq!(contribution.grammar_kind, "native");
        if language_id == "markdown" {
            assert_eq!(
                contribution.style_map["code"].font_role,
                Some(DocumentFontRole::Monospace)
            );
        }
        registry
            .native_language(contribution_id)
            .unwrap_or_else(|| panic!("{contribution_id} native language available"));
    }
}

#[test]
fn first_party_native_registry_falls_back_to_no_grammar_for_unknown_extensions() {
    // All five first-party native grammars are registered, yet a document whose
    // path matches no declared extension must keep `core.code`/`core.text` as
    // the active major mode with no syntax grammar selected. This locks that
    // first-party registration never greedy-matches or breaks fallback.
    let mut registry = SyntaxGrammarRegistry::with_first_party_native();
    assert_eq!(
        SyntaxGrammarRegistry::first_party_native_descriptors().len(),
        5,
        "five first-party native grammars must be registered"
    );

    for (path, fallback_mode) in [
        ("notes.txt", "core.text"),
        ("config.json", "core.code"),
        ("data.xyz", "core.code"),
        ("README", "core.code"),
    ] {
        let activation = if fallback_mode == "core.text" {
            core_text_activation(5, 7)
        } else {
            core_code_activation(5, 7)
        };
        let selection =
            registry.select_for_document(&classification_input(5, Some(path)), &activation, 1);

        assert_eq!(
            selection.active_major_mode, fallback_mode,
            "{path}: unknown extension must keep the active major mode"
        );
        assert_eq!(selection.behavior_version, 7);
        assert!(
            selection.active_syntax_grammar.is_none(),
            "{path}: no first-party grammar may match an unknown extension"
        );
        assert!(
            selection.why.contains("document remains editable"),
            "{path}: selection rationale must explain the no-grammar fallback"
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn forced_wasm_without_wasm_artifact_does_not_masquerade_native_metadata() {
    let (_, record) = first_party_grammar_package_record("rust");
    let mut registry = SyntaxGrammarRegistry::with_first_party_native();
    registry
        .set_engine_preference("rust", SyntaxEngineTier::Wasm)
        .expect("valid preference");

    assert_eq!(registry.register_package(&record), Ok(0));

    let selection = registry.select_for_document(
        &classification_input(17, Some("src/main.rs")),
        &core_code_activation(17, 2),
        3,
    );
    assert!(selection.active_syntax_grammar.is_none());
    assert!(selection.why.contains("document remains editable"));
}

#[cfg(any(unix, windows))]
#[test]
fn js_parser_fallback_still_runs_without_tree_sitter_grammar() {
    let mut registry = SyntaxGrammarRegistry::with_first_party_native();
    registry
        .set_engine_preference("rust", SyntaxEngineTier::JavaScriptFallback)
        .expect("valid preference");

    let selection = registry.select_for_document(
        &classification_input(18, Some("src/main.rs")),
        &MajorModeActivation {
            document_id: 18,
            package_name: "@clay/rust".to_string(),
            package_version: "0.1.0".to_string(),
            api_prefix: "rust".to_string(),
            mode_id: "rust".to_string(),
            behavior_version: 9,
            document_font_role: clay::protocol::DocumentFontRole::Proportional,
            matched_by: ModePatternKind::Extension,
        },
        4,
    );

    assert_eq!(selection.active_major_mode, "rust");
    assert_eq!(selection.behavior_version, 9);
    assert!(selection.active_syntax_grammar.is_none());
    assert!(selection.why.contains("document remains editable"));
}

#[cfg(any(unix, windows))]
#[test]
fn package_cannot_silently_override_native_tier() {
    let (_, record) = first_party_grammar_package_record("rust");
    let mut registry = SyntaxGrammarRegistry::with_first_party_native();

    assert_eq!(registry.register_package(&record), Ok(0));

    let grammar = registry
        .find_for_extension("rs")
        .expect("native rust registered");
    assert_eq!(grammar.engine_tier, SyntaxEngineTier::Native);
    assert_eq!(grammar.package_version, "builtin");
}

#[cfg(any(unix, windows))]
#[test]
fn native_registration_shadows_first_party_wasm_package_metadata() {
    let (_, record) = first_party_grammar_package_record("rust");
    let mut registry = SyntaxGrammarRegistry::with_first_party_native();

    assert_eq!(registry.register_package(&record), Ok(0));

    let grammar = registry
        .find_for_extension("rs")
        .expect("native rust registered");
    assert_eq!(grammar.engine_tier, SyntaxEngineTier::Native);
    assert_eq!(grammar.package_name, "@clay/rust");
    assert_eq!(grammar.package_version, "builtin");
}

#[cfg(any(unix, windows))]
#[test]
fn web_tree_sitter_artifact_contract_accepts_package_confined_wasm() {
    let record = assemble_package_record(&grammar_package("rust", "rust", "rs"))
        .expect("valid wasm grammar package");
    let contribution = rust_contribution(&record);

    let contract = contribution
        .web_tree_sitter_artifact_contract()
        .expect("valid Tier 2 artifact contract");

    assert_eq!(contract.contribution_id, "rust.rust");
    assert_eq!(contract.package_name, "@clay/rust");
    assert_eq!(contract.grammar_path, "./grammars/rust.wasm");
    assert_eq!(contract.highlights_query_path, "./queries/highlights.scm");
}

#[cfg(any(unix, windows))]
#[test]
fn tier2_rejects_grammar_path_outside_package_root() {
    let registry = SyntaxGrammarRegistry::with_first_party_native();
    let native = registry.get("rust.rust").expect("native rust");
    assert!(matches!(
        native.web_tree_sitter_artifact_contract(),
        Err(WebTreeSitterArtifactError::NotWasmTier { .. })
    ));

    let mut contribution = native.clone();
    contribution.engine_tier = SyntaxEngineTier::Wasm;
    contribution.grammar_kind = "tree-sitter-wasm".to_string();
    contribution.grammar_path = "../grammars/rust.wasm".to_string();
    contribution.highlights_query_path = "./queries/highlights.scm".to_string();
    assert!(matches!(
        contribution.web_tree_sitter_artifact_contract(),
        Err(WebTreeSitterArtifactError::GrammarPathNotConfined { .. })
    ));

    contribution.grammar_path = "./grammars/rust.wasm".to_string();
    contribution.highlights_query_path = "https://example.invalid/highlights.scm".to_string();
    assert!(matches!(
        contribution.web_tree_sitter_artifact_contract(),
        Err(WebTreeSitterArtifactError::QueryPathNotConfined { .. })
    ));
}

#[cfg(any(unix, windows))]
#[test]
fn tier2_wasm_override_suppresses_tier1_when_user_selected() {
    let record = assemble_package_record(&grammar_package("rust", "rust", "rs"))
        .expect("valid synthetic Tier 2 wasm package");
    let mut registry = SyntaxGrammarRegistry::with_first_party_native();

    assert_eq!(
        registry.register_package_with_explicit_tier2_override(&record),
        Ok(1)
    );

    let grammar = registry
        .find_for_extension("rs")
        .expect("explicit Tier 2 rust registered");
    assert_eq!(grammar.engine_tier, SyntaxEngineTier::Wasm);
    assert_eq!(grammar.package_version, "0.1.0");
    assert!(grammar.web_tree_sitter_artifact_contract().is_ok());
    assert!(registry.native_language("rust.rust").is_none());
}

#[test]
fn web_tree_sitter_runtime_is_bundled_and_loadable_without_network() {
    let source = std::fs::read_to_string("runtime/js/web-tree-sitter-host.ts")
        .expect("web-tree-sitter host adapter source readable");

    for required in [
        "initializeWebTreeSitter",
        "Parser.init",
        "Language.load",
        "languageCache",
        "queryCache",
        "./grammars/",
        ".wasm",
        "./queries/",
        ".scm",
        "clay://runtime/tree-sitter.wasm",
        "collectWebTreeSitterDiagnostics",
        "node.isError",
        "node.isMissing",
        "hasError",
    ] {
        assert!(
            source.contains(required),
            "Tier 2 host adapter must contain {required}"
        );
    }
    for forbidden in [
        "fetch(",
        "http://",
        "https://",
        "npm:",
        "child_process",
        "Deno.run",
        "Deno.Command",
    ] {
        assert!(
            !source.contains(forbidden),
            "Tier 2 host adapter must not use network/shell/package manager: {forbidden}"
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn native_parser_instance_is_cached_per_grammar() {
    let registry = SyntaxGrammarRegistry::with_first_party_native();
    let contribution = registry.get("rust.rust").expect("native rust").clone();
    let language = registry
        .native_language("rust.rust")
        .expect("native language");
    let handler = TreeSitterSyntaxHandler::new(contribution, language, rust_highlights_query())
        .expect("handler compiles query once");

    let first_parser = handler.parser_cache_id();
    let second_parser = handler.parser_cache_id();

    assert_eq!(first_parser, second_parser);
}

#[test]
fn tiered_engine_has_no_language_specific_rust_branches() {
    let source = std::fs::read_to_string("src/server/syntax.rs").expect("syntax source readable");

    for forbidden in [
        "if contribution.language_id",
        "if grammar.language_id",
        "match contribution.language_id",
        "match grammar.language_id",
        "RustSyntaxHighlighter",
        "TypeScriptSyntaxHighlighter",
        "JavaScriptSyntaxHighlighter",
        "MarkdownTreeSitterHighlighter",
    ] {
        assert!(
            !source.contains(forbidden),
            "tiered syntax engine must stay data-driven, found {forbidden}"
        );
    }
}

#[cfg(any(unix, windows))]
fn core_activation(
    document_id: u64,
    behavior_version: u64,
    mode_id: &str,
    matched_by: ModePatternKind,
) -> MajorModeActivation {
    MajorModeActivation {
        document_id,
        package_name: "core".to_string(),
        package_version: "builtin".to_string(),
        api_prefix: "core".to_string(),
        mode_id: mode_id.to_string(),
        behavior_version,
        document_font_role: if mode_id == "core.code" {
            DocumentFontRole::Monospace
        } else {
            DocumentFontRole::Proportional
        },
        matched_by,
    }
}

fn core_code_activation(document_id: u64, behavior_version: u64) -> MajorModeActivation {
    core_activation(
        document_id,
        behavior_version,
        "core.code",
        ModePatternKind::Extension,
    )
}

fn core_text_activation(document_id: u64, behavior_version: u64) -> MajorModeActivation {
    core_activation(
        document_id,
        behavior_version,
        "core.text",
        ModePatternKind::Fallback,
    )
}

fn classification_input(document_id: u64, path: Option<&str>) -> DocumentClassificationInput {
    DocumentClassificationInput {
        document_id,
        path: path.map(str::to_string),
        mime_type: None,
        shebang: None,
        leading_content: None,
    }
}

#[cfg(any(unix, windows))]
fn parse_notification_for(prefix: &str, version: u64, text: &str) -> ParseEditNotification {
    ParseEditNotification {
        document_id: 11,
        document_version: version,
        behavior_version: 4,
        package_prefix: prefix.to_string(),
        mode_id: prefix.to_string(),
        viewport: ParseByteRange::new(0, text.len() as u64),
        invalidated_ranges: vec![ParseByteRange::new(0, text.len() as u64)],
        accepted_edit: None,
        parse_windows: vec![ParseWindowSnapshot {
            document_id: 11,
            document_version: version,
            package_prefix: prefix.to_string(),
            mode_id: prefix.to_string(),
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

#[cfg(any(unix, windows))]
fn parse_notification(version: u64, text: &str) -> ParseEditNotification {
    parse_notification_for("rust", version, text)
}

#[cfg(any(unix, windows))]
fn point_at(text: &str, offset: usize) -> ParsePoint {
    let prefix = &text[..offset];
    ParsePoint::new(
        prefix.bytes().filter(|byte| *byte == b'\n').count() as u64,
        prefix
            .rsplit_once('\n')
            .map_or(prefix.len(), |(_, tail)| tail.len()) as u64,
    )
}

#[cfg(any(unix, windows))]
fn incremental_notification(
    old_text: &str,
    new_text: &str,
    start: usize,
    old_end: usize,
    new_end: usize,
) -> ParseEditNotification {
    incremental_notification_for("rust", old_text, new_text, start, old_end, new_end)
}

#[cfg(any(unix, windows))]
fn incremental_notification_for(
    package_prefix: &str,
    old_text: &str,
    new_text: &str,
    start: usize,
    old_end: usize,
    new_end: usize,
) -> ParseEditNotification {
    incremental_notification_for_versions(
        package_prefix,
        1,
        old_text,
        new_text,
        start,
        old_end,
        new_end,
    )
}

#[cfg(any(unix, windows))]
fn incremental_notification_for_versions(
    package_prefix: &str,
    base_version: u64,
    old_text: &str,
    new_text: &str,
    start: usize,
    old_end: usize,
    new_end: usize,
) -> ParseEditNotification {
    let document_version = base_version + 1;
    let mut notification = parse_notification_for(package_prefix, document_version, new_text);
    notification.invalidated_ranges = vec![ParseByteRange::new(start as u64, new_end as u64)];
    notification.accepted_edit = Some(ParseInputEdit {
        base_document_version: base_version,
        document_version,
        start_byte: start as u64,
        old_end_byte: old_end as u64,
        new_end_byte: new_end as u64,
        start_position: point_at(old_text, start),
        old_end_position: point_at(old_text, old_end),
        new_end_position: point_at(new_text, new_end),
    });
    notification.parse_windows[0].incremental_edit = true;
    notification
}

#[cfg(any(unix, windows))]
fn single_edit_offsets(old_text: &str, new_text: &str) -> (usize, usize, usize) {
    let start = old_text
        .bytes()
        .zip(new_text.bytes())
        .position(|(old, new)| old != new)
        .unwrap_or_else(|| old_text.len().min(new_text.len()));
    let mut old_end = old_text.len();
    let mut new_end = new_text.len();
    while old_end > start
        && new_end > start
        && old_text.as_bytes()[old_end - 1] == new_text.as_bytes()[new_end - 1]
    {
        old_end -= 1;
        new_end -= 1;
    }
    assert!(old_text.is_char_boundary(start) && old_text.is_char_boundary(old_end));
    assert!(new_text.is_char_boundary(start) && new_text.is_char_boundary(new_end));
    (start, old_end, new_end)
}

#[cfg(any(unix, windows))]
fn first_party_handler(
    contribution_id: &str,
    language: tree_sitter::Language,
    query: &str,
) -> TreeSitterSyntaxHandler {
    let registry = SyntaxGrammarRegistry::with_first_party_native();
    let contribution = registry
        .get(contribution_id)
        .unwrap_or_else(|| panic!("first-party {contribution_id} contribution"))
        .clone();
    TreeSitterSyntaxHandler::new(contribution, language, query)
        .unwrap_or_else(|error| panic!("first-party {contribution_id} query compiles: {error}"))
}

#[cfg(any(unix, windows))]
fn first_party_rust_handler() -> TreeSitterSyntaxHandler {
    first_party_handler(
        "rust.rust",
        tree_sitter_rust::LANGUAGE.into(),
        include_str!("../packages/rust/queries/highlights.scm"),
    )
}

#[cfg(any(unix, windows))]
fn apply_initial_syntax_for(
    handler: &TreeSitterSyntaxHandler,
    package_prefix: &str,
    language_id: &str,
    text: &str,
) -> EditorSurface {
    let update = handler
        .parse_sync(parse_notification_for(package_prefix, 1, text))
        .expect("initial syntax parse");
    assert_eq!(
        update.syntax_tree_delta.as_deref(),
        Some(format!("tree-sitter:{language_id}:full").as_str())
    );
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        11,
        1,
        text.to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    for set in update.decoration_updates {
        assert!(editor.apply_decoration_set(set));
    }
    editor
}

#[cfg(any(unix, windows))]
fn apply_initial_syntax(handler: &TreeSitterSyntaxHandler, text: &str) -> EditorSurface {
    apply_initial_syntax_for(handler, "rust", "rust", text)
}

#[cfg(any(unix, windows))]
fn record_undecorated_ranges(
    failures: &mut Vec<String>,
    stage: &str,
    editor: &EditorSurface,
    expected: &[(&str, std::ops::Range<usize>)],
) {
    let painted = editor.visible_decoration_paint_ranges_for_test();
    for (label, expected) in expected {
        if expected
            .clone()
            .any(|byte| !painted.iter().any(|(range, _)| range.contains(&byte)))
        {
            failures.push(format!("{stage}: {label} {expected:?} was base-colored"));
        }
    }
}

#[cfg(any(unix, windows))]
#[derive(Clone, Copy)]
enum ContinuityAction {
    Insert(&'static str),
    Newline,
    Backspace,
}

#[cfg(any(unix, windows))]
struct ContinuityCase {
    label: &'static str,
    package_prefix: &'static str,
    contribution_id: &'static str,
    language_id: &'static str,
    language: fn() -> tree_sitter::Language,
    query: &'static str,
    old_text: &'static str,
    new_text: &'static str,
    caret: usize,
    action: ContinuityAction,
    continuous: &'static [(&'static str, &'static str)],
    authoritative: &'static [(&'static str, &'static str)],
}

#[cfg(any(unix, windows))]
fn expected_ranges(
    text: &str,
    needles: &'static [(&'static str, &'static str)],
) -> Vec<(&'static str, std::ops::Range<usize>)> {
    needles
        .iter()
        .map(|&(label, needle)| {
            let start = text
                .find(needle)
                .unwrap_or_else(|| panic!("{label}: missing {needle:?} in {text:?}"));
            (label, start..start + needle.len())
        })
        .collect()
}

#[cfg(any(unix, windows))]
fn run_continuity_case(case: ContinuityCase) {
    let handler = first_party_handler(case.contribution_id, (case.language)(), case.query);
    let mut editor = apply_initial_syntax_for(
        &handler,
        case.package_prefix,
        case.language_id,
        case.old_text,
    );
    assert!(
        editor.navigate_to_byte_offset(case.caret as u64),
        "{}: caret",
        case.label
    );
    assert!(
        match case.action {
            ContinuityAction::Insert(text) => editor.insert_text(text),
            ContinuityAction::Newline => editor.insert_newline(),
            ContinuityAction::Backspace => editor.backspace(),
        },
        "{}: local edit",
        case.label
    );
    assert_eq!(editor.visible_text(), case.new_text, "{}: text", case.label);

    let continuous = expected_ranges(case.new_text, case.continuous);
    let mut failures = Vec::new();
    record_undecorated_ranges(
        &mut failures,
        &format!("{} optimistic edit", case.label),
        &editor,
        &continuous,
    );
    assert!(editor.note_confirmed_version(11, 2), "{}: ack", case.label);
    record_undecorated_ranges(
        &mut failures,
        &format!("{} acknowledgement", case.label),
        &editor,
        &continuous,
    );

    let (start, old_end, new_end) = single_edit_offsets(case.old_text, case.new_text);
    let update = handler
        .parse_sync(incremental_notification_for(
            case.package_prefix,
            case.old_text,
            case.new_text,
            start,
            old_end,
            new_end,
        ))
        .unwrap_or_else(|error| panic!("{}: incremental parse: {error}", case.label));
    assert_eq!(
        update.syntax_tree_delta.as_deref(),
        Some(format!("tree-sitter:{}:incremental", case.language_id).as_str()),
        "{}: cached tree",
        case.label
    );
    assert!(
        !update.decoration_updates.is_empty(),
        "{}: authoritative members",
        case.label
    );
    for (index, set) in update.decoration_updates.into_iter().enumerate() {
        assert!(
            editor.apply_decoration_set(set),
            "{}: authoritative member {index}",
            case.label
        );
        record_undecorated_ranges(
            &mut failures,
            &format!("{} authoritative member {index}", case.label),
            &editor,
            &continuous,
        );
    }
    let authoritative = expected_ranges(case.new_text, case.authoritative);
    record_undecorated_ranges(
        &mut failures,
        &format!("{} final authoritative state", case.label),
        &editor,
        &authoritative,
    );

    assert!(
        failures.is_empty(),
        "decoration continuity regressed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn syntax_grammar_contribution_validates_with_provenance_and_budgets() {
    let record = assemble_package_record(&grammar_package("rust", "rust", "rs"))
        .expect("valid syntax grammar package");

    let grammar = &record.contributions.syntax_grammars[0];
    assert_eq!(grammar.id, "rust.rust");
    assert_eq!(grammar.language_id, "rust");
    assert_eq!(grammar.extensions, vec!["rs"]);
    assert_eq!(grammar.grammar_kind, "tree-sitter-wasm");
    assert_eq!(grammar.grammar_path, "./grammars/rust.wasm");
    assert_eq!(grammar.highlights_query_path, "./queries/highlights.scm");
    assert_eq!(grammar.style_map["keyword"].token_type, TokenType::Keyword);
    assert_eq!(grammar.timeout_ms, Some(5000));
    assert_eq!(grammar.max_window_bytes, Some(4096));
    assert!(grammar.estimated_payload_bytes > 0);
}

#[test]
fn syntax_style_map_accepts_document_font_roles_and_rejects_concrete_typography() {
    let mut package = grammar_package("markdown", "markdown", "md");
    package["clay"]["contributions"]["syntaxGrammars"][0]["styleMap"]["code"] = json!({
        "type": "CodeSpan",
        "fontRole": "monospace"
    });

    let record = assemble_package_record(&package).expect("semantic role metadata validates");
    assert_eq!(
        record.contributions.syntax_grammars[0].style_map["code"].font_role,
        Some(DocumentFontRole::Monospace)
    );

    package["clay"]["contributions"]["syntaxGrammars"][0]["styleMap"]["code"] = json!({
        "type": "CodeSpan",
        "fontFamily": "JetBrains Mono"
    });
    assert_eq!(
        assemble_package_record(&package).unwrap_err().rule,
        PackageRecordRule::InvalidContributionDescriptor
    );
}

#[test]
fn syntax_style_map_accepts_bounded_priority_and_rejects_invalid_values() {
    let mut package = grammar_package("markdown", "markdown", "md");
    package["clay"]["contributions"]["syntaxGrammars"][0]["styleMap"]["code"] = json!({
        "type": "CodeSpan",
        "priority": 80
    });

    let record = assemble_package_record(&package).expect("bounded priority validates");
    assert_eq!(
        record.contributions.syntax_grammars[0].style_map["code"].priority,
        80
    );
    // Omitted priority keeps the historical default.
    let keyword = record.contributions.syntax_grammars[0]
        .style_map
        .values()
        .find(|entry| entry.priority != 80);
    assert!(keyword.is_some(), "default entries keep priority 70");
    assert!(
        record.contributions.syntax_grammars[0]
            .style_map
            .values()
            .filter(|entry| entry.priority != 80)
            .all(|entry| entry.priority == 70)
    );

    for bad in [json!(101), json!(-1), json!(1.5), json!("80")] {
        package["clay"]["contributions"]["syntaxGrammars"][0]["styleMap"]["code"] = json!({
            "type": "CodeSpan",
            "priority": bad
        });
        assert_eq!(
            assemble_package_record(&package).unwrap_err().rule,
            PackageRecordRule::InvalidContributionDescriptor,
            "priority {bad} must be rejected"
        );
    }
}

#[test]
fn syntax_grammar_requires_parse_and_decoration_permissions() {
    let mut package = grammar_package("rust", "rust", "rs");
    package["clay"]["permissions"] = json!(["parse-document"]);

    let error = assemble_package_record(&package).unwrap_err();

    assert_eq!(
        error.rule,
        PackageRecordRule::UndeclaredPermissionForContribution
    );
    assert!(error.message.contains("render-decorations"));
}

#[test]
fn syntax_grammar_rejects_third_party_packages_in_phase18_10() {
    let mut package = grammar_package("rust", "rust", "rs");
    package["name"] = json!("@vendor/rust");

    let error = assemble_package_record(&package).unwrap_err();

    assert_eq!(error.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(error.message.contains("first-party-only"));
    assert!(error.message.contains("third-party grammar packages"));
}

#[test]
fn syntax_grammar_rejects_unknown_query_kind_deny_by_default() {
    // Plan 071 task 15: text-object queries are first-party native-descriptor
    // contributions only; package grammar metadata must not declare them, and
    // no unknown query key may slip through silently.
    for key in ["textobjects", "localsX", "folds"] {
        let mut package = grammar_package("rust", "rust", "rs");
        package["clay"]["contributions"]["syntaxGrammars"][0]["queries"][key] =
            json!("./queries/textobjects.scm");

        let error = assemble_package_record(&package).unwrap_err();

        assert_eq!(error.rule, PackageRecordRule::InvalidContributionDescriptor);
        assert!(
            error.message.contains(&format!("queries.{key}")),
            "rejection must name the offending key: {}",
            error.message
        );
    }
}

#[test]
fn syntax_grammar_rejects_external_or_traversing_paths() {
    for path in [
        "../grammars/rust.wasm",
        "/tmp/rust.wasm",
        "https://example.invalid/rust.wasm",
        "./grammars/../rust.wasm",
    ] {
        let mut package = grammar_package("rust", "rust", "rs");
        package["clay"]["contributions"]["syntaxGrammars"][0]["grammar"]["path"] = json!(path);

        let error = assemble_package_record(&package).unwrap_err();

        assert!(matches!(
            error.rule,
            PackageRecordRule::InvalidContributionDescriptor
                | PackageRecordRule::ManifestValidationFailed
        ));
        assert!(
            error.message.contains("package-root-confined")
                || error.message.contains("syntax grammar metadata")
                || error.message.contains("relative ./ module path"),
            "got: {}",
            error.message
        );
    }
}

#[test]
fn syntax_grammar_rejects_native_or_executable_authority_fields() {
    for (field, value) in [
        ("nativeLibrary", json!("./grammars/rust.dll")),
        ("downloadUrl", json!("https://example.invalid/rust.wasm")),
        ("shellCommand", json!("npm install")),
        ("clientJavaScript", json!("alert(1)")),
        ("css", json!("color: red")),
    ] {
        let mut package = grammar_package("rust", "rust", "rs");
        package["clay"]["contributions"]["syntaxGrammars"][0][field] = value;

        let error = assemble_package_record(&package).unwrap_err();

        assert!(matches!(
            error.rule,
            PackageRecordRule::InvalidContributionDescriptor
                | PackageRecordRule::ManifestValidationFailed
        ));
        assert!(
            error.message.contains("syntax grammar metadata")
                || error.message.contains("client-side JavaScript"),
            "got: {}",
            error.message
        );
    }
}

#[test]
fn syntax_grammar_rejects_legacy_style_tokens_and_raw_css() {
    let mut package = grammar_package("rust", "rust", "rs");
    package["clay"]["contributions"]["syntaxGrammars"][0]["styleMap"]["keyword"] =
        json!("color: red");

    let error = assemble_package_record(&package).unwrap_err();

    assert_eq!(error.rule, PackageRecordRule::InvalidContributionDescriptor);
    assert!(error.message.contains("known Clay style tokens"));
}

#[cfg(any(unix, windows))]
#[test]
fn syntax_grammar_registry_registers_provenance_and_selects_by_pattern() {
    let record = assemble_package_record(&grammar_package("rust", "rust", "rs"))
        .expect("valid syntax grammar package");
    let mut registry = SyntaxGrammarRegistry::new();

    assert_eq!(registry.register_package(&record), Ok(1));

    let grammar = registry
        .find_for_extension("rs")
        .expect("registered by extension");
    assert_eq!(grammar.package_name, "@clay/rust");
    assert_eq!(grammar.package_version, "0.1.0");
    assert_eq!(grammar.package_prefix, "rust");
    assert_eq!(grammar.language_id, "rust");
    assert_eq!(registry.list().count(), 1);
}

#[cfg(any(unix, windows))]
#[test]
fn syntax_provider_selection_is_separate_from_active_major_mode() {
    let record = assemble_package_record(&grammar_package("rust", "rust", "rs"))
        .expect("valid rust grammar");
    let mut registry = SyntaxGrammarRegistry::new();
    registry
        .register_package(&record)
        .expect("grammar registers");
    let activation = core_code_activation(11, 42);

    let selection = registry.select_for_document(
        &classification_input(11, Some("src/main.rs")),
        &activation,
        7,
    );
    let grammar = selection
        .active_syntax_grammar
        .as_ref()
        .expect("rust grammar selected");

    assert_eq!(selection.active_major_mode, "core.code");
    assert_eq!(selection.behavior_version, 42);
    assert_eq!(grammar.language_id, "rust");
    assert_eq!(grammar.package_prefix, "rust");
    assert_eq!(grammar.matched_by, SyntaxGrammarPatternKind::Extension);
    assert_eq!(registry.active_selection(11), Some(&selection));
}

#[cfg(any(unix, windows))]
#[test]
fn syntax_provider_selection_can_attach_to_core_text_by_file_name() {
    let mut package = grammar_package("make", "make", "mk");
    package["clay"]["contributions"]["syntaxGrammars"][0]["filePatterns"] =
        json!({ "fileNames": ["Makefile"] });
    let record = assemble_package_record(&package).expect("valid make grammar");
    let mut registry = SyntaxGrammarRegistry::new();
    registry
        .register_package(&record)
        .expect("grammar registers");
    let activation = core_text_activation(12, 43);

    let selection =
        registry.select_for_document(&classification_input(12, Some("Makefile")), &activation, 3);
    let grammar = selection
        .active_syntax_grammar
        .as_ref()
        .expect("filename grammar selected");

    assert_eq!(selection.active_major_mode, "core.text");
    assert_eq!(selection.behavior_version, 43);
    assert_eq!(grammar.language_id, "make");
    assert_eq!(grammar.matched_by, SyntaxGrammarPatternKind::FileName);
}

#[cfg(any(unix, windows))]
#[test]
fn syntax_provider_selection_falls_back_to_no_highlighting_without_changing_mode() {
    let mut registry = SyntaxGrammarRegistry::new();
    let activation = core_code_activation(12, 43);

    let selection = registry.select_for_document(
        &classification_input(12, Some("src/main.rs")),
        &activation,
        3,
    );

    assert_eq!(selection.active_major_mode, "core.code");
    assert_eq!(selection.behavior_version, 43);
    assert!(selection.active_syntax_grammar.is_none());
    assert!(selection.why.contains("document remains editable"));
}

#[cfg(any(unix, windows))]
#[test]
fn syntax_provider_selection_cannot_override_mode_or_behavior_version() {
    let record = assemble_package_record(&grammar_package("rust", "rust", "rs"))
        .expect("valid rust grammar");
    let mut registry = SyntaxGrammarRegistry::new();
    registry
        .register_package(&record)
        .expect("grammar registers");
    let activation = core_code_activation(13, 99);

    let selection =
        registry.select_for_document(&classification_input(13, Some("lib.rs")), &activation, 8);

    assert_eq!(selection.document_id, 13);
    assert_eq!(selection.document_version, 8);
    assert_eq!(selection.active_major_mode, activation.mode_id);
    assert_eq!(selection.behavior_version, activation.behavior_version);
    assert!(selection.active_syntax_grammar.is_some());
}

#[cfg(any(unix, windows))]
#[test]
fn syntax_grammar_registry_rejects_duplicate_language_or_pattern_deterministically() {
    let rust = assemble_package_record(&grammar_package("rust", "rust", "rs"))
        .expect("valid rust grammar");
    let rust_again = assemble_package_record(&grammar_package("rust2", "rust", "rust"))
        .expect("valid duplicate language grammar");
    let other_rs = assemble_package_record(&grammar_package("other", "other", "rs"))
        .expect("valid duplicate extension grammar");
    let mut registry = SyntaxGrammarRegistry::new();

    assert_eq!(registry.register_package(&rust), Ok(1));
    assert!(matches!(
        registry.register_package(&rust_again),
        Err(SyntaxGrammarRegistryError::DuplicateLanguage { language_id, existing_package_prefix, new_package_prefix })
            if language_id == "rust" && existing_package_prefix == "rust" && new_package_prefix == "rust2"
    ));
    assert!(matches!(
        registry.register_package(&other_rs),
        Err(SyntaxGrammarRegistryError::DuplicateExtension { extension, existing_language_id, new_language_id })
            if extension == "rs" && existing_language_id == "rust" && new_language_id == "other"
    ));
}

#[cfg(any(unix, windows))]
#[test]
fn manual_syntax_smoke_contract_is_covered_by_deterministic_fixture_flow() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let mut registry = SyntaxGrammarRegistry::new();
    for package_dir in ["rust", "typescript", "javascript"] {
        let (_, record) = first_party_grammar_package_record(package_dir);
        registry
            .register_package(&record)
            .unwrap_or_else(|error| panic!("register {package_dir}: {error:?}"));
    }

    for (package_dir, contribution_id, language, fixture_path, document_path) in [
        (
            "rust",
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            "tests/fixtures/syntax/rust.rs",
            "smoke/rust.rs",
        ),
        (
            "typescript",
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "tests/fixtures/syntax/typescript.ts",
            "smoke/typescript.ts",
        ),
        (
            "javascript",
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            "tests/fixtures/syntax/javascript.js",
            "smoke/javascript.js",
        ),
    ] {
        let activation = core_code_activation(44, 7);
        let selected = registry.select_for_document(
            &classification_input(44, Some(document_path)),
            &activation,
            1,
        );
        let active = selected
            .active_syntax_grammar
            .as_ref()
            .unwrap_or_else(|| panic!("{package_dir} should select an active syntax grammar"));
        assert_eq!(active.contribution_id, contribution_id);
        assert_eq!(selected.active_major_mode, "core.code");
        assert_eq!(selected.behavior_version, 7);

        let contribution = registry
            .get(contribution_id)
            .unwrap_or_else(|| panic!("registered {contribution_id}"))
            .clone();
        let query = std::fs::read_to_string(format!(
            "{manifest_dir}/packages/{package_dir}/queries/highlights.scm"
        ))
        .unwrap_or_else(|error| panic!("read {package_dir} query: {error}"));
        let text = std::fs::read_to_string(format!("{manifest_dir}/{fixture_path}"))
            .unwrap_or_else(|error| panic!("read {fixture_path}: {error}"));
        let handler = TreeSitterSyntaxHandler::new(contribution, language, &query)
            .unwrap_or_else(|error| panic!("{package_dir} query compiles: {error}"));

        for (version, source) in [(1, text.clone()), (2, format!("{text}\n"))] {
            let update = handler
                .parse_sync(parse_notification_for(package_dir, version, &source))
                .unwrap_or_else(|error| panic!("{package_dir} fixture parses v{version}: {error}"));
            let set = update.decoration_updates.unwrap_or_else(|| {
                panic!("{package_dir} fixture publishes v{version} decorations")
            });
            assert_eq!(set.document_version, version);
            assert!(
                set.spans
                    .iter()
                    .any(|span| span.token_type == TokenType::Keyword),
                "{package_dir} fixture should highlight after v{version} parse"
            );
        }
    }

    let mut unloaded_registry = SyntaxGrammarRegistry::new();
    let fallback = unloaded_registry.select_for_document(
        &classification_input(55, Some("smoke/rust.rs")),
        &core_code_activation(55, 8),
        1,
    );
    assert_eq!(fallback.active_major_mode, "core.code");
    assert!(fallback.active_syntax_grammar.is_none());
    assert!(fallback.why.contains("document remains editable"));
}

#[cfg(any(unix, windows))]
#[test]
fn first_party_syntax_fixtures_produce_bounded_decoration_sets() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    for (package_dir, contribution_id, language, fixture) in [
        (
            "rust",
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            "tests/fixtures/syntax/rust.rs",
        ),
        (
            "typescript",
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "tests/fixtures/syntax/typescript.ts",
        ),
        (
            "javascript",
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            "tests/fixtures/syntax/javascript.js",
        ),
    ] {
        let (_, record) = first_party_grammar_package_record(package_dir);
        let mut registry = SyntaxGrammarRegistry::new();
        registry
            .register_package(&record)
            .expect("first-party grammar registers");
        let contribution = registry
            .get(contribution_id)
            .unwrap_or_else(|| panic!("registered {contribution_id}"))
            .clone();
        let query = std::fs::read_to_string(format!(
            "{manifest_dir}/packages/{package_dir}/queries/highlights.scm"
        ))
        .unwrap_or_else(|error| panic!("read {package_dir} query: {error}"));
        let text = std::fs::read_to_string(format!("{manifest_dir}/{fixture}"))
            .unwrap_or_else(|error| panic!("read {fixture}: {error}"));
        let handler = TreeSitterSyntaxHandler::new(contribution, language, &query)
            .unwrap_or_else(|error| panic!("{package_dir} query compiles: {error}"));

        let update = handler
            .parse_sync(parse_notification_for(package_dir, 1, &text))
            .unwrap_or_else(|error| panic!("{package_dir} fixture parses: {error}"));
        let set = update
            .decoration_updates
            .unwrap_or_else(|| panic!("{package_dir} fixture publishes decorations"));

        assert_eq!(set.document_version, 1);
        assert!(
            set.spans
                .iter()
                .any(|span| span.token_type == TokenType::Keyword),
            "{package_dir} fixture should highlight a keyword"
        );
        assert!(
            set.spans
                .iter()
                .any(|span| span.token_type == TokenType::String),
            "{package_dir} fixture should highlight a string"
        );
        assert!(
            set.spans
                .iter()
                .any(|span| span.token_type == TokenType::Comment),
            "{package_dir} fixture should highlight a comment"
        );
        assert!(
            set.spans
                .iter()
                .all(|span| span.byte_start >= set.viewport_byte_start)
        );
        assert!(
            set.spans
                .iter()
                .all(|span| span.byte_end <= set.viewport_byte_end)
        );
        assert!(
            set.spans
                .iter()
                .all(|span| span.provenance.package_prefix == package_dir)
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn first_party_language_fixtures_produce_themed_vocabulary_decorations() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let registry = SyntaxGrammarRegistry::with_first_party_native();

    for (package_dir, contribution_id, language, fixture, expected_tokens) in [
        (
            "rust",
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            "tests/fixtures/syntax/rust.rs",
            &[TokenType::Keyword, TokenType::String, TokenType::Comment][..],
        ),
        (
            "typescript",
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "tests/fixtures/syntax/typescript.ts",
            &[TokenType::Keyword, TokenType::String, TokenType::Comment][..],
        ),
        (
            "typescript",
            "typescript.tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            "tests/fixtures/syntax/typescript.tsx",
            &[TokenType::Keyword, TokenType::String, TokenType::Comment][..],
        ),
        (
            "javascript",
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            "tests/fixtures/syntax/javascript.js",
            &[TokenType::Keyword, TokenType::String, TokenType::Comment][..],
        ),
        (
            "markdown",
            "markdown.markdown",
            tree_sitter_md_025::LANGUAGE.into(),
            "tests/fixtures/syntax/markdown.md",
            &[TokenType::Paragraph, TokenType::Operator][..],
        ),
    ] {
        let contribution = registry
            .get(contribution_id)
            .unwrap_or_else(|| panic!("registered {contribution_id}"))
            .clone();
        let query = std::fs::read_to_string(format!(
            "{manifest_dir}/packages/{package_dir}/queries/highlights.scm"
        ))
        .unwrap_or_else(|error| panic!("read {package_dir} query: {error}"));
        let text = std::fs::read_to_string(format!("{manifest_dir}/{fixture}"))
            .unwrap_or_else(|error| panic!("read {fixture}: {error}"));
        let handler = TreeSitterSyntaxHandler::new(contribution, language, &query)
            .unwrap_or_else(|error| panic!("{fixture} query compiles: {error}"));

        let set = handler
            .parse_sync(parse_notification_for(package_dir, 1, &text))
            .unwrap_or_else(|error| panic!("{fixture} parses: {error}"))
            .decoration_updates
            .unwrap_or_else(|| panic!("{fixture} publishes decorations"));

        assert_eq!(set.document_version, 1);
        assert!(
            set.spans.len() <= 512,
            "{fixture} decoration count stays bounded"
        );
        for expected in expected_tokens {
            assert!(
                set.spans.iter().any(|span| span.token_type == *expected),
                "{fixture} should emit {expected:?} vocabulary decoration"
            );
        }
        // Phase 18.18: first-party grammar captures are pure two-axis
        // vocabulary (closed token_type + modifiers); the optional `scope`
        // escape stays reserved for third-party package-JS decorations.
        assert!(set.spans.iter().all(|span| span.scope.is_none()));
        assert!(
            set.spans
                .iter()
                .all(|span| span.provenance.package_prefix == package_dir)
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn rust_grammar_emits_vocabulary_tokens_through_stylemap() {
    let registry = SyntaxGrammarRegistry::with_first_party_native();
    let contribution = registry
        .get("rust.rust")
        .expect("native Rust grammar")
        .clone();
    let query = std::fs::read_to_string(format!(
        "{}/packages/rust/queries/highlights.scm",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("read Rust highlights query");
    let handler = TreeSitterSyntaxHandler::new(contribution, rust_language(), &query)
        .expect("Rust query compiles");
    let set = handler
        .parse_sync(parse_notification(
            1,
            "fn main() { let s = \"x\"; // comment\n}",
        ))
        .expect("Rust parses")
        .decoration_updates
        .expect("Rust decorations");

    for token_type in [
        TokenType::Keyword,
        TokenType::String,
        TokenType::Comment,
        TokenType::Function,
    ] {
        assert!(
            set.spans.iter().any(|span| span.token_type == token_type),
            "Rust should emit {token_type:?} through styleMap"
        );
    }
    assert!(set.spans.iter().any(|span| {
        span.token_type == TokenType::Function
            && span.modifiers.contains(Modifiers::DECLARATION)
            && span.scope.is_none()
    }));
}

#[cfg(any(unix, windows))]
#[test]
fn typescript_grammar_covers_ts_tsx_mts_and_cts_extensions() {
    let mut registry = SyntaxGrammarRegistry::with_first_party_native();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let query = std::fs::read_to_string(format!(
        "{manifest_dir}/packages/typescript/queries/highlights.scm"
    ))
    .expect("read TypeScript highlights query");

    for (path, contribution_id, language_id, language, fixture) in [
        (
            "src/app.ts",
            "typescript.typescript",
            "typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "tests/fixtures/syntax/typescript.ts",
        ),
        (
            "src/app.tsx",
            "typescript.tsx",
            "tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            "tests/fixtures/syntax/typescript.tsx",
        ),
        (
            "src/app.mts",
            "typescript.typescript",
            "typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "tests/fixtures/syntax/typescript.ts",
        ),
        (
            "src/app.cts",
            "typescript.typescript",
            "typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "tests/fixtures/syntax/typescript.ts",
        ),
    ] {
        let selection = registry.select_for_document(
            &classification_input(71, Some(path)),
            &core_code_activation(71, 1),
            1,
        );
        let grammar = selection
            .active_syntax_grammar
            .unwrap_or_else(|| panic!("{path} should select Tier 1 syntax"));
        let contribution = registry
            .get(contribution_id)
            .expect("registered TS grammar")
            .clone();

        assert_eq!(grammar.contribution_id, contribution_id);
        assert_eq!(grammar.language_id, language_id);
        assert_eq!(contribution.engine_tier, SyntaxEngineTier::Native);
        assert_eq!(
            contribution.grammar_source.as_deref(),
            Some("tree-sitter-typescript")
        );

        let source = std::fs::read_to_string(format!("{manifest_dir}/{fixture}"))
            .expect("read TypeScript fixture");
        let set = TreeSitterSyntaxHandler::new(contribution, language, &query)
            .expect("TypeScript query compiles")
            .parse_sync(parse_notification_for("typescript", 1, &source))
            .expect("TypeScript fixture parses")
            .decoration_updates
            .expect("TypeScript decorations");
        assert!(set.spans.iter().any(|span| {
            span.token_type == TokenType::Function
                && span.modifiers.contains(Modifiers::DECLARATION)
        }));
        assert!(
            set.spans
                .iter()
                .any(|span| span.token_type == TokenType::Type)
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn javascript_grammar_covers_js_jsx_mjs_cjs_extensions() {
    let mut registry = SyntaxGrammarRegistry::with_first_party_native();

    for extension in ["js", "jsx", "mjs", "cjs"] {
        let path = format!("src/app.{extension}");
        let selection = registry.select_for_document(
            &classification_input(72, Some(&path)),
            &core_code_activation(72, 1),
            1,
        );
        let grammar = selection
            .active_syntax_grammar
            .unwrap_or_else(|| panic!("{path} should select Tier 1 syntax"));

        assert_eq!(grammar.contribution_id, "javascript.javascript");
        assert_eq!(grammar.language_id, "javascript");
    }

    let contribution = registry
        .get("javascript.javascript")
        .expect("native JavaScript grammar")
        .clone();
    assert_eq!(contribution.engine_tier, SyntaxEngineTier::Native);
    assert_eq!(
        contribution.grammar_source.as_deref(),
        Some("tree-sitter-javascript")
    );

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let query = std::fs::read_to_string(format!(
        "{manifest_dir}/packages/javascript/queries/highlights.scm"
    ))
    .expect("read JavaScript highlights query");
    let source = std::fs::read_to_string(format!(
        "{manifest_dir}/tests/fixtures/syntax/javascript.js"
    ))
    .expect("read JavaScript fixture");
    let set = TreeSitterSyntaxHandler::new(
        contribution,
        tree_sitter_javascript::LANGUAGE.into(),
        &query,
    )
    .expect("JavaScript query compiles")
    .parse_sync(parse_notification_for("javascript", 1, &source))
    .expect("JavaScript fixture parses")
    .decoration_updates
    .expect("JavaScript decorations");
    assert!(set.spans.iter().any(|span| {
        span.token_type == TokenType::Function && span.modifiers.contains(Modifiers::DECLARATION)
    }));
}

#[cfg(any(unix, windows))]
#[test]
fn markdown_grammar_emits_prose_vocabulary_tokens_with_modifiers() {
    let registry = SyntaxGrammarRegistry::with_first_party_native();
    let contribution = registry
        .get("markdown.markdown")
        .expect("native Markdown grammar")
        .clone();
    let query = std::fs::read_to_string(format!(
        "{}/packages/markdown/queries/highlights.scm",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("read Markdown highlights query");
    let mut handler =
        TreeSitterSyntaxHandler::new(contribution, tree_sitter_md_025::LANGUAGE.into(), &query)
            .expect("Markdown query compiles");
    handler
        .enable_injections(include_str!("../packages/markdown/queries/injections.scm"))
        .expect("Markdown injections query compiles");
    let source = "# h1\n## h2\n### h3\n#### h4\n##### h5\n###### h6\n\n**bold**\n\n_emphasis_\n\n`code`\n\n[x](https://example.com)\n\n- item\n\n> quote\n\n```rust\nfn main() {}\n```\n";
    let set = handler
        .parse_sync(parse_notification_for("markdown", 1, source))
        .expect("Markdown parses")
        .decoration_updates
        .expect("Markdown decorations");

    for token_type in [
        TokenType::Heading1,
        TokenType::Heading2,
        TokenType::Heading3,
        TokenType::Heading4,
        TokenType::Heading5,
        TokenType::Heading6,
        TokenType::CodeSpan,
        TokenType::CodeBlock,
        TokenType::ListItem,
        TokenType::Link,
        TokenType::Quote,
    ] {
        assert!(
            set.spans.iter().any(|span| span.token_type == token_type),
            "Markdown should emit {token_type:?} through styleMap"
        );
    }
    assert!(set.spans.iter().any(|span| {
        span.token_type == TokenType::Paragraph && span.modifiers.contains(Modifiers::BOLD)
    }));
    assert!(set.spans.iter().any(|span| {
        span.token_type == TokenType::Paragraph && span.modifiers.contains(Modifiers::ITALIC)
    }));
    assert!(set.spans.iter().all(|span| span.scope.is_none()));
}

#[cfg(any(unix, windows))]
#[test]
fn markdown_inline_injection_styles_mixed_runs() {
    // Generic injection engine: block parse -> injections.scm declares the
    // `inline` ranges for `markdown_inline` -> embedded grammar re-parses
    // those ranges -> inline captures flow through the same styleMap. Mixed
    // inline runs (the old regex-predicate dead end) must now style.
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
    let source = "# Title\n\nSee **bold** and *italic* and `code` and [link](https://example.com) inline.\n\n# Title with `code`\n\n<https://example.com> and <a@b.c>\n\n```unknownlanguage\nfn mixed() {}\n```\n";
    let set = handler
        .parse_sync(parse_notification_for("markdown", 1, source))
        .expect("Markdown parses")
        .decoration_updates
        .expect("Markdown decorations");

    // Spans crossing a 128-byte decoration chunk boundary are split; the
    // editor coalesces adjacent same-token spans (plan058), so covering the
    // needle's first byte is the honest per-chunk assertion.
    let covering = |needle: &str| {
        let start = source.find(needle).expect("needle") as u64;
        move |span: &DecorationSpan| span.byte_start <= start && span.byte_end > start
    };
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Paragraph
                && span.modifiers.contains(Modifiers::BOLD)
                && covering("**bold**")(span)),
        "mixed-run strong_emphasis must emit Paragraph+BOLD"
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Paragraph
                && span.modifiers.contains(Modifiers::ITALIC)
                && covering("*italic*")(span)),
        "mixed-run emphasis must emit Paragraph+ITALIC"
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::CodeSpan && covering("`code`")(span)),
        "mixed-run code_span must emit CodeSpan"
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Link
                && covering("[link](https://example.com)")(span)),
        "mixed-run inline_link must emit Link"
    );
    // Narrow inline captures outrank broad prose via declarative priority.
    let paragraph_priority = set
        .spans
        .iter()
        .find(|span| span.token_type == TokenType::Paragraph && span.modifiers == Modifiers::NONE)
        .map(|span| span.priority);
    assert_eq!(paragraph_priority, Some(70));
    for token_type in [TokenType::CodeSpan, TokenType::Link] {
        let priority = set
            .spans
            .iter()
            .find(|span| span.token_type == token_type)
            .map(|span| span.priority);
        assert_eq!(priority, Some(80), "{token_type:?} narrow capture priority");
    }
    // Plain paragraph prose stays Paragraph without inline modifiers.
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Paragraph
                && span.modifiers == Modifiers::NONE
                && covering("See")(span)),
        "plain prose must stay unmodified Paragraph"
    );
    // Inline inside headings: Heading1 base with CodeSpan sub-range.
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Heading1 && covering("Title with")(span)),
        "heading text keeps Heading1"
    );
    let heading_code_start = source.rfind("`code`").expect("heading code") as u64;
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::CodeSpan
                && span.byte_start <= heading_code_start
                && span.byte_end > heading_code_start),
        "code span inside heading must emit CodeSpan"
    );
    // Autolinks map to Link.
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Link
                && covering("<https://example.com>")(span)),
        "uri_autolink must emit Link"
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Link && covering("<a@b.c>")(span)),
        "email_autolink must emit Link"
    );
    // Unregistered fenced-code language: skipped gracefully, block-level
    // CodeBlock span still ships.
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::CodeBlock),
        "fenced code keeps the block-level CodeBlock span"
    );
}

#[cfg(any(unix, windows))]
#[test]
fn markdown_inline_injection_does_not_pair_backticks_across_lines() {
    // Regression: batching every `(inline)` injection range into one
    // `set_included_ranges` call fed the bytes *between* inline blocks
    // (newlines, `- ` list markers) to markdown_inline, which then paired a
    // line's closing backtick with the next line's opening backtick. The
    // result painted following plain text ("\n- `", "\n\nSteps:") as code.
    // Each inline block must parse in isolation, so a code span never crosses
    // a line boundary.
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
    let source = "Optional bindings you have:\n\n- `Ctrl+Shift+Z` → `clientRequestResync`\n- `Ctrl+Shift+D` → `clientDismissRecovery`\n\nSteps:\n";
    let set = handler
        .parse_sync(parse_notification_for("markdown", 1, source))
        .expect("Markdown parses")
        .decoration_updates
        .expect("Markdown decorations");

    for needle in [
        "`Ctrl+Shift+Z`",
        "`clientRequestResync`",
        "`Ctrl+Shift+D`",
        "`clientDismissRecovery`",
    ] {
        let start = source.find(needle).expect("needle") as u64;
        let end = start + needle.len() as u64;
        let exact = set.spans.iter().any(|span| {
            span.token_type == TokenType::CodeSpan
                && span.byte_start == start
                && span.byte_end == end
        });
        assert!(
            exact,
            "{needle} must be exactly one CodeSpan; got {:?}",
            set.spans
                .iter()
                .filter(|span| span.token_type == TokenType::CodeSpan)
                .map(|span| (span.byte_start, span.byte_end))
                .collect::<Vec<_>>()
        );
    }
    // No code span may span a newline: the cross-line pairing bug produced
    // captures like "`clientRequestResync`\n- `".
    for span in set
        .spans
        .iter()
        .filter(|s| s.token_type == TokenType::CodeSpan)
    {
        let slice = &source[span.byte_start as usize..span.byte_end as usize];
        assert!(
            !slice.contains('\n'),
            "CodeSpan must not cross a line boundary: {slice:?}"
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn markdown_decoration_renders_through_tier1_native_engine() {
    let registry = SyntaxGrammarRegistry::with_first_party_native();
    let contribution = registry
        .get("markdown.markdown")
        .expect("compiled Markdown descriptor")
        .clone();
    assert_eq!(contribution.engine_tier, SyntaxEngineTier::Native);
    assert_eq!(
        contribution.grammar_source.as_deref(),
        Some("tree-sitter-md-025")
    );

    let query = include_str!("../packages/markdown/queries/highlights.scm");
    let set =
        TreeSitterSyntaxHandler::new(contribution, tree_sitter_md_025::LANGUAGE.into(), query)
            .expect("compiled Markdown query")
            .parse_sync(parse_notification_for(
                "markdown",
                1,
                "# heading\n\n> quote\n",
            ))
            .expect("native Markdown parse")
            .decoration_updates
            .expect("native Markdown decoration set");

    assert!(set.spans.iter().any(|span| {
        span.token_type == TokenType::Heading1 && span.provenance.package_version == "builtin"
    }));
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Quote)
    );
}

#[test]
fn first_party_artifact_provenance_is_recorded() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    for (package_dir, wasm_name, upstream) in [
        ("rust", "rust.wasm", "tree-sitter-rust = 0.24.2"),
        (
            "typescript",
            "typescript.wasm",
            "tree-sitter-typescript = 0.23.2",
        ),
        (
            "javascript",
            "javascript.wasm",
            "tree-sitter-javascript = 0.25.0",
        ),
        ("markdown", "markdown.wasm", "tree-sitter-md-025 = 0.5.6"),
    ] {
        let grammar_dir = format!("{manifest_dir}/packages/{package_dir}/grammars");
        let provenance_path = format!("{grammar_dir}/PROVENANCE.md");
        let provenance = std::fs::read_to_string(&provenance_path)
            .unwrap_or_else(|error| panic!("read {provenance_path}: {error}"));
        let wasm_path = format!("{grammar_dir}/{wasm_name}");
        let has_committed_wasm = std::path::Path::new(&wasm_path).exists();

        assert!(provenance.contains(upstream));
        assert!(provenance.contains("sha256sum"));
        assert!(provenance.contains("No network fetch"));
        assert!(provenance.contains("No network fetch, package-manager install, shell build, or native-library load occurs at Clay runtime"));
        assert!(
            has_committed_wasm || provenance.contains("is not committed yet"),
            "{package_dir} must either commit {wasm_name} or document reproducible build provenance"
        );
        assert!(
            std::path::Path::new(&format!(
                "{manifest_dir}/packages/{package_dir}/queries/highlights.scm"
            ))
            .exists()
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn syntax_pipeline_maps_captures_to_vocabulary_tokens() {
    let record = rust_record();
    let contribution = rust_contribution(&record);

    let keyword = map_capture_to_vocabulary(
        &contribution,
        &SyntaxCapture {
            byte_start: 0,
            byte_end: 2,
            capture_name: "keyword".to_string(),
        },
    )
    .expect("keyword capture maps");
    let unmapped = map_capture_to_vocabulary(
        &contribution,
        &SyntaxCapture {
            byte_start: 3,
            byte_end: 7,
            capture_name: "function.declaration".to_string(),
        },
    );

    assert_eq!(keyword.token_type, TokenType::Keyword);
    assert_eq!(keyword.modifiers, Modifiers::NONE);
    assert!(matches!(
        unmapped,
        Err(TreeSitterSyntaxError::QueryCaptureNotMapped { capture }) if capture == "function.declaration"
    ));
}

#[cfg(any(unix, windows))]
#[test]
fn tree_sitter_handler_extracts_highlight_captures_as_bounded_decorations() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), rust_highlights_query())
            .expect("query compiles");
    let text = "fn main() {\n  let s = \"hello\"; // greet\n}\n";

    let update = handler
        .parse_sync(parse_notification(1, text))
        .expect("tree-sitter parse succeeds");
    let set = update.decoration_updates.expect("syntax decorations");

    assert_eq!(
        update.syntax_tree_delta.as_deref(),
        Some("tree-sitter:rust:full")
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Keyword)
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::String)
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Comment)
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Operator)
    );
    assert!(
        set.spans
            .iter()
            .all(|span| span.byte_start >= set.viewport_byte_start)
    );
    assert!(
        set.spans
            .iter()
            .all(|span| span.byte_end <= set.viewport_byte_end)
    );
    assert!(
        set.spans
            .iter()
            .all(|span| span.provenance.package_prefix == "rust")
    );
}

#[cfg(any(unix, windows))]
#[test]
fn tree_sitter_highlighting_does_not_emit_range_diagnostics() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler = TreeSitterSyntaxHandler::new(contribution, rust_language(), "")
        .expect("empty highlight query compiles");

    for (version, text) in [
        (1, "fn main() { let value = ; }"),
        (2, "fn main() { let x = 1 let y = 2; }"),
        (3, "fn main() {}"),
    ] {
        assert!(
            handler
                .parse_sync(parse_notification(version, text))
                .expect("tree-sitter parse succeeds")
                .diagnostic_update
                .is_none(),
            "syntax highlighting must not masquerade as analyzer diagnostics"
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn scroll_sized_native_sources_produce_bounded_decorations() {
    let registry = SyntaxGrammarRegistry::with_first_party_native();
    for (label, prefix, contribution_id, language, query, source) in [
        (
            "Rust",
            "rust",
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("../packages/rust/queries/highlights.scm"),
            "fn value() -> usize { 42 }\n".repeat(80),
        ),
        (
            "TypeScript",
            "typescript",
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "const value: number = 42;\n".repeat(80),
        ),
        (
            "TSX",
            "typescript",
            "typescript.tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "const view = <div>{42}</div>;\n".repeat(80),
        ),
        (
            "JavaScript",
            "javascript",
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("../packages/javascript/queries/highlights.scm"),
            "const value = 42;\n".repeat(80),
        ),
        (
            "Markdown",
            "markdown",
            "markdown.markdown",
            tree_sitter_md_025::LANGUAGE.into(),
            include_str!("../packages/markdown/queries/highlights.scm"),
            "## Heading\n\nParagraph with `code`.\n\n".repeat(40),
        ),
    ] {
        let contribution = registry
            .get(contribution_id)
            .unwrap_or_else(|| panic!("native {label} grammar"))
            .clone();
        let handler = TreeSitterSyntaxHandler::new(contribution, language, query)
            .unwrap_or_else(|error| panic!("{label} highlight query: {error}"));

        let update = handler
            .parse_sync(parse_notification_for(prefix, 1, &source))
            .unwrap_or_else(|error| panic!("scroll-sized {label} parses: {error}"));
        assert!(!update.decoration_updates.is_empty(), "{label}");
        for set in &update.decoration_updates {
            assert!(
                rkyv::to_bytes::<rkyv::rancor::Error>(set)
                    .expect("serialize bounded decorations")
                    .len()
                    <= clay::perf::budgets::DECORATION_PAYLOAD_BUDGET_BYTES,
                "{label}"
            );
            let mut member = update.clone();
            member.decoration_updates = vec![set.clone()];
            assert!(
                rkyv::to_bytes::<rkyv::rancor::Error>(&member)
                    .expect("serialize bounded parse update")
                    .len()
                    <= clay::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES,
                "{label}"
            );
        }
    }
}

#[cfg(any(unix, windows))]
#[test]
fn first_party_invalid_fixtures_do_not_masquerade_as_analyzer_diagnostics() {
    let registry = SyntaxGrammarRegistry::with_first_party_native();
    for (contribution_id, language, source) in [
        (
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            "fn main() { let = ;",
        ),
        (
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "const value: = ;",
        ),
        (
            "typescript.tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            "const view = <div>;",
        ),
        (
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            "const = ;",
        ),
    ] {
        let contribution = registry
            .get(contribution_id)
            .unwrap_or_else(|| panic!("registered {contribution_id}"))
            .clone();
        let handler = TreeSitterSyntaxHandler::new(contribution, language, "")
            .unwrap_or_else(|error| panic!("{contribution_id} handler: {error}"));
        assert!(
            handler
                .parse_sync(parse_notification_for(
                    contribution_id.split('.').next().unwrap(),
                    1,
                    source,
                ))
                .unwrap_or_else(|error| panic!("{contribution_id} invalid parse: {error}"))
                .diagnostic_update
                .is_none(),
            "{contribution_id} highlighting must not publish diagnostics"
        );
    }

    let syntax_source = std::fs::read_to_string(format!(
        "{}/src/server/syntax.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("read generic syntax handler source");
    assert!(!syntax_source.contains("match self.contribution.language_id"));
}

#[cfg(any(unix, windows))]
#[test]
fn tree_sitter_handler_reuses_cached_tree_for_later_document_versions() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), rust_highlights_query())
            .expect("query compiles");

    let first = handler
        .parse_sync(parse_notification(1, "fn main() {}\n"))
        .expect("initial parse");
    let second_text = "fn main() {\n  // changed\n}\n";
    let mut second_notification = parse_notification(2, second_text);
    second_notification.accepted_edit = Some(ParseInputEdit {
        base_document_version: 1,
        document_version: 2,
        start_byte: 0,
        old_end_byte: 13,
        new_end_byte: second_text.len() as u64,
        start_position: ParsePoint::new(0, 0),
        old_end_position: ParsePoint::new(1, 0),
        new_end_position: ParsePoint::new(3, 0),
    });
    second_notification.parse_windows[0].incremental_edit = true;
    let second = handler
        .parse_sync(second_notification)
        .expect("incremental parse");

    assert_eq!(
        first.syntax_tree_delta.as_deref(),
        Some("tree-sitter:rust:full")
    );
    assert_eq!(
        second.syntax_tree_delta.as_deref(),
        Some("tree-sitter:rust:incremental")
    );
    assert_eq!(handler.cached_tree_version(11), Some(2));
}

#[cfg(any(unix, windows))]
#[test]
fn incremental_keyword_completion_republishes_complete_replacement_chunk() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler = TreeSitterSyntaxHandler::new(
        contribution,
        rust_language(),
        "\"fn\" @keyword\n(let_declaration \"let\" @keyword)",
    )
    .expect("query compiles");
    let old_text = "fn main() { le value = 1; }\n";
    let new_text = "fn main() { let value = 1; }\n";
    let insertion = old_text.find("le").expect("partial keyword") + 2;

    handler
        .parse_sync(parse_notification(1, old_text))
        .expect("initial parse");
    let update = handler
        .parse_sync(incremental_notification(
            old_text,
            new_text,
            insertion,
            insertion,
            insertion + 1,
        ))
        .expect("incremental parse");
    ParseCoordinator::new()
        .validate_update(&update)
        .expect("changed-range update metadata validates");
    let set = update.decoration_updates.expect("changed decoration range");
    let keyword_start = new_text.find("let").expect("completed keyword") as u64;

    assert!(
        set.spans.iter().any(|span| {
            span.token_type == TokenType::Keyword
                && span.byte_start == keyword_start
                && span.byte_end == keyword_start + 3
        }),
        "completed keyword missing from {set:?}"
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.byte_start == 0 && span.byte_end == 2),
        "unchanged `fn` is missing from complete replacement chunk"
    );
}

#[cfg(any(unix, windows))]
#[test]
fn incremental_comment_opener_requeries_containing_capture() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), rust_highlights_query())
            .expect("query compiles");
    let old_text = "fn main() { / note\nlet value = 1; }\n";
    let slash = old_text.find('/').expect("slash");
    let mut new_text = old_text.to_string();
    new_text.insert(slash, '/');

    handler
        .parse_sync(parse_notification(1, old_text))
        .expect("initial parse");
    let set = handler
        .parse_sync(incremental_notification(
            old_text,
            &new_text,
            slash,
            slash,
            slash + 1,
        ))
        .expect("incremental parse")
        .decoration_updates
        .expect("changed decoration range");
    let comment_end = new_text.find('\n').expect("line end") as u64;

    assert!(set.spans.iter().any(|span| {
        span.token_type == TokenType::Comment
            && span.byte_start == slash as u64
            && span.byte_end == comment_end
    }));
}

#[cfg(any(unix, windows))]
#[test]
fn incremental_string_opener_requeries_complete_string_capture() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), rust_highlights_query())
            .expect("query compiles");
    let old_text = "fn main() { let value = note\"; }\n";
    let insertion = old_text.find("note").expect("string text");
    let mut new_text = old_text.to_string();
    new_text.insert(insertion, '"');

    handler
        .parse_sync(parse_notification(1, old_text))
        .expect("initial parse");
    let set = handler
        .parse_sync(incremental_notification(
            old_text,
            &new_text,
            insertion,
            insertion,
            insertion + 1,
        ))
        .expect("incremental parse")
        .decoration_updates
        .expect("changed decoration range");
    let string_end = new_text.rfind('"').expect("string end") as u64 + 1;

    assert!(set.spans.iter().any(|span| {
        span.token_type == TokenType::String
            && span.byte_start == insertion as u64
            && span.byte_end == string_end
    }));
}

#[cfg(any(unix, windows))]
#[test]
fn incremental_string_closer_requeries_complete_string_capture() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), rust_highlights_query())
            .expect("query compiles");
    let old_text = "fn main() { let value = \"note; }\n";
    let insertion = old_text.find(';').expect("semicolon");
    let mut new_text = old_text.to_string();
    new_text.insert(insertion, '"');

    handler
        .parse_sync(parse_notification(1, old_text))
        .expect("initial parse");
    let set = handler
        .parse_sync(incremental_notification(
            old_text,
            &new_text,
            insertion,
            insertion,
            insertion + 1,
        ))
        .expect("incremental parse")
        .decoration_updates
        .expect("changed decoration range");
    let string_start = new_text.find('"').expect("string start") as u64;

    assert!(set.spans.iter().any(|span| {
        span.token_type == TokenType::String
            && span.byte_start == string_start
            && span.byte_end == insertion as u64 + 1
    }));
}

#[cfg(any(unix, windows))]
#[test]
fn incremental_newline_shortens_line_comment_capture() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), rust_highlights_query())
            .expect("query compiles");
    let old_text = "fn main() { // note let hidden = 1;\n}\n";
    let insertion = old_text.find(" let hidden").expect("comment suffix");
    let mut new_text = old_text.to_string();
    new_text.insert(insertion, '\n');

    handler
        .parse_sync(parse_notification(1, old_text))
        .expect("initial parse");
    let set = handler
        .parse_sync(incremental_notification(
            old_text,
            &new_text,
            insertion,
            insertion,
            insertion + 1,
        ))
        .expect("incremental parse")
        .decoration_updates
        .expect("changed decoration range");

    assert!(set.spans.iter().any(|span| {
        span.token_type == TokenType::Comment && span.byte_end == insertion as u64
    }));
}

#[cfg(any(unix, windows))]
#[test]
fn plan058_repeated_comment_edits_do_not_grow_a_shifted_chunk_boundary_gap() {
    let handler = first_party_rust_handler();
    let mut text = format!("//{}\nfn after() {{}}\n", "c".repeat(150));
    let mut editor = apply_initial_syntax(&handler, &text);
    let mut caret = 2;
    assert!(editor.navigate_to_byte_offset(caret));
    let mut authority_gap_bytes = Vec::new();
    let mut transition_failures = Vec::new();

    for base_version in 1..=3 {
        let old_text = text.clone();
        assert!(editor.insert_text("x"));
        text.insert(caret as usize, 'x');
        caret += 1;
        let comment_end = text.find('\n').expect("line comment terminator");
        let expected = [("comment suffix", 128..comment_end)];
        record_undecorated_ranges(
            &mut transition_failures,
            &format!("version {} optimistic edit", base_version + 1),
            &editor,
            &expected,
        );
        assert!(editor.note_confirmed_version(11, base_version + 1));
        record_undecorated_ranges(
            &mut transition_failures,
            &format!("version {} acknowledgement", base_version + 1),
            &editor,
            &expected,
        );

        let update = handler
            .parse_sync(incremental_notification_for_versions(
                "rust",
                base_version,
                &old_text,
                &text,
                caret as usize - 1,
                caret as usize - 1,
                caret as usize,
            ))
            .expect("incremental syntax parse");
        assert_eq!(
            update.syntax_tree_delta.as_deref(),
            Some("tree-sitter:rust:incremental")
        );
        assert_eq!(
            update.decoration_updates.len(),
            1,
            "one accepted edit should produce one touched replacement member"
        );
        for (index, set) in update.decoration_updates.into_iter().enumerate() {
            assert!(editor.apply_decoration_set(set));
            record_undecorated_ranges(
                &mut transition_failures,
                &format!("version {} authoritative member {index}", base_version + 1),
                &editor,
                &expected,
            );
        }
        let painted = editor.visible_decoration_paint_ranges_for_test();
        authority_gap_bytes.push(
            (128..comment_end)
                .filter(|byte| !painted.iter().any(|(range, _)| range.contains(byte)))
                .count(),
        );
    }

    assert_eq!(
        authority_gap_bytes,
        vec![0, 0, 0],
        "each authoritative response currently exposes one more base-colored byte"
    );
    assert!(
        transition_failures.is_empty(),
        "decoration continuity regressed:\n{}",
        transition_failures.join("\n")
    );
}

#[cfg(any(unix, windows))]
#[test]
fn plan058_first_party_languages_preserve_shifted_boundary_continuity() {
    for (label, package_prefix, contribution_id, language_id, language, query, mut text) in [
        (
            "Rust",
            "rust",
            "rust.rust",
            "rust",
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("../packages/rust/queries/highlights.scm"),
            format!("//{}\nfn after() {{}}\n", "c".repeat(150)),
        ),
        (
            "TypeScript",
            "typescript",
            "typescript.typescript",
            "typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            format!("//{}\nfunction after() {{}}\n", "c".repeat(150)),
        ),
        (
            "TSX",
            "typescript",
            "typescript.tsx",
            "tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            format!(
                "//{}\nfunction After() {{ return <div />; }}\n",
                "c".repeat(150)
            ),
        ),
        (
            "JavaScript",
            "javascript",
            "javascript.javascript",
            "javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("../packages/javascript/queries/highlights.scm"),
            format!("//{}\nfunction after() {{}}\n", "c".repeat(150)),
        ),
        (
            "Markdown",
            "markdown",
            "markdown.markdown",
            "markdown",
            tree_sitter_md_025::LANGUAGE.into(),
            include_str!("../packages/markdown/queries/highlights.scm"),
            format!("{}\n\n# After\n", "p".repeat(152)),
        ),
    ] {
        let handler = first_party_handler(contribution_id, language, query);
        let mut editor = apply_initial_syntax_for(&handler, package_prefix, language_id, &text);
        let mut caret = if package_prefix == "markdown" { 1 } else { 2 };
        assert!(editor.navigate_to_byte_offset(caret), "{label}: caret");
        let mut failures = Vec::new();

        for base_version in 1..=3 {
            let old_text = text.clone();
            assert!(editor.insert_text("x"), "{label}: local insertion");
            text.insert(caret as usize, 'x');
            caret += 1;
            let decorated_end = text.find('\n').expect("decorated line end");
            let expected = [("decorated suffix", 128..decorated_end)];
            record_undecorated_ranges(
                &mut failures,
                &format!("{label} version {} optimistic edit", base_version + 1),
                &editor,
                &expected,
            );
            assert!(editor.note_confirmed_version(11, base_version + 1));
            record_undecorated_ranges(
                &mut failures,
                &format!("{label} version {} acknowledgement", base_version + 1),
                &editor,
                &expected,
            );

            let update = handler
                .parse_sync(incremental_notification_for_versions(
                    package_prefix,
                    base_version,
                    &old_text,
                    &text,
                    caret as usize - 1,
                    caret as usize - 1,
                    caret as usize,
                ))
                .unwrap_or_else(|error| panic!("{label}: incremental parse: {error}"));
            assert_eq!(
                update.syntax_tree_delta.as_deref(),
                Some(format!("tree-sitter:{language_id}:incremental").as_str()),
                "{label}: cached tree"
            );
            assert_eq!(
                update.decoration_updates.len(),
                1,
                "{label}: one touched replacement member"
            );
            for (index, set) in update.decoration_updates.into_iter().enumerate() {
                assert!(editor.apply_decoration_set(set));
                record_undecorated_ranges(
                    &mut failures,
                    &format!(
                        "{label} version {} authoritative member {index}",
                        base_version + 1
                    ),
                    &editor,
                    &expected,
                );
            }
        }

        assert!(
            failures.is_empty(),
            "{label} boundary continuity regressed:\n{}",
            failures.join("\n")
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn plan057_function_suffix_stays_decorated_through_local_ack_and_authoritative_states() {
    let handler = first_party_rust_handler();
    let old_text = "fn main() {}\n";
    let insertion = old_text.find("main").expect("function") + "main".len();
    let mut new_text = old_text.to_string();
    new_text.insert(insertion, 'x');
    let mut editor = apply_initial_syntax(&handler, old_text);
    let mut failures = Vec::new();

    record_undecorated_ranges(
        &mut failures,
        "initial",
        &editor,
        &[("keyword", 0..2), ("function", 3..7)],
    );
    assert!(editor.navigate_to_byte_offset(insertion as u64));
    assert!(editor.insert_text("x"));
    record_undecorated_ranges(
        &mut failures,
        "optimistic local edit",
        &editor,
        &[("keyword", 0..2), ("grown function", 3..8)],
    );
    assert!(editor.note_confirmed_version(11, 2));
    record_undecorated_ranges(
        &mut failures,
        "edit acknowledgement",
        &editor,
        &[("keyword", 0..2), ("grown function", 3..8)],
    );

    let update = handler
        .parse_sync(incremental_notification(
            old_text,
            &new_text,
            insertion,
            insertion,
            insertion + 1,
        ))
        .expect("incremental syntax parse");
    assert_eq!(
        update.syntax_tree_delta.as_deref(),
        Some("tree-sitter:rust:incremental")
    );
    for (index, set) in update.decoration_updates.into_iter().enumerate() {
        assert!(editor.apply_decoration_set(set));
        record_undecorated_ranges(
            &mut failures,
            &format!("authoritative member {index}"),
            &editor,
            &[("keyword", 0..2), ("grown function", 3..8)],
        );
    }

    assert!(
        failures.is_empty(),
        "decoration continuity regressed:\n{}",
        failures.join("\n")
    );
}

#[cfg(any(unix, windows))]
#[test]
fn plan057_newline_keeps_unrelated_short_file_syntax_through_every_state() {
    let handler = first_party_rust_handler();
    let old_text = "fn main() { // note let hidden = 1;\n}\n";
    let insertion = old_text.find(" let hidden").expect("comment suffix");
    let mut new_text = old_text.to_string();
    new_text.insert(insertion, '\n');
    let comment_start = old_text.find("// note").expect("comment");
    let brace = old_text.find('{').expect("opening brace");
    let expected = [
        ("keyword", 0..2),
        ("function", 3..7),
        ("punctuation", brace..brace + 1),
        (
            "comment prefix",
            comment_start..comment_start + "// note".len(),
        ),
    ];
    let mut editor = apply_initial_syntax(&handler, old_text);
    let mut failures = Vec::new();

    record_undecorated_ranges(&mut failures, "initial", &editor, &expected);
    assert!(editor.navigate_to_byte_offset(insertion as u64));
    assert!(editor.insert_newline());
    record_undecorated_ranges(&mut failures, "optimistic newline", &editor, &expected);
    assert!(editor.note_confirmed_version(11, 2));
    record_undecorated_ranges(&mut failures, "edit acknowledgement", &editor, &expected);

    let update = handler
        .parse_sync(incremental_notification(
            old_text,
            &new_text,
            insertion,
            insertion,
            insertion + 1,
        ))
        .expect("incremental syntax parse");
    assert_eq!(
        update.syntax_tree_delta.as_deref(),
        Some("tree-sitter:rust:incremental")
    );
    for (index, set) in update.decoration_updates.into_iter().enumerate() {
        assert!(editor.apply_decoration_set(set));
        record_undecorated_ranges(
            &mut failures,
            &format!("authoritative member {index}"),
            &editor,
            &expected,
        );
    }

    assert!(
        failures.is_empty(),
        "decoration continuity regressed:\n{}",
        failures.join("\n")
    );
}

#[cfg(any(unix, windows))]
#[test]
fn plan057_first_party_languages_keep_continuity_across_edit_boundaries() {
    let rust_query = include_str!("../packages/rust/queries/highlights.scm");
    let typescript_query = include_str!("../packages/typescript/queries/highlights.scm");
    let javascript_query = include_str!("../packages/javascript/queries/highlights.scm");
    let markdown_query = include_str!("../packages/markdown/queries/highlights.scm");

    for case in [
        ContinuityCase {
            label: "Rust word growth",
            package_prefix: "rust",
            contribution_id: "rust.rust",
            language_id: "rust",
            language: || tree_sitter_rust::LANGUAGE.into(),
            query: rust_query,
            old_text: "fn app() {}\n",
            new_text: "fn application() {}\n",
            caret: "fn app".len(),
            action: ContinuityAction::Insert("lication"),
            continuous: &[("keyword", "fn"), ("function", "application")],
            authoritative: &[],
        },
        ContinuityCase {
            label: "TypeScript word growth",
            package_prefix: "typescript",
            contribution_id: "typescript.typescript",
            language_id: "typescript",
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            query: typescript_query,
            old_text: "function app() {}\n",
            new_text: "function application() {}\n",
            caret: "function app".len(),
            action: ContinuityAction::Insert("lication"),
            continuous: &[("keyword", "function"), ("function", "application")],
            authoritative: &[],
        },
        ContinuityCase {
            label: "TSX word growth",
            package_prefix: "typescript",
            contribution_id: "typescript.tsx",
            language_id: "tsx",
            language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
            query: typescript_query,
            old_text: "function app() {}\n",
            new_text: "function application() {}\n",
            caret: "function app".len(),
            action: ContinuityAction::Insert("lication"),
            continuous: &[("keyword", "function"), ("function", "application")],
            authoritative: &[],
        },
        ContinuityCase {
            label: "JavaScript word growth",
            package_prefix: "javascript",
            contribution_id: "javascript.javascript",
            language_id: "javascript",
            language: || tree_sitter_javascript::LANGUAGE.into(),
            query: javascript_query,
            old_text: "function app() {}\n",
            new_text: "function application() {}\n",
            caret: "function app".len(),
            action: ContinuityAction::Insert("lication"),
            continuous: &[("keyword", "function"), ("function", "application")],
            authoritative: &[],
        },
        ContinuityCase {
            label: "Rust comment newline",
            package_prefix: "rust",
            contribution_id: "rust.rust",
            language_id: "rust",
            language: || tree_sitter_rust::LANGUAGE.into(),
            query: rust_query,
            old_text: "fn app() { // note hidden\n}\n",
            new_text: "fn app() { // note\n hidden\n}\n",
            caret: "fn app() { // note".len(),
            action: ContinuityAction::Newline,
            continuous: &[
                ("keyword", "fn"),
                ("function", "app"),
                ("comment", "// note"),
            ],
            authoritative: &[],
        },
        ContinuityCase {
            label: "TypeScript comment newline",
            package_prefix: "typescript",
            contribution_id: "typescript.typescript",
            language_id: "typescript",
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            query: typescript_query,
            old_text: "function app() { // note hidden\n}\n",
            new_text: "function app() { // note\n hidden\n}\n",
            caret: "function app() { // note".len(),
            action: ContinuityAction::Newline,
            continuous: &[
                ("keyword", "function"),
                ("function", "app"),
                ("comment", "// note"),
            ],
            authoritative: &[],
        },
        ContinuityCase {
            label: "TSX comment newline",
            package_prefix: "typescript",
            contribution_id: "typescript.tsx",
            language_id: "tsx",
            language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
            query: typescript_query,
            old_text: "function app() { // note hidden\n}\n",
            new_text: "function app() { // note\n hidden\n}\n",
            caret: "function app() { // note".len(),
            action: ContinuityAction::Newline,
            continuous: &[
                ("keyword", "function"),
                ("function", "app"),
                ("comment", "// note"),
            ],
            authoritative: &[],
        },
        ContinuityCase {
            label: "JavaScript comment newline",
            package_prefix: "javascript",
            contribution_id: "javascript.javascript",
            language_id: "javascript",
            language: || tree_sitter_javascript::LANGUAGE.into(),
            query: javascript_query,
            old_text: "function app() { // note hidden\n}\n",
            new_text: "function app() { // note\n hidden\n}\n",
            caret: "function app() { // note".len(),
            action: ContinuityAction::Newline,
            continuous: &[
                ("keyword", "function"),
                ("function", "app"),
                ("comment", "// note"),
            ],
            authoritative: &[],
        },
        ContinuityCase {
            label: "Rust string growth",
            package_prefix: "rust",
            contribution_id: "rust.rust",
            language_id: "rust",
            language: || tree_sitter_rust::LANGUAGE.into(),
            query: rust_query,
            old_text: "fn app() { let note = \"text\"; }\n",
            new_text: "fn app() { let note = \"texts\"; }\n",
            caret: "fn app() { let note = \"text".len(),
            action: ContinuityAction::Insert("s"),
            continuous: &[
                ("keyword", "fn"),
                ("function", "app"),
                ("string", "\"texts\""),
            ],
            authoritative: &[],
        },
        ContinuityCase {
            label: "TypeScript string growth",
            package_prefix: "typescript",
            contribution_id: "typescript.typescript",
            language_id: "typescript",
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            query: typescript_query,
            old_text: "function app() { const note = \"text\"; }\n",
            new_text: "function app() { const note = \"texts\"; }\n",
            caret: "function app() { const note = \"text".len(),
            action: ContinuityAction::Insert("s"),
            continuous: &[
                ("keyword", "function"),
                ("function", "app"),
                ("string", "\"texts\""),
            ],
            authoritative: &[],
        },
        ContinuityCase {
            label: "TSX string growth",
            package_prefix: "typescript",
            contribution_id: "typescript.tsx",
            language_id: "tsx",
            language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
            query: typescript_query,
            old_text: "function app() { const note = \"text\"; }\n",
            new_text: "function app() { const note = \"texts\"; }\n",
            caret: "function app() { const note = \"text".len(),
            action: ContinuityAction::Insert("s"),
            continuous: &[
                ("keyword", "function"),
                ("function", "app"),
                ("string", "\"texts\""),
            ],
            authoritative: &[],
        },
        ContinuityCase {
            label: "JavaScript string growth",
            package_prefix: "javascript",
            contribution_id: "javascript.javascript",
            language_id: "javascript",
            language: || tree_sitter_javascript::LANGUAGE.into(),
            query: javascript_query,
            old_text: "function app() { const note = \"text\"; }\n",
            new_text: "function app() { const note = \"texts\"; }\n",
            caret: "function app() { const note = \"text".len(),
            action: ContinuityAction::Insert("s"),
            continuous: &[
                ("keyword", "function"),
                ("function", "app"),
                ("string", "\"texts\""),
            ],
            authoritative: &[],
        },
        ContinuityCase {
            label: "Markdown paragraph growth",
            package_prefix: "markdown",
            contribution_id: "markdown.markdown",
            language_id: "markdown",
            language: || tree_sitter_md_025::LANGUAGE.into(),
            query: markdown_query,
            old_text: "Paragraph text.\n",
            new_text: "Paragraph texts.\n",
            caret: "Paragraph text".len(),
            action: ContinuityAction::Insert("s"),
            continuous: &[("paragraph", "Paragraph texts.")],
            authoritative: &[],
        },
        ContinuityCase {
            label: "Markdown code span growth",
            package_prefix: "markdown",
            contribution_id: "markdown.markdown",
            language_id: "markdown",
            language: || tree_sitter_md_025::LANGUAGE.into(),
            query: markdown_query,
            old_text: "`code`\n",
            new_text: "`codes`\n",
            caret: "`code".len(),
            action: ContinuityAction::Insert("s"),
            continuous: &[("code span", "`codes`")],
            authoritative: &[],
        },
        ContinuityCase {
            label: "Markdown prose newline",
            package_prefix: "markdown",
            contribution_id: "markdown.markdown",
            language_id: "markdown",
            language: || tree_sitter_md_025::LANGUAGE.into(),
            query: markdown_query,
            old_text: "# Heading\n\nParagraph text tail.\n",
            new_text: "# Heading\n\nParagraph text\n tail.\n",
            caret: "# Heading\n\nParagraph text".len(),
            action: ContinuityAction::Newline,
            continuous: &[
                ("heading", "Heading"),
                ("paragraph prefix", "Paragraph text"),
            ],
            authoritative: &[],
        },
        ContinuityCase {
            label: "Rust punctuation",
            package_prefix: "rust",
            contribution_id: "rust.rust",
            language_id: "rust",
            language: || tree_sitter_rust::LANGUAGE.into(),
            query: rust_query,
            old_text: "fn app() { }\n",
            new_text: "fn app() { [] }\n",
            caret: "fn app() { ".len(),
            action: ContinuityAction::Insert("[] "),
            continuous: &[("keyword", "fn"), ("function", "app")],
            authoritative: &[("inserted punctuation", "[")],
        },
        ContinuityCase {
            label: "TypeScript punctuation",
            package_prefix: "typescript",
            contribution_id: "typescript.typescript",
            language_id: "typescript",
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            query: typescript_query,
            old_text: "function app() { }\n",
            new_text: "function app() { [] }\n",
            caret: "function app() { ".len(),
            action: ContinuityAction::Insert("[] "),
            continuous: &[("keyword", "function"), ("function", "app")],
            authoritative: &[("inserted punctuation", "[")],
        },
        ContinuityCase {
            label: "TSX punctuation",
            package_prefix: "typescript",
            contribution_id: "typescript.tsx",
            language_id: "tsx",
            language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
            query: typescript_query,
            old_text: "function app() { }\n",
            new_text: "function app() { [] }\n",
            caret: "function app() { ".len(),
            action: ContinuityAction::Insert("[] "),
            continuous: &[("keyword", "function"), ("function", "app")],
            authoritative: &[("inserted punctuation", "[")],
        },
        ContinuityCase {
            label: "JavaScript punctuation",
            package_prefix: "javascript",
            contribution_id: "javascript.javascript",
            language_id: "javascript",
            language: || tree_sitter_javascript::LANGUAGE.into(),
            query: javascript_query,
            old_text: "function app() { }\n",
            new_text: "function app() { [] }\n",
            caret: "function app() { ".len(),
            action: ContinuityAction::Insert("[] "),
            continuous: &[("keyword", "function"), ("function", "app")],
            authoritative: &[("inserted punctuation", "[")],
        },
        ContinuityCase {
            label: "Markdown punctuation",
            package_prefix: "markdown",
            contribution_id: "markdown.markdown",
            language_id: "markdown",
            language: || tree_sitter_md_025::LANGUAGE.into(),
            query: markdown_query,
            old_text: "`code`\nparagraph\n",
            new_text: "`code`\n# paragraph\n",
            caret: "`code`\n".len(),
            action: ContinuityAction::Insert("# "),
            continuous: &[("code span", "`code`")],
            authoritative: &[("heading", "paragraph")],
        },
        ContinuityCase {
            label: "Rust deletion",
            package_prefix: "rust",
            contribution_id: "rust.rust",
            language_id: "rust",
            language: || tree_sitter_rust::LANGUAGE.into(),
            query: rust_query,
            old_text: "fn appx() {}\n",
            new_text: "fn app() {}\n",
            caret: "fn appx".len(),
            action: ContinuityAction::Backspace,
            continuous: &[("keyword", "fn"), ("function", "app")],
            authoritative: &[],
        },
        ContinuityCase {
            label: "TypeScript deletion",
            package_prefix: "typescript",
            contribution_id: "typescript.typescript",
            language_id: "typescript",
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            query: typescript_query,
            old_text: "function appx() {}\n",
            new_text: "function app() {}\n",
            caret: "function appx".len(),
            action: ContinuityAction::Backspace,
            continuous: &[("keyword", "function"), ("function", "app")],
            authoritative: &[],
        },
        ContinuityCase {
            label: "TSX deletion",
            package_prefix: "typescript",
            contribution_id: "typescript.tsx",
            language_id: "tsx",
            language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
            query: typescript_query,
            old_text: "function appx() {}\n",
            new_text: "function app() {}\n",
            caret: "function appx".len(),
            action: ContinuityAction::Backspace,
            continuous: &[("keyword", "function"), ("function", "app")],
            authoritative: &[],
        },
        ContinuityCase {
            label: "JavaScript deletion",
            package_prefix: "javascript",
            contribution_id: "javascript.javascript",
            language_id: "javascript",
            language: || tree_sitter_javascript::LANGUAGE.into(),
            query: javascript_query,
            old_text: "function appx() {}\n",
            new_text: "function app() {}\n",
            caret: "function appx".len(),
            action: ContinuityAction::Backspace,
            continuous: &[("keyword", "function"), ("function", "app")],
            authoritative: &[],
        },
        ContinuityCase {
            label: "Markdown deletion",
            package_prefix: "markdown",
            contribution_id: "markdown.markdown",
            language_id: "markdown",
            language: || tree_sitter_md_025::LANGUAGE.into(),
            query: markdown_query,
            old_text: "`codes`\n",
            new_text: "`code`\n",
            caret: "`codes".len(),
            action: ContinuityAction::Backspace,
            continuous: &[("code span", "`code`")],
            authoritative: &[],
        },
    ] {
        run_continuity_case(case);
    }
}

#[cfg(any(unix, windows))]
#[test]
fn plan057_authoritative_queries_correct_inherited_code_keywords() {
    for (
        label,
        package_prefix,
        contribution_id,
        language_id,
        language,
        query,
        old_text,
        new_text,
    ) in [
        (
            "Rust",
            "rust",
            "rust.rust",
            "rust",
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("../packages/rust/queries/highlights.scm"),
            "fn app() { let value = 1; }\n",
            "fn app() { letx value = 1; }\n",
        ),
        (
            "TypeScript",
            "typescript",
            "typescript.typescript",
            "typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function app() { const value = 1; }\n",
            "function app() { constx value = 1; }\n",
        ),
        (
            "TSX",
            "typescript",
            "typescript.tsx",
            "tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function app() { const value = 1; }\n",
            "function app() { constx value = 1; }\n",
        ),
        (
            "JavaScript",
            "javascript",
            "javascript.javascript",
            "javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("../packages/javascript/queries/highlights.scm"),
            "function app() { const value = 1; }\n",
            "function app() { constx value = 1; }\n",
        ),
    ] {
        let handler = first_party_handler(contribution_id, language, query);
        let mut editor = apply_initial_syntax_for(&handler, package_prefix, language_id, old_text);
        let keyword = if package_prefix == "rust" {
            "let"
        } else {
            "const"
        };
        let insertion = old_text.find(keyword).expect("keyword") + keyword.len();
        assert!(editor.navigate_to_byte_offset(insertion as u64));
        assert!(editor.insert_text("x"));
        let inherited = insertion - keyword.len()..insertion + 1;
        let mut failures = Vec::new();
        record_undecorated_ranges(
            &mut failures,
            "provisional correction input",
            &editor,
            &[("inherited keyword", inherited.clone())],
        );
        assert!(failures.is_empty(), "{label}: {}", failures.join("\n"));
        assert!(editor.note_confirmed_version(11, 2));

        let (start, old_end, new_end) = single_edit_offsets(old_text, new_text);
        let update = handler
            .parse_sync(incremental_notification_for(
                package_prefix,
                old_text,
                new_text,
                start,
                old_end,
                new_end,
            ))
            .unwrap_or_else(|error| panic!("{label}: correction parse: {error}"));
        for set in update.decoration_updates {
            assert!(
                editor.apply_decoration_set(set),
                "{label}: correction member"
            );
        }
        let painted = editor.visible_decoration_paint_ranges_for_test();
        assert!(
            painted
                .iter()
                .all(|(range, _)| range.end <= inherited.start || range.start >= inherited.end),
            "{label}: authoritative output retained provisional keyword geometry {painted:?}"
        );
        let function = new_text.find("app").expect("function");
        assert!(
            painted
                .iter()
                .any(|(range, _)| range.start <= function && range.end >= function + 3),
            "{label}: correction cleared unrelated function decoration"
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn plan057_empty_authoritative_chunk_clears_only_fully_queried_range() {
    let record = rust_record();
    let handler = TreeSitterSyntaxHandler::new(
        rust_contribution(&record),
        rust_language(),
        "(identifier) @keyword",
    )
    .expect("query compiles");

    handler
        .parse_sync(parse_notification(1, "value"))
        .expect("initial parse");
    let update = handler
        .parse_sync(incremental_notification("value", "1", 0, 5, 1))
        .expect("replacement parse");

    assert_eq!(update.decoration_updates.len(), 1);
    let set = &update.decoration_updates[0];
    assert_eq!((set.viewport_byte_start, set.viewport_byte_end), (0, 1));
    assert!(set.spans.is_empty());
}

#[cfg(any(unix, windows))]
#[test]
fn plan057_utf8_scalar_at_nominal_chunk_boundary_is_never_split() {
    let record = rust_record();
    let handler = TreeSitterSyntaxHandler::new(
        rust_contribution(&record),
        rust_language(),
        "(identifier) @keyword",
    )
    .expect("query compiles");
    let text = format!("{}éclair", " ".repeat(127));

    let update = handler
        .parse_sync(parse_notification(1, &text))
        .expect("UTF-8 parse");

    assert!(update.decoration_updates.len() >= 2);
    assert!(update.decoration_updates.iter().all(|set| {
        text.is_char_boundary(set.viewport_byte_start as usize)
            && text.is_char_boundary(set.viewport_byte_end as usize)
            && set.spans.iter().all(|span| {
                text.is_char_boundary(span.byte_start as usize)
                    && text.is_char_boundary(span.byte_end as usize)
            })
    }));
    assert_eq!(update.decoration_updates[0].viewport_byte_end, 129);
    assert_eq!(update.decoration_updates[1].viewport_byte_start, 129);
}

#[cfg(any(unix, windows))]
#[test]
fn plan057_changed_broad_capture_completely_fills_touched_replacement_chunk() {
    let record = rust_record();
    let handler = TreeSitterSyntaxHandler::new(
        rust_contribution(&record),
        rust_language(),
        "(line_comment) @comment",
    )
    .expect("query compiles");
    let old_text = format!("//{}\n", "a".repeat(300));
    let insertion = 190;
    let mut new_text = old_text.clone();
    new_text.insert(insertion, 'b');

    handler
        .parse_sync(parse_notification(1, &old_text))
        .expect("initial parse");
    let update = handler
        .parse_sync(incremental_notification(
            &old_text,
            &new_text,
            insertion,
            insertion,
            insertion + 1,
        ))
        .expect("incremental broad capture parse");

    assert_eq!(update.decoration_updates.len(), 1);
    let set = &update.decoration_updates[0];
    assert_eq!((set.viewport_byte_start, set.viewport_byte_end), (128, 256));
    assert!(set.spans.iter().any(|span| {
        span.token_type == TokenType::Comment
            && span.byte_start == set.viewport_byte_start
            && span.byte_end == set.viewport_byte_end
    }));
}

#[cfg(any(unix, windows))]
#[test]
fn first_party_package_queries_keep_authoritative_token_boundaries() {
    let registry = SyntaxGrammarRegistry::with_first_party_native();

    for (label, package_prefix, contribution_id, language, query, old_text, new_text, expected) in [
        (
            "rust keyword completion",
            "rust",
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("../packages/rust/queries/highlights.scm"),
            "fn main() { le value = 1; }\n",
            "fn main() { let value = 1; }\n",
            Some((TokenType::Keyword, "let")),
        ),
        (
            "typescript keyword completion",
            "typescript",
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function main() { con value = 1; }\n",
            "function main() { const value = 1; }\n",
            Some((TokenType::Keyword, "const")),
        ),
        (
            "tsx keyword completion",
            "typescript",
            "typescript.tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function main() { con value = 1; }\n",
            "function main() { const value = 1; }\n",
            Some((TokenType::Keyword, "const")),
        ),
        (
            "javascript keyword completion",
            "javascript",
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("../packages/javascript/queries/highlights.scm"),
            "function main() { con value = 1; }\n",
            "function main() { const value = 1; }\n",
            Some((TokenType::Keyword, "const")),
        ),
        (
            "markdown heading transition",
            "markdown",
            "markdown.markdown",
            tree_sitter_md_025::LANGUAGE.into(),
            include_str!("../packages/markdown/queries/highlights.scm"),
            "# heading\n",
            "## heading\n",
            Some((TokenType::Heading2, "heading")),
        ),
        (
            "rust identifier growth",
            "rust",
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("../packages/rust/queries/highlights.scm"),
            "fn app() {}\n",
            "fn application() {}\n",
            Some((TokenType::Function, "application")),
        ),
        (
            "typescript identifier growth",
            "typescript",
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function app() {}\n",
            "function application() {}\n",
            Some((TokenType::Function, "application")),
        ),
        (
            "tsx identifier growth",
            "typescript",
            "typescript.tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function app() {}\n",
            "function application() {}\n",
            Some((TokenType::Function, "application")),
        ),
        (
            "javascript identifier growth",
            "javascript",
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("../packages/javascript/queries/highlights.scm"),
            "function app() {}\n",
            "function application() {}\n",
            Some((TokenType::Function, "application")),
        ),
        (
            "rust punctuation insertion",
            "rust",
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("../packages/rust/queries/highlights.scm"),
            "fn main()\n",
            "fn main() {}\n",
            Some((TokenType::Operator, "{")),
        ),
        (
            "typescript punctuation insertion",
            "typescript",
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function main()\n",
            "function main() {}\n",
            Some((TokenType::Operator, "{")),
        ),
        (
            "tsx punctuation insertion",
            "typescript",
            "typescript.tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function main()\n",
            "function main() {}\n",
            Some((TokenType::Operator, "{")),
        ),
        (
            "javascript punctuation insertion",
            "javascript",
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("../packages/javascript/queries/highlights.scm"),
            "function main()\n",
            "function main() {}\n",
            Some((TokenType::Operator, "{")),
        ),
        (
            "markdown punctuation insertion",
            "markdown",
            "markdown.markdown",
            tree_sitter_md_025::LANGUAGE.into(),
            include_str!("../packages/markdown/queries/highlights.scm"),
            "heading\n",
            "# heading\n",
            Some((TokenType::Operator, "#")),
        ),
        (
            "rust keyword removal",
            "rust",
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("../packages/rust/queries/highlights.scm"),
            "fn main() { let value = 1; }\n",
            "fn main() { le value = 1; }\n",
            None,
        ),
        (
            "typescript keyword removal",
            "typescript",
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function main() { const value = 1; }\n",
            "function main() { con value = 1; }\n",
            None,
        ),
        (
            "tsx keyword removal",
            "typescript",
            "typescript.tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function main() { const value = 1; }\n",
            "function main() { con value = 1; }\n",
            None,
        ),
        (
            "javascript keyword removal",
            "javascript",
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("../packages/javascript/queries/highlights.scm"),
            "function main() { const value = 1; }\n",
            "function main() { con value = 1; }\n",
            None,
        ),
    ] {
        let contribution = registry
            .get(contribution_id)
            .unwrap_or_else(|| panic!("{label}: contribution is registered"))
            .clone();
        let handler = TreeSitterSyntaxHandler::new(contribution, language, query)
            .unwrap_or_else(|error| panic!("{label}: query compiles: {error}"));
        let (start, old_end, new_end) = single_edit_offsets(old_text, new_text);

        handler
            .parse_sync(parse_notification_for(package_prefix, 1, old_text))
            .unwrap_or_else(|error| panic!("{label}: initial parse: {error}"));
        let update = handler
            .parse_sync(incremental_notification_for(
                package_prefix,
                old_text,
                new_text,
                start,
                old_end,
                new_end,
            ))
            .unwrap_or_else(|error| panic!("{label}: authoritative parse: {error}"));
        let expected_delta = format!(
            "tree-sitter:{}:incremental",
            contribution_id
                .rsplit_once('.')
                .map_or(contribution_id, |(_, id)| id)
        );
        assert_eq!(
            update.syntax_tree_delta.as_deref(),
            Some(expected_delta.as_str()),
            "{label}: consecutive edit reuses cached tree"
        );
        let spans = update
            .decoration_updates
            .iter()
            .flat_map(|set| &set.spans)
            .collect::<Vec<_>>();

        if let Some((token_type, text)) = expected {
            let byte_start = new_text
                .find(text)
                .unwrap_or_else(|| panic!("{label}: {text}"));
            assert!(
                spans.iter().any(|span| {
                    span.token_type == token_type
                        && span.byte_start == byte_start as u64
                        && span.byte_end == (byte_start + text.len()) as u64
                }),
                "{label}: expected complete {token_type:?} capture for {text:?}, got {spans:?}"
            );
        } else {
            assert!(
                spans.iter().all(|span| {
                    span.token_type != TokenType::Keyword
                        || span.byte_start > new_end as u64
                        || span.byte_end <= start as u64
                }),
                "{label}: removed keyword remains decorated: {spans:?}"
            );
        }
    }
}

#[cfg(any(unix, windows))]
#[test]
fn first_party_package_queries_keep_broad_captures_continuous() {
    let registry = SyntaxGrammarRegistry::with_first_party_native();

    for (
        label,
        package_prefix,
        contribution_id,
        language,
        query,
        old_text,
        new_text,
        token_type,
        text,
    ) in [
        (
            "rust line comment",
            "rust",
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("../packages/rust/queries/highlights.scm"),
            "fn main() { // note\n}\n",
            "fn main() { // notes\n}\n",
            TokenType::Comment,
            "// notes",
        ),
        (
            "typescript block comment",
            "typescript",
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function main() { /* note */ }\n",
            "function main() { /* notes */ }\n",
            TokenType::Comment,
            "/* notes */",
        ),
        (
            "tsx block comment",
            "typescript",
            "typescript.tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "function main() { /* note */ }\n",
            "function main() { /* notes */ }\n",
            TokenType::Comment,
            "/* notes */",
        ),
        (
            "javascript block comment",
            "javascript",
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("../packages/javascript/queries/highlights.scm"),
            "function main() { /* note */ }\n",
            "function main() { /* notes */ }\n",
            TokenType::Comment,
            "/* notes */",
        ),
        (
            "rust raw multiline string",
            "rust",
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("../packages/rust/queries/highlights.scm"),
            "fn main() { let note = r#\"one\ntwo\"#; }\n",
            "fn main() { let note = r#\"one\ntwos\"#; }\n",
            TokenType::String,
            "r#\"one\ntwos\"#",
        ),
        (
            "typescript multiline template string",
            "typescript",
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "const note = `one\ntwo`;\n",
            "const note = `one\ntwos`;\n",
            TokenType::String,
            "`one\ntwos`",
        ),
        (
            "tsx multiline template string",
            "typescript",
            "typescript.tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            "const note = `one\ntwo`;\n",
            "const note = `one\ntwos`;\n",
            TokenType::String,
            "`one\ntwos`",
        ),
        (
            "javascript multiline template string",
            "javascript",
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("../packages/javascript/queries/highlights.scm"),
            "const note = `one\ntwo`;\n",
            "const note = `one\ntwos`;\n",
            TokenType::String,
            "`one\ntwos`",
        ),
        (
            "markdown prose",
            "markdown",
            "markdown.markdown",
            tree_sitter_md_025::LANGUAGE.into(),
            include_str!("../packages/markdown/queries/highlights.scm"),
            "Paragraph text.\n",
            "Paragraph texts.\n",
            TokenType::Paragraph,
            "Paragraph texts.",
        ),
        (
            "markdown code span",
            "markdown",
            "markdown.markdown",
            tree_sitter_md_025::LANGUAGE.into(),
            include_str!("../packages/markdown/queries/highlights.scm"),
            "`code`\n",
            "`codes`\n",
            TokenType::CodeSpan,
            "`codes`",
        ),
        (
            "markdown code block",
            "markdown",
            "markdown.markdown",
            tree_sitter_md_025::LANGUAGE.into(),
            include_str!("../packages/markdown/queries/highlights.scm"),
            "```rust\nfn main() {}\n```\n",
            "```rust\nfn mains() {}\n```\n",
            TokenType::CodeBlock,
            "```rust\nfn mains() {}\n```",
        ),
    ] {
        let contribution = registry
            .get(contribution_id)
            .unwrap_or_else(|| panic!("{label}: contribution is registered"))
            .clone();
        let mut handler = TreeSitterSyntaxHandler::new(contribution, language, query)
            .unwrap_or_else(|error| panic!("{label}: query compiles: {error}"));
        if contribution_id == "markdown.markdown" {
            // Markdown inline forms come from the markdown_inline injection
            // layer, wired the same way `native_handler` wires it.
            handler
                .enable_injections(include_str!("../packages/markdown/queries/injections.scm"))
                .unwrap_or_else(|error| panic!("{label}: injections compile: {error}"));
        }
        let (start, old_end, new_end) = single_edit_offsets(old_text, new_text);

        handler
            .parse_sync(parse_notification_for(package_prefix, 1, old_text))
            .unwrap_or_else(|error| panic!("{label}: initial parse: {error}"));
        let update = handler
            .parse_sync(incremental_notification_for(
                package_prefix,
                old_text,
                new_text,
                start,
                old_end,
                new_end,
            ))
            .unwrap_or_else(|error| panic!("{label}: authoritative parse: {error}"));
        let byte_start = new_text
            .find(text)
            .unwrap_or_else(|| panic!("{label}: {text}"));

        assert!(
            update
                .decoration_updates
                .iter()
                .flat_map(|set| &set.spans)
                .any(|span| {
                    span.token_type == token_type
                        && span.byte_start == byte_start as u64
                        && span.byte_end >= (byte_start + text.len()) as u64
                }),
            "{label}: expected complete {token_type:?} capture for {text:?}, got {:?}",
            update.decoration_updates
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn changed_decoration_chunk_does_not_publish_untouched_adjacent_chunks() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), "(line_comment) @comment")
            .expect("query compiles");
    let old_text = format!("// {}\n", "a".repeat(700));
    let insertion = 400;
    let mut new_text = old_text.clone();
    new_text.insert(insertion, 'b');

    handler
        .parse_sync(parse_notification(1, &old_text))
        .expect("initial parse");
    let update = handler
        .parse_sync(incremental_notification(
            &old_text,
            &new_text,
            insertion,
            insertion,
            insertion + 1,
        ))
        .expect("incremental parse");

    assert_eq!(update.decoration_updates.len(), 1);
    let first = &update.decoration_updates[0];
    assert!(
        first.viewport_byte_start <= insertion as u64 && first.viewport_byte_end > insertion as u64
    );
}

#[cfg(any(unix, windows))]
#[test]
fn cached_tree_version_gap_uses_one_bounded_full_parse() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), rust_highlights_query())
            .expect("query compiles");

    handler
        .parse_sync(parse_notification(1, "fn one() {}\n"))
        .expect("initial parse");
    let update = handler
        .parse_sync(parse_notification(3, "fn three() {}\n"))
        .expect("bounded fallback parse");

    assert_eq!(
        update.syntax_tree_delta.as_deref(),
        Some("tree-sitter:rust:full")
    );
    assert_eq!(handler.cached_tree_version(11), Some(3));
}

#[cfg(any(unix, windows))]
#[test]
fn tree_sitter_handler_fails_closed_for_invalid_query() {
    let record = rust_record();
    let contribution = rust_contribution(&record);

    let invalid =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), "(not_a_node) @keyword")
            .unwrap_err();
    assert!(matches!(
        invalid,
        TreeSitterSyntaxError::QueryCompileFailed { .. }
    ));
    assert!(invalid.to_string().contains("query failed to compile"));
}

#[cfg(any(unix, windows))]
#[test]
fn unmatched_grammar_captures_stay_unstyled_without_color_leak() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler = TreeSitterSyntaxHandler::new(
        contribution,
        rust_language(),
        r#""fn" @keyword
           (identifier) @unmapped"#,
    )
    .expect("unmapped query captures are inert");

    let set = handler
        .parse_sync(parse_notification(1, "fn main() {}"))
        .expect("parse succeeds")
        .decoration_updates
        .expect("decoration update");

    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Keyword)
    );
    assert!(set.spans.iter().all(|span| {
        span.scope.is_none()
            && span.token_type == TokenType::Keyword
            && span.modifiers == Modifiers::NONE
    }));
}

#[cfg(any(unix, windows))]
#[test]
fn dense_4k_capture_pass_fans_out_complete_bounded_decoration_sets() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), "(identifier) @keyword")
            .expect("query compiles");
    let text = (0..190)
        .map(|index| format!("let value_{index} = {index};"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.len() <= 4096);

    let update = handler
        .parse_sync(parse_notification(1, &text))
        .expect("dense capture pass succeeds");

    assert!(update.decoration_updates.len() > 1);
    assert!(
        update
            .decoration_updates
            .iter()
            .map(|set| set.spans.len())
            .sum::<usize>()
            >= 190
    );
    for set in &update.decoration_updates {
        assert!(
            rkyv::to_bytes::<rkyv::rancor::Error>(set)
                .expect("serialize bounded decorations")
                .len()
                <= clay::perf::budgets::DECORATION_PAYLOAD_BUDGET_BYTES
        );
    }
    ParseCoordinator::new()
        .validate_update(&update)
        .expect("all fan-out members validate atomically");
}

#[cfg(any(unix, windows))]
#[test]
fn tree_sitter_handler_enforces_window_budget_before_parsing() {
    let record = rust_record();
    let mut contribution = rust_contribution(&record);
    contribution.max_window_bytes = Some(4);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), rust_highlights_query())
            .expect("query compiles");

    assert!(matches!(
        handler.parse_sync(parse_notification(1, "fn main() {}")),
        Err(TreeSitterSyntaxError::WindowTooLarge { bytes, budget }) if bytes > budget && budget == 4
    ));
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn tree_sitter_handler_publishes_through_parse_coordinator_and_rejects_stale_results() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), rust_highlights_query())
            .expect("query compiles");
    let coordinator = ParseCoordinator::new();
    coordinator
        .register_handler(&record, "rust", handler)
        .expect("parse handler registers");
    let text = "fn main() {\n  // greet\n}\n";
    let request = ParseScheduleRequest {
        document_id: 11,
        document_version: 1,
        behavior_version: 4,
        package_prefix: "rust".to_string(),
        mode_id: "rust".to_string(),
        viewport: ParseByteRange::new(0, text.len() as u64),
        invalidated_ranges: vec![ParseByteRange::new(0, text.len() as u64)],
        accepted_edit: None,
    };
    let window = ParseWindowSnapshot {
        document_id: 11,
        document_version: 1,
        package_prefix: "rust".to_string(),
        mode_id: "rust".to_string(),
        window_id: 0,
        byte_start: 0,
        byte_end: text.len() as u64,
        base_line: 0,
        base_column: 0,
        incremental_edit: false,
        text: text.to_string(),
    };
    coordinator
        .schedule_parse_with_windows(
            request,
            vec![window],
            Some(ParsePolicy::new(4096, 16, 4096, 5000)),
        )
        .expect("parse scheduled");

    let update = tokio::time::timeout(std::time::Duration::from_secs(1), coordinator.next_update())
        .await
        .expect("parse completes")
        .expect("update published");

    assert_eq!(update.document_version, 1);
    assert!(update.decoration_updates.is_some());
    assert_eq!(coordinator.stats().published_updates, 1);
}

fn first_party_grammar_package_record(package_dir: &str) -> (String, PackageRecord) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{manifest_dir}/packages/{package_dir}/package.json");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    let value: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("parse {path}: {error}"));
    let record = assemble_package_record(&value)
        .unwrap_or_else(|error| panic!("assemble {package_dir}: {error:?}"));
    (path, record)
}

#[test]
fn first_party_language_packages_load_with_required_assets() {
    use clay::packages::permissions::PackagePermission;

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let docs_index =
        std::fs::read_to_string(format!("{manifest_dir}/docs/index.md")).expect("read docs index");
    let package_guide = std::fs::read_to_string(format!(
        "{manifest_dir}/docs/reference/packages/creating-packages.md"
    ))
    .expect("read package guide");

    for (package_dir, expected_name, language_id, extension, command_id, provider_id, status_id) in [
        (
            "rust",
            "@clay/rust",
            "rust",
            "rs",
            "rust.toggleLineComment",
            "rust.keywords",
            "rust.status.mode",
        ),
        (
            "typescript",
            "@clay/typescript",
            "typescript",
            "ts",
            "typescript.toggleLineComment",
            "typescript.keywords",
            "typescript.status.mode",
        ),
        (
            "javascript",
            "@clay/javascript",
            "javascript",
            "js",
            "javascript.toggleLineComment",
            "javascript.keywords",
            "javascript.status.mode",
        ),
    ] {
        let (path, record) = first_party_grammar_package_record(package_dir);

        assert_eq!(record.manifest.name, expected_name);
        assert_eq!(record.manifest.clay.modes, vec![language_id.to_string()]);
        assert_eq!(
            record.manifest.clay.permissions,
            vec![
                PackagePermission::ModeRegistration,
                PackagePermission::ModeActivation,
                PackagePermission::CommandRegistration,
                PackagePermission::CompletionProvider,
                PackagePermission::ParseDocument,
                PackagePermission::RenderDecorations,
            ],
            "{expected_name} must request only first-party language package permissions"
        );

        let contributions = &record.contributions;
        assert_eq!(
            contributions.syntax_grammars.len(),
            1,
            "{expected_name} must declare one syntax grammar"
        );
        let grammar = &contributions.syntax_grammars[0];
        assert_eq!(grammar.language_id, language_id);
        assert!(grammar.extensions.contains(&extension.to_string()));
        assert_eq!(grammar.grammar_kind, "native");
        assert!(grammar.style_map.values().all(|entry| matches!(
            entry.token_type,
            TokenType::Keyword
                | TokenType::String
                | TokenType::Comment
                | TokenType::Operator
                | TokenType::Paragraph
                | TokenType::Function
                | TokenType::Type
                | TokenType::Number
        )));

        assert_eq!(contributions.commands.len(), 1);
        assert_eq!(contributions.commands[0].id, command_id);
        assert_eq!(
            contributions.completion_providers.len(),
            if package_dir == "javascript" { 1 } else { 2 }
        );
        assert!(
            contributions
                .completion_providers
                .iter()
                .any(|provider| provider.id == provider_id)
        );
        assert_eq!(contributions.ui_components.len(), 1);
        assert_eq!(contributions.ui_components[0].id, status_id);

        assert!(contributions.configuration.is_empty());
        assert!(contributions.key_routing.is_empty());
        assert!(contributions.text_transforms.is_empty());
        assert!(contributions.sdui.is_empty());
        assert!(contributions.decorations.is_empty());
        assert!(contributions.ui_panels.is_empty());
        assert!(contributions.ui_overlays.is_empty());
        assert!(contributions.theme_tokens.is_empty());
        assert!(contributions.input_contributions.is_empty());
        assert!(contributions.ui_state_scopes.is_empty());
        assert!(contributions.layout_overrides.is_empty());
        assert!(contributions.package_options.is_empty());

        for api_id in [
            "clay.syntax.serverRegisterSyntaxGrammar",
            "clay.modes.serverRegisterModePattern",
            "clay.commands.serverRegisterCommand",
            "clay.completion.serverRegisterCompletionProvider",
        ] {
            assert!(
                record
                    .api_dependencies
                    .iter()
                    .any(|dep| dep.api_id == api_id),
                "{expected_name} must depend on {api_id}"
            );
        }

        let package_root = format!("{manifest_dir}/packages/{package_dir}");
        assert!(std::path::Path::new(&format!("{package_root}/dist/load.js")).exists());
        assert!(std::path::Path::new(&format!("{package_root}/dist/index.js")).exists());
        assert!(std::path::Path::new(&format!("{package_root}/docs/index.md")).exists());
        assert!(std::path::Path::new(&format!("{package_root}/queries/highlights.scm")).exists());
        assert!(std::path::Path::new(&format!("{package_root}/grammars/README.md")).exists());
        let reference_doc = format!("docs/reference/packages/{package_dir}.md");
        assert!(std::path::Path::new(&format!("{manifest_dir}/{reference_doc}")).exists());
        assert!(docs_index.contains(&format!("reference/packages/{package_dir}.md")));
        assert!(package_guide.contains(&format!("({package_dir}.md)")));

        let load_js = std::fs::read_to_string(format!("{package_root}/dist/load.js"))
            .unwrap_or_else(|error| panic!("read load.js: {error}"));
        assert!(load_js.contains("export default"));

        let docs = std::fs::read_to_string(format!("{package_root}/docs/index.md"))
            .unwrap_or_else(|error| panic!("read docs: {error}"));
        assert!(docs.contains(&format!("loadPackage(\"{expected_name}\")")));
        assert!(docs.contains(command_id));
        assert!(docs.contains(provider_id));
        assert!(docs.contains(status_id));

        assert!(std::path::Path::new(&path).exists());
    }
}

#[test]
fn first_party_grammar_packages_do_not_add_language_specific_rust_branches() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    for relative in ["src/server/syntax.rs", "src/packages/record.rs"] {
        let source = std::fs::read_to_string(format!("{manifest_dir}/{relative}"))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        for denied in [
            "language_id == \"rust\"",
            "language_id == \"typescript\"",
            "language_id == \"javascript\"",
            "language == \"rust\"",
            "language == \"typescript\"",
            "language == \"javascript\"",
            "match language_id",
        ] {
            assert!(
                !source.contains(denied),
                "{relative} must stay generic; found language-specific branch `{denied}`"
            );
        }
    }
}

#[test]
fn syntax_grammars_init_fixture_loads_all_first_party_grammar_packages_explicitly() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture = std::fs::read_to_string(format!(
        "{manifest_dir}/tests/fixtures/configuration/syntax-grammars-init.js"
    ))
    .expect("read syntax-grammars fixture");

    for specifier in [
        "@clay/rust",
        "@clay/typescript",
        "@clay/javascript",
        "@clay/markdown",
    ] {
        assert!(
            fixture.contains(&format!("loadPackage(\"{specifier}\")")),
            "fixture must explicitly load {specifier}"
        );
    }
    // No manual primitive plumbing presented as ordinary end-user setup.
    assert!(
        !fixture.contains("serverRegisterSyntaxGrammar"),
        "fixture must not call low-level registration as end-user setup"
    );
    assert!(
        !fixture.contains("serverLoadPackage("),
        "fixture must use loadPackage, not the lower-level helper"
    );
}
