use clay::packages::{
    modes::{DocumentClassificationInput, MajorModeActivation, ModePatternKind},
    record::{PackageRecord, PackageRecordRule, assemble_package_record},
};
use clay::protocol::{ParseByteRange, ParseEditNotification, ParsePolicy, ParseWindowSnapshot};
#[cfg(any(unix, windows))]
use clay::server::{
    parse_coordinator::{ParseCoordinator, ParseScheduleRequest},
    syntax::{
        SyntaxGrammarContribution, SyntaxGrammarPatternKind, SyntaxGrammarRegistry,
        SyntaxGrammarRegistryError, TreeSitterSyntaxError, TreeSitterSyntaxHandler,
    },
};
use serde_json::{Value, json};

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
                        "keyword": "keyword.control",
                        "string": "string.quoted",
                        "comment": "comment.line",
                        "punctuation": "punctuation.definition"
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
        parse_windows: vec![ParseWindowSnapshot {
            document_id: 11,
            document_version: version,
            package_prefix: prefix.to_string(),
            mode_id: prefix.to_string(),
            byte_start: 0,
            byte_end: text.len() as u64,
            base_line: 0,
            text: text.to_string(),
        }],
        memory_budget: None,
    }
}

#[cfg(any(unix, windows))]
fn parse_notification(version: u64, text: &str) -> ParseEditNotification {
    parse_notification_for("rust", version, text)
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
    assert_eq!(grammar.style_map["keyword"], "keyword.control");
    assert_eq!(grammar.timeout_ms, Some(5000));
    assert_eq!(grammar.max_window_bytes, Some(4096));
    assert!(grammar.estimated_payload_bytes > 0);
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
fn syntax_grammar_rejects_unknown_style_tokens_and_raw_css() {
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
            let set = update.decoration_update.unwrap_or_else(|| {
                panic!("{package_dir} fixture publishes v{version} decorations")
            });
            assert_eq!(set.document_version, version);
            assert!(
                set.spans
                    .iter()
                    .any(|span| span.style_token == "keyword.control"),
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
            .decoration_update
            .unwrap_or_else(|| panic!("{package_dir} fixture publishes decorations"));

        assert_eq!(set.document_version, 1);
        assert!(
            set.spans
                .iter()
                .any(|span| span.style_token == "keyword.control"),
            "{package_dir} fixture should highlight a keyword"
        );
        assert!(
            set.spans
                .iter()
                .any(|span| span.style_token == "string.quoted"),
            "{package_dir} fixture should highlight a string"
        );
        assert!(
            set.spans
                .iter()
                .any(|span| span.style_token == "comment.line"),
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
    let set = update.decoration_update.expect("syntax decorations");

    assert_eq!(
        update.syntax_tree_delta.as_deref(),
        Some("tree-sitter:rust:full")
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.style_token == "keyword.control")
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.style_token == "string.quoted")
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.style_token == "comment.line")
    );
    assert!(
        set.spans
            .iter()
            .any(|span| span.style_token == "punctuation.definition")
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
fn tree_sitter_handler_reuses_cached_tree_for_later_document_versions() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), rust_highlights_query())
            .expect("query compiles");

    let first = handler
        .parse_sync(parse_notification(1, "fn main() {}\n"))
        .expect("initial parse");
    let second = handler
        .parse_sync(parse_notification(2, "fn main() {\n  // changed\n}\n"))
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
fn tree_sitter_handler_fails_closed_for_invalid_query_or_unmapped_capture() {
    let record = rust_record();
    let contribution = rust_contribution(&record);

    let invalid = TreeSitterSyntaxHandler::new(
        contribution.clone(),
        rust_language(),
        "(not_a_node) @keyword",
    )
    .unwrap_err();
    assert!(matches!(
        invalid,
        TreeSitterSyntaxError::QueryCompileFailed { .. }
    ));
    assert!(invalid.to_string().contains("query failed to compile"));

    let unmapped = TreeSitterSyntaxHandler::new(contribution, rust_language(), "\"fn\" @function")
        .unwrap_err();
    assert!(matches!(
        unmapped,
        TreeSitterSyntaxError::QueryCaptureNotMapped { ref capture } if capture == "function"
    ));
    assert!(unmapped.to_string().contains("@function"));
    assert!(unmapped.to_string().contains("known Clay style token"));
}

#[cfg(any(unix, windows))]
#[test]
fn tree_sitter_handler_rejects_capture_output_over_viewport_limit() {
    let record = rust_record();
    let contribution = rust_contribution(&record);
    let handler =
        TreeSitterSyntaxHandler::new(contribution, rust_language(), "(identifier) @keyword")
            .expect("query compiles");
    let text = (0..140)
        .map(|index| format!("let value_{index} = {index};"))
        .collect::<Vec<_>>()
        .join("\n");

    let error = handler
        .parse_sync(parse_notification(1, &text))
        .unwrap_err();

    assert!(matches!(
        error,
        TreeSitterSyntaxError::CaptureLimitExceeded { limit: 128 }
    ));
    assert!(error.to_string().contains("more than 128 captures"));
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
    };
    let window = ParseWindowSnapshot {
        document_id: 11,
        document_version: 1,
        package_prefix: "rust".to_string(),
        mode_id: "rust".to_string(),
        byte_start: 0,
        byte_end: text.len() as u64,
        base_line: 0,
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
    assert!(update.decoration_update.is_some());
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
        assert_eq!(grammar.grammar_kind, "tree-sitter-wasm");
        assert!(grammar.style_map.values().all(|token| matches!(
            token.as_str(),
            "keyword.control"
                | "string.quoted"
                | "comment.line"
                | "punctuation.definition"
                | "text"
        )));

        assert_eq!(contributions.commands.len(), 1);
        assert_eq!(contributions.commands[0].id, command_id);
        assert_eq!(contributions.completion_providers.len(), 1);
        assert_eq!(contributions.completion_providers[0].id, provider_id);
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

    for specifier in ["@clay/rust", "@clay/typescript", "@clay/javascript"] {
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
