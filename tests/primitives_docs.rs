use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use clay::perf::budgets::{
    COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES, DECORATION_PAYLOAD_BUDGET_BYTES,
    FOLDING_RANGE_PAYLOAD_BUDGET_BYTES, INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES,
    KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS, MODE_ACTIVATION_P95_BUDGET_MS,
    PRIMITIVES_REGISTRY_VERSION, SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES,
    SDUI_UPDATE_PAYLOAD_BUDGET_BYTES, SYNTAX_CACHE_BUDGET_BYTES,
};

fn repository_path(relative: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn api_inventory_text() -> String {
    fs::read_to_string(repository_path(
        "docs/reference/clay-js-api/api-inventory.toml",
    ))
    .expect("read api inventory")
}

fn api_inventory_ids() -> BTreeSet<String> {
    let text = api_inventory_text();
    text.lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("id = \"")
                .and_then(|rest| rest.strip_suffix('"'))
                .map(ToOwned::to_owned)
        })
        .collect()
}

fn api_inventory_entry_block<'a>(inventory: &'a str, id: &str) -> &'a str {
    let marker = format!("id = \"{id}\"");
    let start = inventory
        .find(&marker)
        .unwrap_or_else(|| panic!("missing API inventory entry {id}"));
    let block_start = inventory[..start]
        .rfind("[[api]]")
        .unwrap_or_else(|| panic!("missing [[api]] marker before {id}"));
    let block_end = inventory[start..]
        .find("\n[[api]]")
        .map(|offset| start + offset)
        .unwrap_or(inventory.len());
    &inventory[block_start..block_end]
}

fn backtick_clay_api_ids(text: &str) -> BTreeSet<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .filter(|token| token.starts_with("clay."))
        .map(ToOwned::to_owned)
        .collect()
}

fn primitives_registry() -> String {
    fs::read_to_string(repository_path("docs/reference/primitives/registry.md"))
        .expect("read primitive registry")
}

fn rendering_strategy() -> String {
    fs::read_to_string(repository_path(
        "docs/reference/primitives/rendering-strategy.md",
    ))
    .expect("read rendering strategy")
}

fn parse_update_strategy() -> String {
    fs::read_to_string(repository_path(
        "docs/reference/primitives/parse-update-strategy.md",
    ))
    .expect("read parse update strategy")
}

fn markdown_mode_requirements() -> String {
    fs::read_to_string(repository_path(
        "docs/reference/primitives/markdown-mode-requirements.md",
    ))
    .expect("read markdown mode requirements")
}

fn syntax_vocabulary_contract() -> String {
    fs::read_to_string(repository_path(
        "docs/reference/primitives/syntax-vocabulary.md",
    ))
    .expect("read syntax vocabulary contract")
}

fn typography_contract() -> String {
    fs::read_to_string(repository_path("docs/reference/primitives/typography.md"))
        .expect("read semantic typography contract")
}

fn diagnostics_contract() -> String {
    fs::read_to_string(repository_path("docs/reference/primitives/diagnostics.md"))
        .expect("read range diagnostics contract")
}

fn package_security() -> String {
    fs::read_to_string(repository_path(
        "docs/reference/primitives/package-security.md",
    ))
    .expect("read package security")
}

fn shell_layout_strategy() -> String {
    fs::read_to_string(repository_path(
        "docs/reference/primitives/shell-layout-strategy.md",
    ))
    .expect("read shell layout strategy")
}

fn creating_packages_guide() -> String {
    fs::read_to_string(repository_path(
        "docs/reference/packages/creating-packages.md",
    ))
    .expect("read package authoring guide")
}

fn implementation_gate() -> String {
    fs::read_to_string(repository_path(
        "docs/reference/primitives/implementation-gate.md",
    ))
    .expect("read primitive implementation gate")
}

fn primitives_index() -> String {
    fs::read_to_string(repository_path("docs/reference/primitives/index.md"))
        .expect("read primitives index")
}

fn primitives_backlog() -> String {
    fs::read_to_string(repository_path("docs/reference/primitives/backlog.md"))
        .expect("read primitives backlog")
}

#[test]
fn primitives_audit_doc_linked_from_index() {
    let index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");

    assert!(
        index.contains("reference/primitives/audit.md"),
        "docs/index.md must link docs/reference/primitives/audit.md"
    );
}

#[test]
fn primitives_audit_cites_valid_api_ids() {
    let audit = fs::read_to_string(repository_path("docs/reference/primitives/audit.md"))
        .expect("read primitives audit");
    let api_ids = api_inventory_ids();
    let referenced = backtick_clay_api_ids(&audit);

    assert!(
        !referenced.is_empty(),
        "primitives audit should cite current Clay JS API IDs"
    );

    let missing = referenced.difference(&api_ids).cloned().collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "primitives audit references API IDs missing from api-inventory.toml: {missing:?}"
    );
}

#[test]
fn primitives_registry_linked_from_index() {
    let index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");

    assert!(
        index.contains("reference/primitives/registry.md"),
        "docs/index.md must link docs/reference/primitives/registry.md"
    );
}

#[test]
fn primitives_budget_constants_compile() {
    assert_eq!(DECORATION_PAYLOAD_BUDGET_BYTES, 8192);
    assert_eq!(INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES, 4096);
    assert_eq!(SYNTAX_CACHE_BUDGET_BYTES, 30 * 1024 * 1024);
    assert_eq!(MODE_ACTIVATION_P95_BUDGET_MS, 100);
    assert_eq!(COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES, 16 * 1024);
    assert_eq!(FOLDING_RANGE_PAYLOAD_BUDGET_BYTES, 2048);
    assert_eq!(PRIMITIVES_REGISTRY_VERSION, "phase16-primitives-v1");
}

#[test]
fn primitives_registry_categories_cover_required_list() {
    let registry = primitives_registry();

    for required in [
        "DocumentClassification",
        "MajorModeActivation",
        "MinorModeActivation",
        "KeyRoutingOverride",
        "TextTransform",
        "IncrementalParseUpdate",
        "DecorationRange",
        "SyntaxGrammarContribution",
        "FoldingRange",
        "CompletionTriggerAndResult",
        "CommandDeclaration",
        "SduiPanelStatusContribution",
        "PackageOwnedConfiguration",
        "PackagePermissionDeclaration",
    ] {
        assert!(
            registry.contains(required),
            "primitive registry must contain required category {required}"
        );
    }

    for required_field in [
        "owner",
        "authority",
        "hot_path_policy",
        "js_module",
        "js_export",
        "stable_id",
        "user_facing_name",
        "permissions",
        "budget_ref",
        "primitive_kind",
        "documentation_metadata",
        "test_expectations",
    ] {
        assert!(
            registry.contains(required_field),
            "primitive registry schema must define field {required_field}"
        );
    }
}

#[test]
fn rendering_strategy_doc_linked_from_index() {
    let index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");

    assert!(
        index.contains("reference/primitives/rendering-strategy.md"),
        "docs/index.md must link docs/reference/primitives/rendering-strategy.md"
    );
}

#[test]
fn decoration_budget_constant_exists() {
    assert_eq!(DECORATION_PAYLOAD_BUDGET_BYTES, 8192);
}

#[test]
fn rendering_strategy_covers_inert_client_rendering_contract() {
    let strategy = rendering_strategy();

    for required in [
        "DecorationUpdate",
        "DecorationSpan",
        "byte_start",
        "byte_end",
        "kind",
        "style_token",
        "priority",
        "LayoutHint",
        "block_or_inline",
        "margin_class",
        "emphasis_level",
        "render_intent_version",
        "viewport_byte_start",
        "viewport_byte_end",
        "server validates",
        "client receives only bounded, inert declarations",
        "src/masonry_sdui.rs",
        "src/masonry_editor.rs",
        "src/editor/surface.rs",
        "Parley",
        "Vello",
        "No package JavaScript runs in client paint",
        "Inject arbitrary GPU draw calls",
        "Mutate Masonry widgets directly",
    ] {
        assert!(
            strategy.contains(required),
            "rendering strategy must document required contract text: {required}"
        );
    }
}

#[test]
fn rendering_strategy_references_payload_budgets() {
    let strategy = rendering_strategy();

    assert_eq!(SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES, 4096);
    assert_eq!(SDUI_UPDATE_PAYLOAD_BUDGET_BYTES, 1024);
    for required in [
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES",
        "SDUI_UPDATE_PAYLOAD_BUDGET_BYTES",
        "viewport-prioritized",
    ] {
        assert!(
            strategy.contains(required),
            "rendering strategy must reference budget/update concept {required}"
        );
    }
}

#[test]
fn parse_strategy_doc_linked_from_index() {
    let index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");

    assert!(
        index.contains("reference/primitives/parse-update-strategy.md"),
        "docs/index.md must link docs/reference/primitives/parse-update-strategy.md"
    );
}

#[test]
fn incremental_parse_budget_constant_exists() {
    assert_eq!(INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES, 4096);
}

#[test]
fn parse_strategy_covers_background_update_contract() {
    let strategy = parse_update_strategy();

    assert_eq!(KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS, 16);
    for required in [
        "file-level",
        "region-level",
        "line-group-level",
        "ParseEditNotification",
        "spawn",
        "cancel",
        "timeout",
        "result publication",
        "viewport-prioritized",
        "no-decoration-update",
        "server validates",
        "INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES",
        "KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS",
        "Background",
        "ClientFirstPredictable",
        "src/server/document.rs",
        "src/server/js_runtime.rs",
        "src/server/parse_coordinator.rs",
        "deno_core",
        "DecorationUpdate",
        "filesystem",
        "network",
        "shell",
        "AI",
    ] {
        assert!(
            strategy.contains(required),
            "parse strategy must document required contract text: {required}"
        );
    }
}

#[test]
fn markdown_mode_requirements_doc_linked_from_index() {
    let index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");

    assert!(
        index.contains("reference/primitives/markdown-mode-requirements.md"),
        "docs/index.md must link docs/reference/primitives/markdown-mode-requirements.md"
    );
}

#[test]
fn syntax_vocabulary_contract_doc_linked_from_index() {
    let index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");

    assert!(
        index.contains("reference/primitives/syntax-vocabulary.md"),
        "docs/index.md must link docs/reference/primitives/syntax-vocabulary.md"
    );

    let primitives_index =
        fs::read_to_string(repository_path("docs/reference/primitives/index.md"))
            .expect("read primitives index");
    assert!(
        primitives_index.contains("syntax-vocabulary.md"),
        "docs/reference/primitives/index.md must link syntax-vocabulary.md"
    );
}

#[test]
fn syntax_vocabulary_contract_locks_two_axis_vocabulary() {
    let contract = syntax_vocabulary_contract();

    // Decision + status provenance.
    assert!(
        contract.contains("Locked contract"),
        "syntax vocabulary contract must declare its locked-contract status"
    );
    assert!(
        contract.contains("2026-07-09-0352-tiered-tree-sitter"),
        "syntax vocabulary contract must cite decision log 2026-07-09-0352"
    );

    // LSP base vocabulary (axis 1): the closed enum is LSP-derived, not invented.
    for required in [
        "Language Server Protocol `SemanticTokenType`",
        "Namespace, Type, Class, Enum, Interface, Struct, TypeParameter, Parameter, Variable, Property, EnumMember, Event, Function, Method, Macro, Keyword, Modifier, Comment, String, Number, Regexp, Operator, Decorator",
    ] {
        assert!(
            contract.contains(required),
            "syntax vocabulary contract must lock the LSP base TokenType set: {required}"
        );
    }

    // Clay prose extension (axis 1) explicitly separated as Clay-owned.
    for required in [
        "Clay prose extension",
        "Heading1, Heading2, Heading3, Heading4, Heading5, Heading6, ListItem, Quote, CodeBlock, CodeSpan, Link, Paragraph",
    ] {
        assert!(
            contract.contains(required),
            "syntax vocabulary contract must lock the Clay prose TokenType extension: {required}"
        );
    }

    // LSP base modifiers (axis 2).
    assert!(
        contract.contains("Declaration, Definition, Readonly, Static, Deprecated, Abstract, Async, Modification, Documentation, DefaultLibrary"),
        "syntax vocabulary contract must lock the LSP base Modifiers bitflag set"
    );

    // Clay text-attribute modifiers (axis 2).
    assert!(
        contract.contains("Bold, Italic, Underline, Strikethrough"),
        "syntax vocabulary contract must lock the Clay text-attribute Modifiers extension"
    );

    // Two-axis composition example + open-string escape.
    assert!(
        contract.contains("TokenType::Function + Modifiers::Bold | Declaration"),
        "syntax vocabulary contract must demonstrate two-axis composition"
    );
    assert!(
        contract.contains("longest-prefix fallback"),
        "syntax vocabulary contract must lock the open-string scope escape rule"
    );

    // Compatibility mapping + baseline lock cross-reference.
    assert!(
        contract.contains("free_form_style_token_decoration_colors_baseline_locked"),
        "syntax vocabulary contract must reference the baseline-color lock test"
    );

    // Inert-data security boundary + hot-path performance rule.
    assert!(
        contract
            .contains("No code, widgets, ops, raw CSS, filesystem, network, or shell authority"),
        "syntax vocabulary contract must lock the inert-data security boundary"
    );
    assert!(
        contract.contains("no per-glyph map lookup"),
        "syntax vocabulary contract must lock the hot-path performance rule"
    );
}

#[test]
fn phase18_15_theme_authoring_docs_lock_textstyles_contract() {
    let contract = syntax_vocabulary_contract();
    let guide = creating_packages_guide();
    let index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");

    for required in [
        "`StyleSpec` (`color`, `bold`, `italic`, `underline`, `strike`)",
        "`clay.contributions.textStyles`",
        "`shellBg`, `panelBg`, `text`, `placeholder`, `selection`, `caret`, `scrollbar`, `scrollbarTrack`, `statusBg`, `statusText`, `diagnosticError`, `diagnosticWarning`, `diagnosticInfo`",
        "Theme resolution happens at configuration/package-load time",
        "No code, widgets, ops, raw CSS, filesystem, network, or shell authority",
    ] {
        assert!(
            contract.contains(required),
            "syntax vocabulary contract must document theme binding detail: {required}"
        );
    }

    for required in [
        "Phase 18.15 theme authoring: `textStyles` and `setTheme`",
        "Text Vocabulary and Two-Axis Decoration Contract",
        "`textStyles` entry fields",
        "Base UI keys are: `shellBg`, `panelBg`, `text`, `placeholder`, `selection`, `caret`, `scrollbar`, `scrollbarTrack`, `statusBg`, `statusText`, `diagnosticError`, `diagnosticWarning`, `diagnosticInfo`",
        "Only one active theme is applied",
        "setTheme(\"@clay/theme-gruvbox-material-dark\")",
        "rawColor`, `value`, `css`, `rawCss`, `cssText`",
        "No theme JavaScript, package parser, or raw IPC runs in paint",
        "packages/theme-gruvbox-material-dark/",
        "packages/theme-gruvbox-material-light/",
    ] {
        assert!(
            guide.contains(required),
            "package guide must document Phase 18.15 theme authoring detail: {required}"
        );
    }

    assert!(
        index.contains("reference/primitives/syntax-vocabulary.md"),
        "docs/index.md must link syntax vocabulary docs"
    );
}

fn text_vocabulary_theme_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/text-vocabulary-and-theme-primitive-review.md",
    ))
    .expect("read text vocabulary and theme primitive review")
}

#[test]
fn text_vocabulary_theme_primitive_review_linked_from_wiki_index() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    assert!(
        wiki_index.contains("modules/text-vocabulary-and-theme-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.15 text vocabulary and theme primitive review"
    );
}

#[test]
fn text_vocabulary_theme_primitive_review_inventories_and_gaps() {
    let review = text_vocabulary_theme_primitive_review();

    // Source provenance.
    assert!(
        review.contains("046-Phase-18.15"),
        "primitive review must cite Plan 046"
    );
    assert!(
        review.contains("2026-07-09-0352-tiered-tree-sitter"),
        "primitive review must cite decision log 2026-07-09-0352"
    );

    // Existing primitive inventory (every primitive the refactor must reuse or extend).
    for required in [
        "DecorationSpan",
        "DecorationKind { Syntax, Semantic, Diagnostic, SearchMatch }",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "visible_decoration_ranges",
        "ThemeTokenType { ColorRole, Spacing, Radius, Typography, Opacity }",
        "ThemeTokenContributionDescriptor",
        "reject_ui_prohibited_authority",
        "op_clay_ui_register_theme_token",
        "serverRegisterThemeToken",
        "decorations_for_window",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.15 primitive review must inventory existing primitive `{required}`"
        );
    }

    // Generic gaps the phase introduces.
    for required in [
        "Two-axis `TokenType` + `Modifiers` vocabulary",
        "`StyleSpec` and `StyleRegistry`",
        "Text-style theme contribution and active-theme application",
        "Free-form `style_token` compatibility mapper",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.15 primitive review must record generic gap: {required}"
        );
    }

    // Reuse statement + rejected non-generic approaches.
    assert!(
        review.contains("What the Refactor Achieves with Existing Primitives"),
        "primitive review must state what the refactor reuses"
    );
    for rejected in [
        "Per-language Rust branches",
        "Relaxing `StaticSduiState` or decoration validation",
        "Client-side JavaScript or raw CSS for styling",
        "Overloading the SDUI `ThemeTokenType` typed-scalar resolver",
        "Hidden JSON/TOML configuration keys for themes",
    ] {
        assert!(
            review.contains(rejected),
            "Phase 18.15 primitive review must reject non-generic approach: {rejected}"
        );
    }

    // Hot-path + security boundaries.
    assert!(
        review.contains("no per-glyph lookup"),
        "primitive review must lock the StyleRegistry hot-path rule"
    );
    assert!(
        review.contains("inert data only"),
        "primitive review must lock the inert-data security boundary"
    );
    assert!(
        review.contains("denied by default"),
        "primitive review must lock deny-by-default theme authority"
    );
}

#[test]
fn editor_theme_registry_wiki_documents_phase18_15_implementation() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let page = fs::read_to_string(repository_path(
        "docs/wiki/modules/editor-theme-registry.md",
    ))
    .expect("read editor theme registry wiki");
    let editor_page = fs::read_to_string(repository_path("docs/wiki/modules/masonry-editor.md"))
        .expect("read masonry editor wiki");
    let sdui_page = fs::read_to_string(repository_path("docs/wiki/modules/server-driven-ui.md"))
        .expect("read server-driven-ui wiki");
    let review = text_vocabulary_theme_primitive_review();

    assert!(
        wiki_index.contains("modules/editor-theme-registry.md"),
        "wiki index must link the Phase 18.15 editor theme registry implementation page"
    );

    for required in [
        "src/editor/theme.rs",
        "StyleRegistry",
        "[Color; 35]",
        "TokenType::index()",
        "StyleSpec { color, bold, italic, underline, strike }",
        "VisibleTextStyleRun",
        "Diagnostics and search may still paint their rectangles and attributes but cannot select a font role.",
        "clay.contributions.textStyles",
        "TextStyleOverrideDescriptor",
        "reject_ui_prohibited_authority",
        "op_clay_theme_set_theme",
        "ServerMessage::ActiveTheme",
        "@clay/theme-gruvbox-material-dark",
        "@clay/theme-gruvbox-material-light",
        "no per-language Rust color branches",
        "no package JavaScript",
        "non-`@clay/*` specifiers are denied",
        "cargo test --test theme_packages",
    ] {
        assert!(
            page.contains(required),
            "editor theme registry wiki must document `{required}`"
        );
    }

    assert!(
        editor_page.contains("StyleRegistry::from_active_theme"),
        "masonry editor wiki must document ActiveTheme-to-StyleRegistry application"
    );
    assert!(
        sdui_page.contains("SDUI theme tokens and editor text themes are separate primitives"),
        "SDUI wiki must document separation from editor StyleRegistry"
    );
    assert!(
        review.contains("Editor Theme Registry"),
        "primitive review must link final implementation wiki"
    );

    let decoration_page =
        fs::read_to_string(repository_path("docs/wiki/modules/decoration-transport.md"))
            .expect("read decoration transport wiki");
    assert!(
        decoration_page.contains("only syntax/semantic roles survive normalization"),
        "decoration transport wiki must document client-side fail-closed font-role normalization"
    );
}

#[test]
fn markdown_mode_prerequisites_reference_registry_entries() {
    let registry = primitives_registry();
    let requirements = markdown_mode_requirements();

    for registry_entry in [
        "DocumentClassification",
        "MajorModeActivation",
        "TextTransform",
        "DecorationRange",
        "IncrementalParseUpdate",
        "CommandDeclaration",
        "KeyRoutingOverride",
        "SduiPanelStatusContribution",
        "FoldingRange",
        "CompletionTriggerAndResult",
        "MinorModeActivation",
        "PackagePermissionDeclaration",
    ] {
        assert!(
            registry.contains(registry_entry),
            "primitive registry must contain Markdown prerequisite entry {registry_entry}"
        );
        assert!(
            requirements.contains(&format!("`{registry_entry}`")),
            "markdown requirements must reference registry entry {registry_entry} in a primitive row"
        );
    }
}

#[test]
fn markdown_mode_requirements_cover_phase18_poc_contract() {
    let requirements = markdown_mode_requirements();

    for required in [
        "@clay/markdown",
        "markdown",
        ".md",
        ".markdown",
        ".mdown",
        "text/markdown",
        "serverRegisterModePattern",
        "serverActivateMajorMode",
        "markdown_list_continuation",
        "markdown_fenced_code_indent",
        "markdown.heading",
        "markdown.strong",
        "markdown.emphasis",
        "markdown.code_span",
        "markdown.code_block",
        "markdown.list_marker",
        "markdown.togglePreview",
        "markdown.insertHeading",
        "markdown.toggleList",
        "definePanel",
        "MODE_ACTIVATION_P95_BUDGET_MS",
        "INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS",
        "KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS",
        "BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES",
        "filesystem",
        "network",
        "shell",
        "AI mutation",
        "raw `Deno.core.ops`",
        "client-side JavaScript",
    ] {
        assert!(
            requirements.contains(required),
            "markdown requirements must document required POC contract text: {required}"
        );
    }
}

#[test]
fn package_security_doc_linked_from_index() {
    let index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");

    assert!(
        index.contains("reference/primitives/package-security.md"),
        "docs/index.md must link docs/reference/primitives/package-security.md"
    );
}

#[test]
fn package_security_doc_references_decision_sources() {
    let security = package_security();

    for required in [
        "package-distribution.md",
        "extensions-and-ai.md",
        "clay-js-api-boundary.md",
        "decision-logs/2026-05-08-1958-clay-js-api-naming-and-package-distribution.md",
        "decision-logs/2026-05-08-0408-server-authoritative-documents-client-behavior-manifests.md",
        "decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md",
    ] {
        assert!(
            security.contains(required),
            "package security doc must reference decision source or pattern: {required}"
        );
    }
}

#[test]
fn package_security_doc_covers_validation_conflicts_and_prohibitions() {
    let security = package_security();

    for required in [
        "apiPrefix",
        "^[a-z][a-z0-9-]{1,31}$",
        "package prefix",
        "permissions",
        "schema",
        "payload size",
        "load-time errors",
        "not on every edit",
        "must not appear on the typing hot path",
        "raw `Deno.core.ops`",
        "client-side JavaScript",
        "Duplicate mode name",
        "Same key binding",
        "Decoration range overlap",
        "Unified Package Trust and Authorization Policy",
        "Package source (`@clay/*`, npm, GitHub, git URL, tarball, or local path)",
        "source does not create a permanent first-party/third-party capability ceiling",
        "Capability grants are explicit, visible, revocable",
        "filesystem",
        "network",
        "shell",
        "ai-tools",
        "workspace-mutation",
        "Powerful capabilities are allowed by user choice",
        "direct Masonry mutation",
    ] {
        assert!(
            security.contains(required),
            "package security doc must cover required security/provenance text: {required}"
        );
    }
}

#[test]
fn primitives_index_linked_from_docs_index() {
    let docs_index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");
    let primitive_index = primitives_index();

    assert!(
        docs_index.contains("reference/primitives/index.md"),
        "docs/index.md must link docs/reference/primitives/index.md"
    );

    for required in [
        "audit.md",
        "registry.md",
        "rendering-strategy.md",
        "parse-update-strategy.md",
        "markdown-mode-requirements.md",
        "package-security.md",
        "implementation-gate.md",
        "backlog.md",
        "Phase 17 Readiness Summary",
    ] {
        assert!(
            primitive_index.contains(required),
            "primitives index must link or describe {required}"
        );
    }
}

#[test]
fn primitive_implementation_gate_doc_linked_from_indexes() {
    let docs_index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");
    let primitive_index = primitives_index();
    let backlog = primitives_backlog();

    assert!(
        docs_index.contains("reference/primitives/implementation-gate.md"),
        "docs/index.md must link docs/reference/primitives/implementation-gate.md"
    );
    assert!(
        primitive_index.contains("implementation-gate.md"),
        "docs/reference/primitives/index.md must link implementation-gate.md"
    );
    assert!(
        backlog.contains("implementation-gate.md"),
        "primitive backlog must hand off to implementation-gate.md"
    );
}

#[test]
fn primitive_gate_doc_covers_scope_security_and_handoff() {
    let gate = implementation_gate();

    for required in [
        "Phase 16.5",
        "package installation",
        "package-manager integration remain out of scope",
        "Supported Fixture Format",
        "apiPrefix",
        "permissions",
        "entry",
        "loadEntry",
        "Validation Failures",
        "invalid `apiPrefix`",
        "duplicate package prefixes",
        "reserved `clay.*`",
        "unknown permissions",
        "prohibited authorities",
        "permission validation",
        "conflict handling",
        "raw `Deno.core.ops`",
        "client-side JavaScript",
        "duplicate mode names",
        "duplicate command IDs",
        "ambiguous package key bindings",
        "package-security.md",
        "Phase 17 Handoff",
        "Phase 18 Handoff",
        "DecorationRange",
        "IncrementalParseUpdate",
    ] {
        assert!(
            gate.contains(required),
            "primitive implementation gate doc must cover required text: {required}"
        );
    }
}

#[test]
fn primitive_gate_doc_keeps_validation_out_of_typing_hot_path() {
    let gate = implementation_gate();

    for required in [
        "Package validation and mode activation are load/open/reload/configuration-time operations",
        "must not be documented or wired as ordinary typing",
        "Manifest and permission validation run at package fixture/load time",
        "Document classification and major-mode activation run when a document is opened, reloaded, or explicitly reclassified",
        "KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS",
        "MODE_ACTIVATION_P95_BUDGET_MS",
    ] {
        assert!(
            gate.contains(required),
            "primitive implementation gate doc must keep hot-path policy explicit: {required}"
        );
    }
}

#[test]
fn primitive_gate_tests_run_with_cargo_test() {
    let gate = implementation_gate();
    let test_source = fs::read_to_string(repository_path("tests/package_primitive_gate.rs"))
        .expect("read package primitive gate tests");

    for required_command in [
        "cargo test --test package_primitive_gate",
        "cargo test --test primitives_docs",
    ] {
        assert!(
            gate.contains(required_command),
            "implementation gate doc must document verification command: {required_command}"
        );
    }

    for required_test in [
        "package_manifest_accepts_minimal_markdown_fixture",
        "package_manifest_rejects_duplicate_prefixes_raw_ops_and_client_hooks",
        "mode_registry_classifies_markdown_extension",
        "mode_activation_keeps_one_major_mode_per_document",
        "package_command_registry_rejects_duplicate_command_id",
        "package_keybindings_reject_ambiguous_bindings",
        "package_text_transforms_are_inert_manifest_data",
    ] {
        assert!(
            test_source.contains(required_test),
            "package primitive gate coverage must include {required_test}"
        );
    }
}

#[test]
fn primitives_backlog_entries_trace_to_registry() {
    let registry = primitives_registry();
    let backlog = primitives_backlog();

    for primitive in [
        "DocumentClassification",
        "MajorModeActivation",
        "CommandDeclaration",
        "PackagePermissionDeclaration",
        "KeyRoutingOverride",
        "TextTransform",
        "SduiPanelStatusContribution",
        "PackageOwnedConfiguration",
        "DecorationRange",
        "IncrementalParseUpdate",
        "FoldingRange",
        "MinorModeActivation",
        "CompletionTriggerAndResult",
    ] {
        assert!(
            backlog.contains(primitive),
            "primitives backlog must contain primitive entry {primitive}"
        );
        assert!(
            registry.contains(primitive),
            "primitive backlog entry {primitive} must trace to registry.md"
        );
    }

    for required_trace in [
        "registry.md",
        "audit.md",
        "rendering-strategy.md",
        "parse-update-strategy.md",
        "markdown-mode-requirements.md",
        "package-security.md",
    ] {
        assert!(
            backlog.contains(required_trace),
            "primitive backlog must cite Phase 16 analysis document {required_trace}"
        );
    }
}

#[test]
fn primitives_phase17_required_entries_exist() {
    let backlog = primitives_backlog();

    for required in [
        "Phase-17-required",
        "MajorModeActivation",
        "DocumentClassification",
        "CommandDeclaration",
        "clay.modes.serverActivateMajorMode",
        "clay.modes.serverRegisterModePattern",
        "clay.commands.serverRegisterCommand",
        "Phase 17 Prerequisite Checklist",
        "apiPrefix",
        "duplicate mode names",
        "ambiguous keybinding conflicts",
        "DecorationRange",
        "IncrementalParseUpdate",
    ] {
        assert!(
            backlog.contains(required),
            "primitive backlog must document Phase 17 prerequisite content: {required}"
        );
    }
}

#[test]
fn primitives_backlog_permission_bearing_entries_include_security_notes() {
    let backlog = primitives_backlog();

    for permission in [
        "mode-registration",
        "mode-activation",
        "command-registration",
        "parse-document",
        "render-decorations",
        "render-folding",
        "completion-provider",
        "package-configuration",
        "server-validated",
    ] {
        assert!(
            backlog.contains(permission),
            "permission-bearing backlog entries must include security note for {permission}"
        );
    }
}

#[test]
fn phase16_configuration_api_stubs_cover_reviewed_surfaces() {
    let inventory = api_inventory_text();
    let registry = primitives_registry();
    let backlog = primitives_backlog();

    let config_stubs = vec![
        (
            "clay.configuration.setPackageOption",
            "setPackageOption",
            "package-configuration",
            vec!["packagePrefix", "option", "value", "source"],
        ),
        (
            "clay.configuration.setModePreference",
            "setModePreference",
            "mode activation defaults",
            vec!["modeId", "defaultActivation", "source"],
        ),
        (
            "clay.configuration.setDecorationTheme",
            "setDecorationTheme",
            "decoration style preferences",
            vec!["theme", "styleTokens", "contrastMode", "source"],
        ),
        (
            "clay.configuration.setParsePolicy",
            "setParsePolicy",
            "parse timeout",
            vec![
                "timeoutMs",
                "maxTimeoutMs",
                "parseUnits",
                "viewportPriority",
                "source",
            ],
        ),
    ];

    for (id, js_export, surface_phrase, properties) in config_stubs {
        let block = api_inventory_entry_block(&inventory, id);
        assert!(
            registry.contains(id),
            "{id} must trace to PackageOwnedConfiguration in registry.md"
        );
        assert!(
            backlog.contains(id),
            "{id} must trace to the primitive backlog"
        );
        for required in [
            "visibility = \"public\"",
            "status = \"planned\"",
            "js_module = \"clay:configuration\"",
            "key_bindings = []",
            "custom_properties = [",
            "registry_public = false",
            "~/.config/clay/init.js",
            "server validation",
            "does not grant filesystem",
            "network",
            "shell",
            "extension loading",
            "AI mutation",
            "workspace",
            "package",
            "WASM",
            "client-side JavaScript",
            "raw Deno ops",
            "typing",
        ] {
            assert!(block.contains(required), "{id} is missing {required}");
        }
        assert!(
            block.contains(&format!("js_export = \"{js_export}\"")),
            "{id} must keep the planned JS export {js_export}"
        );
        for property in properties {
            assert!(
                block.contains(property),
                "{id} custom_properties must declare behavior-changing setting {property}"
            );
        }
        assert!(
            registry.contains(surface_phrase) || block.contains(surface_phrase),
            "{id} must document reviewed configuration surface {surface_phrase}"
        );
    }

    for required in [
        "Phase 16 configuration review",
        "package enable/disable",
        "no configuration API is added for it in this phase",
        "future enable/disable API requires an approved decision log",
        "setPackageOption",
    ] {
        assert!(
            inventory.contains(required),
            "configuration review must explicitly document package enable/disable decision: {required}"
        );
    }
}

fn phase18_markdown_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18-markdown-primitive-review.md",
    ))
    .expect("read Phase 18 Markdown primitive review")
}

fn phase18_large_file_markdown_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18-large-file-markdown-primitive-review.md",
    ))
    .expect("read Phase 18.5 large-file Markdown primitive review")
}

fn phase19_windows_file_open_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase19-windows-file-open-primitive-review.md",
    ))
    .expect("read Phase 19 Windows file-open primitive review")
}

fn phase18_1_shell_layout_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.1-shell-layout-primitive-review.md",
    ))
    .expect("read Phase 18.1 shell/layout primitive review")
}

fn phase18_2_shell_runtime_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.2-shell-runtime-primitive-review.md",
    ))
    .expect("read Phase 18.2 shell runtime primitive review")
}

fn phase18_3_slot_ui_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.3-slot-ui-primitive-review.md",
    ))
    .expect("read Phase 18.3 slot-aware package UI primitive review")
}

fn phase18_4_input_state_config_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.4-input-state-config-primitive-review.md",
    ))
    .expect("read Phase 18.4 input/state/config primitive review")
}

fn phase18_5_markdown_replan_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.5-markdown-replan-primitive-review.md",
    ))
    .expect("read Phase 18.5 Markdown replan primitive review")
}

fn phase18_8_transient_menu_command_execution_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.8-transient-menu-command-execution-primitive-review.md",
    ))
    .expect("read Phase 18.8 transient menu and command execution primitive review")
}

fn phase18_9_generic_text_code_modes_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.9-generic-text-code-modes-primitive-review.md",
    ))
    .expect("read Phase 18.9 generic text/code modes primitive review")
}

fn phase18_10_tree_sitter_grammar_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.10-tree-sitter-grammar-primitive-review.md",
    ))
    .expect("read Phase 18.10 Tree-sitter grammar primitive review")
}

fn phase18_11_completion_provider_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.11-completion-provider-primitive-review.md",
    ))
    .expect("read Phase 18.11 completion provider primitive review")
}

fn phase18_12_workspace_discovery_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.12-workspace-discovery-primitive-review.md",
    ))
    .expect("read Phase 18.12 workspace discovery and file browser foundation primitive review")
}

fn end_to_end_file_browser_workflow_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/end-to-end-file-browser-workflow-primitive-review.md",
    ))
    .expect("read end-to-end file browser workflow primitive review")
}

fn manual_file_browser_workflow_bugfix_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/manual-file-browser-workflow-bugfix-primitive-review.md",
    ))
    .expect("read manual file browser workflow bugfix primitive review")
}

fn phase18_13_git_discovery_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.13-git-discovery-primitive-review.md",
    ))
    .expect("read Phase 18.13 Git discovery service primitive review")
}

fn phase18_14_language_package_expansion_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.14-language-package-expansion-primitive-review.md",
    ))
    .expect("read Phase 18.14 first-party Rust, TypeScript, and JavaScript language package expansion primitive review")
}

fn phase18_16_tiered_engine_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.16-tiered-tree-sitter-engine-primitive-review.md",
    ))
    .expect("read Phase 18.16 tiered Tree-sitter syntax engine primitive review")
}

fn phase18_16_5_typography_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.16.5-typography-primitive-review.md",
    ))
    .expect("read Phase 18.16.5 semantic typography primitive review")
}

fn phase18_17_range_diagnostics_primitive_review() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/phase18.17-range-diagnostics-primitive-review.md",
    ))
    .expect("read Phase 18.17 range diagnostics primitive review")
}

fn workspace_file_browser_wiki() -> String {
    fs::read_to_string(repository_path(
        "docs/wiki/modules/workspace-file-browser.md",
    ))
    .expect("read workspace discovery and file browser wiki")
}

#[test]
fn phase18_1_shell_layout_primitive_review_records_existing_inventory() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = phase18_1_shell_layout_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.1-shell-layout-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.1 shell/layout primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.1-shell-layout-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.1 shell/layout primitive review"
    );
    for required in [
        "SDUI tree publication",
        "Action intents and command registry",
        "Editor views and current root widget",
        "Fixed-sidebar SDUI paint path",
        "Behavior manifests",
        "Keybindings",
        "Configuration runtime",
        "Package loading and contribution descriptors",
        "Decoration transport and rendering",
        "Parse coordinator",
        "current `EditorWidget` root",
        "fixed-sidebar SDUI paint path",
        "`SduiTree` snapshots/updates",
        "`SduiActionIntent`",
        "`SIDEBAR_WIDTH`",
        "`editor_region_for_document`",
        "`SduiPanelStatusContribution`",
        "SDUI region collisions",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.1 shell/layout primitive review must record existing inventory text: {required}"
        );
    }
}

#[test]
fn phase18_1_shell_layout_primitive_review_rejects_mode_specific_rust_layout() {
    let review = phase18_1_shell_layout_primitive_review();

    for required in [
        "Do not add Markdown-specific Rust shell branches",
        "No Markdown-specific or package-specific Rust UI branch is required",
        "Markdown preview/status behavior should consume future `PanelContribution` / `PaneSlotLayout` primitives",
        "Acceptable names include `WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`, `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, `PackageThemeTokenDeclaration`, `PackageUiStateScope`, and `PackageLayoutOverride`",
        "Rejected names include `MarkdownPreviewSidebar`, `MarkdownPaneLayout`, `MarkdownMasonryPanel`, `MarkdownThemeCss`, `MarkdownOverlay`, or any `if mode == \"markdown\"` / `if package == \"@clay/markdown\"` Rust shell-layout branch",
        "only generic reusable shell/layout primitives",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.1 shell/layout primitive review must reject mode-specific layout guidance: {required}"
        );
    }
}

#[test]
fn phase18_1_shell_layout_primitive_review_records_hot_path_and_security_boundaries() {
    let review = phase18_1_shell_layout_primitive_review();

    for required in [
        "Configuration/load time",
        "Package validation time",
        "Protocol/update time",
        "Layout/update time",
        "Paint time",
        "Client-first editor hot path",
        "Proposed package JavaScript remains outside paint/layout/input/text-event handlers",
        "no package JavaScript in paint/layout/input/text-event handlers",
        "raw `Deno.core.ops`",
        "native widget handles",
        "Masonry widget constructors",
        "Vello/Parley callbacks",
        "raw CSS",
        "client-side JavaScript hooks",
        "registered package or Clay commands",
        "Style/theme tokens",
        "Package state/data declarations",
        "Package/user overrides",
        "`~/.config/clay/init.js`",
        "WorkingAreaLayout",
        "PaneSplitTree",
        "PaneSlotLayout",
        "PanelContribution",
        "ComponentContribution",
        "TransientOverlayContribution",
        "PackageThemeTokenDeclaration",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.1 shell/layout primitive review must record hot-path/security text: {required}"
        );
    }
}

#[test]
fn phase18_2_shell_runtime_review_records_existing_inventory() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = phase18_2_shell_runtime_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.2-shell-runtime-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.2 shell runtime primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.2-shell-runtime-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.2 shell runtime primitive review"
    );
    for required in [
        "Current application root and driver actions",
        "Editor component and local input surface",
        "Fixed SDUI sidebar and editor-region helper",
        "Behavior manifests and key routing",
        "Command/action registry and UI command routes",
        "Configuration runtime",
        "Package loading and primitive contribution descriptors",
        "Decoration transport and editor render data",
        "Parse coordinator and background package work",
        "Masonry widget/container substrate",
        "`src/main.rs` (`run_editor`, `Driver::on_start`, `Driver::on_action`, `spawn_client_connection_event_bridge`, `connection_event_user_event`)",
        "`src/masonry_editor.rs`, `src/editor/surface.rs`",
        "`src/masonry_sdui.rs` (`SIDEBAR_WIDTH`, `SduiNativeState::paint`, `editor_region`, `editor_region_for_document`, `SduiObservableSnapshot`)",
        "`src/protocol/mod.rs`, `src/behavior/manifest.rs`, `src/editor/surface.rs`",
        "`src/packages/commands.rs`, `runtime/js/commands.ts`, `runtime/js/keybindings.ts`, `src/protocol/sdui.rs`, `src/main.rs::handle_client_ui_command`",
        "`src/server/configuration.rs`, `src/server/ops/configuration.rs`, `runtime/js/configuration.ts`",
        "`src/packages/record.rs`, `src/packages/service.rs`, `src/packages/conflict.rs`",
        "`src/protocol/decorations.rs`, `src/server/decorations.rs`, `src/editor/surface.rs`",
        "`src/server/parse_coordinator.rs`, `src/protocol/parse.rs`, `runtime/js/parse.ts`",
        "`NewWidget`, `NewWindow`, `AppDriver`, `Widget`, `WidgetId`, `WidgetPod`, `RegisterCtx`, `ChildrenIds`, `LayoutCtx`, `PaintCtx`, `RenderRoot::edit_widget`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.2 shell runtime review must record existing inventory text: {required}"
        );
    }
}

#[test]
fn phase18_2_shell_runtime_review_maps_generic_primitives() {
    let review = phase18_2_shell_runtime_primitive_review();

    for required in [
        "Generic Runtime Gaps to Implement in Phase 18.2",
        "### `WorkingAreaLayout`",
        "### `PaneSplitTree`",
        "### `PaneSlotLayout`",
        "### Internal shell observability",
        "`src/shell/mod.rs`, `src/shell/layout.rs`, `src/masonry_shell.rs`, `src/lib.rs`, and `src/main.rs`",
        "State should record one native window working area, one active pane tree root, the active pane, a layout version, and the editor component binding",
        "Model leaf panes and horizontal/vertical split nodes with stable pane IDs, split orientation, bounded ratio/min/max validation",
        "Every leaf pane has exactly one mandatory `main` slot",
        "Optional `left`, `right`, `top`, and `bottom` slots",
        "Existing `SduiNativeState` can be bridged as internal Clay-owned slot content",
        "layout version, pane count, split count, active pane, visible slots, editor component binding",
        "Do not parse packages, run package JavaScript, wait on IPC, deserialize full documents, validate package metadata, or mutate Masonry children during layout",
        "Keypress, text-event, pointer selection, scroll, caret movement, local edit application, and first local paint after input remain client-first",
        "Hidden JSON/TOML/ad hoc shell layout keys are rejected",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.2 shell runtime review must map generic primitive guidance: {required}"
        );
    }

    for deferred in [
        "`PanelContribution` and `clay.ui.serverRegisterPanelContribution`",
        "`ComponentContribution` and `clay.ui.serverRegisterComponentContribution`",
        "`TransientOverlayContribution` and `clay.ui.serverRegisterTransientOverlayContribution`",
        "`PackageThemeTokenDeclaration` and `clay.ui.serverRegisterThemeToken`",
        "`PackageUiStateScope` and `clay.ui.serverRegisterUiStateScope`",
        "`PackageLayoutOverride` and `clay.ui.serverSetLayoutOverride`",
        "public `clay:ui` configuration APIs remain planned",
    ] {
        assert!(
            review.contains(deferred),
            "Phase 18.2 shell runtime review must defer later package UI surface: {deferred}"
        );
    }
}

#[test]
fn phase18_2_shell_runtime_review_rejects_mode_specific_shell_branches() {
    let review = phase18_2_shell_runtime_primitive_review();

    for required in [
        "Do not keep `EditorWidget` as the application shell and hide shell state inside it",
        "Do not fork the fixed sidebar into `MarkdownPreviewSidebar`, `MarkdownPaneLayout`, `MarkdownMasonryPanel`, `MarkdownShellWidget`, or any `if mode == \"markdown\"` / `if package == \"@clay/markdown\"` Rust shell/layout branch",
        "Do not expose Masonry `Widget`, `WidgetId`, `WidgetPod`, `Split`, `Flex`, native handles, layout callbacks, Vello callbacks, Parley callbacks, or raw op names as package APIs",
        "Do not add package validation, package parsing, configuration evaluation, JavaScript execution, or blocking IPC to Masonry paint/layout/pointer/scroll/key/text-event handlers",
        "Do not promote planned `clay:ui` inventory stubs to callable APIs without full Clay JS facade/op/reference docs/registry/test coverage",
        "Packages must not create Masonry widgets, mutate native layout, provide raw CSS/HTML/scripts, run client-side JavaScript, call raw `Deno.core.ops`, receive native widget IDs/handles, provide Vello/Parley callbacks",
        "No package JavaScript, package validation, configuration evaluation, package parsing, blocking IPC, or full-document serialization may enter these paths",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.2 shell runtime review must reject unsafe/mode-specific shell branch: {required}"
        );
    }
}

#[test]
fn phase18_3_slot_ui_review_records_existing_inventory() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = phase18_3_slot_ui_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.3-slot-ui-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.3 slot UI primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.3-slot-ui-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.3 slot UI primitive review"
    );

    for required in [
        "Existing Package UI Primitive Inventory",
        "SDUI helpers and runtime publication",
        "Shell slot state and geometry",
        "Command registry and action validation",
        "Package manifest, permissions, and provenance",
        "Package contribution metadata and conflicts",
        "Clay JS API inventory and documentation registry",
        "Structural observability",
        "`runtime/js/sdui.ts`, `src/server/ops/sdui.rs`, `src/server/sdui.rs`, `src/protocol/sdui.rs`",
        "`src/shell/layout.rs`, `src/masonry_shell.rs`",
        "`src/masonry_sdui.rs` (`SIDEBAR_WIDTH`, `sdui_panel_left_slot_rect`, `editor_region`, `editor_region_for_document`, `SduiObservableSnapshot`)",
        "`src/packages/manifest.rs`, `src/packages/permissions.rs`, `src/packages/record.rs`",
        "`src/packages/record.rs`, `src/packages/conflict.rs`",
        "panel`, `label`, `button`, `list`, `editorView`, `flex`, and `stack`",
        "`PANEL_BACKGROUND`, `BUTTON_BACKGROUND`, `LIST_BACKGROUND`, `TEXT_COLOR`, `PANEL_PADDING`, `ROW_HEIGHT`, `TITLE_TEXT_SIZE`, and `BODY_TEXT_SIZE`",
        "SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES",
        "SDUI_UPDATE_PAYLOAD_BUDGET_BYTES",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.3 slot UI primitive review must record existing inventory text: {required}"
        );
    }
}

#[test]
fn phase18_3_slot_ui_review_maps_generic_primitives() {
    let review = phase18_3_slot_ui_primitive_review();

    for required in [
        "Generic Phase 18.3 Primitive Gaps",
        "### `PanelContribution`",
        "### `ComponentContribution`",
        "### `TransientOverlayContribution`",
        "### `PackageThemeTokenDeclaration`",
        "`runtime/js/ui.ts`, `src/server/ops/ui.rs`, `src/server/ui.rs` or `src/shell/contributions.rs`, `src/shell/components.rs`, `src/protocol/ui.rs` or `src/protocol/sdui.rs`, `src/masonry_sdui.rs`, `src/masonry_shell.rs`, `src/packages/record.rs`, `src/packages/conflict.rs`, and public docs/tests",
        "package-prefixed panel ID, target slot (`left`, `right`, `top`, or `bottom`)",
        "fixed/transient kind separation",
        "Reuse existing SDUI node semantics for `panel`, `label`, `button`, `list`, `editorView`, `flex`, and `stack`",
        "Add or explicitly defer `scroll/portal`, `statusItem`, `table`, `dropdown`, `collapse`, and `modal`",
        "component IDs must be package-prefixed or Clay-owned",
        "focus policy, dismissal policy, accessibility role/label metadata",
        "typed semantic package tokens and component style variables",
        "Package-owned tokens must use the package prefix",
        "Token declaration and user override validation are package load/config/update work",
        "Deferred to Phase 18.4 Unless Deliberately Promoted",
        "`clay.ui.serverRegisterUiStateScope`",
        "`clay.ui.serverSetLayoutOverride`",
        "hidden JSON/TOML/ad hoc panel/style/layout configuration keys",
        "docs/index links, generated registry coverage, key binding metadata, permissions/security notes, backing Rust/op/facade paths, and tests",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.3 slot UI primitive review must map generic primitive guidance: {required}"
        );
    }
}

#[test]
fn phase18_3_slot_ui_review_rejects_mode_specific_ui_branches() {
    let review = phase18_3_slot_ui_primitive_review();

    for required in [
        "not a Markdown preview sidebar",
        "Do not add `MarkdownPreviewSidebar`, `MarkdownPaneLayout`, `MarkdownMasonryPanel`, `MarkdownThemeCss`, `MarkdownOverlay`, or any `if mode == \"markdown\"` / `if package == \"@clay/markdown\"` Rust shell/UI branch",
        "Do not expose Masonry `Widget`, `WidgetId`, `WidgetPod`, `Flex`, `Portal`, `Split`, native handles, layout callbacks, Vello callbacks, Parley callbacks, or raw op names as package APIs",
        "Do not promote planned `clay:ui` APIs by wiring only raw ops or inventory rows",
        "Do not add package validation, package parsing, configuration evaluation, JavaScript execution, or blocking IPC to Masonry paint/layout/pointer/scroll/key/text-event handlers",
        "Do not treat hidden config keys, raw CSS, raw style strings, or arbitrary color strings as temporary package author APIs",
        "No package JavaScript, schema validation, package parsing, full-document serialization, raw IPC wait, or child mutation should happen inside Masonry paint/layout/pointer/scroll/key/text-event handlers",
        "package JavaScript and package validation stay out of Masonry hot paths",
        "raw CSS, raw ops, native widget handles, direct Masonry widget constructors, client-side JavaScript, renderer callbacks",
        "Package UI declarations grant no filesystem, network, shell, AI mutation, WASM, package-manager execution, package enable/disable, workspace mutation, raw Deno op, native widget, or client-side JavaScript authority",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.3 slot UI primitive review must reject unsafe/mode-specific UI branch: {required}"
        );
    }
}

#[test]
fn phase18_4_input_state_config_review_records_existing_inventory() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = phase18_4_input_state_config_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.4-input-state-config-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.4 input/state/config primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.4-input-state-config-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.4 input/state/config primitive review"
    );

    for required in [
        "Existing Package Primitive Inventory",
        "Behavior manifests and text/key routing",
        "Keybindings",
        "Command registry and action intents",
        "SDUI and Clay component catalog",
        "Shell `PaneSlotLayout` and internal shell runtime",
        "Package UI registry and runtime state",
        "Package manifest and contribution metadata",
        "Configuration runtime and planned configuration APIs",
        "Clay JS API inventory and docs registry",
        "Structural observability",
        "`src/behavior/manifest.rs`, `src/protocol/mod.rs`, `src/editor/surface.rs`, `runtime/js/keybindings.ts`",
        "`runtime/js/commands.ts`, `src/server/ops/commands.rs`, `src/packages/commands.rs`, `src/protocol/sdui.rs`",
        "`runtime/js/sdui.ts`, `src/protocol/sdui.rs`, `src/server/sdui.rs`, `src/server/ops/sdui.rs`, `src/shell/components.rs`, `src/masonry_sdui.rs`",
        "`src/shell/layout.rs`, `src/masonry_shell.rs`",
        "`runtime/js/ui.ts`, `src/server/ops/ui.rs`, `src/server/ui.rs`, `src/shell/package_ui.rs`, `src/shell/theme.rs`",
        "`runtime/js/configuration.ts`, `src/server/configuration.rs`, `src/server/ops/configuration.rs`, `src/server/js_runtime.rs`",
        "`docs/reference/clay-js-api/api-inventory.toml`, `docs/reference/clay-js-api/`, `docs/generated/clay-js-api-registry.json`",
        "`SduiActionIntent`",
        "`serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken`",
        "`setPackageOption`, `setModePreference`, `setDecorationTheme`, and `setParsePolicy`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.4 primitive review must record existing inventory text: {required}"
        );
    }
}

#[test]
fn phase18_4_input_state_config_review_maps_generic_primitives() {
    let review = phase18_4_input_state_config_primitive_review();

    for required in [
        "Generic Phase 18.4 Primitive Gaps",
        "### `PackageInputContribution`",
        "`clay.ui.serverRegisterInputContribution` / `serverRegisterInputContribution`",
        "component-scoped action and focus metadata",
        "pointer click interests, hover/menu hints if needed, mouse selection and drag policies",
        "behavior manifests for key/text behavior",
        "command registry for side effects",
        "### `PackageUiStateScope`",
        "`clay.ui.serverRegisterUiStateScope` / `serverRegisterUiStateScope`",
        "`package-global`, `user-config`, `workspace`, `document`, `pane`, `component`, and `transient-overlay`",
        "### `PackageLayoutOverride`",
        "`clay.ui.serverSetLayoutOverride` / `serverSetLayoutOverride`",
        "Precedence remains: Clay shell safety invariants and hard prohibitions, user configuration through documented Clay JS APIs, active major mode layout defaults, compatible minor mode contributions, global package contributions, package fallback/defaults",
        "### `PackageOwnedConfiguration`",
        "`clay.configuration.setPackageOption` / `setPackageOption`",
        "Package options should be available only for package-declared typed option schemas",
        "Theme-token remaps and package fallback/defaults",
        "reuse `PackageThemeTokenDeclaration` and `ThemeTokenResolver`",
        "`~/.config/clay/init.js`",
        "Configuration/load/update work",
        "Behavior-manifest update work",
        "Explicit command/UI update work",
        "Protocol/client update work",
        "Paint/layout state read",
        "Editor hot-path work",
        "Extend package records/conflicts/provenance for input/state/config/layout metadata",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.4 primitive review must map generic primitive guidance: {required}"
        );
    }
}

#[test]
fn phase18_4_input_state_config_review_rejects_mode_specific_branches() {
    let review = phase18_4_input_state_config_primitive_review();

    for required in [
        "Do not add `MarkdownPreviewInput`, `MarkdownPreviewState`, `MarkdownLayoutOverride`, `MarkdownPanelVisibility`, `MarkdownThemeOverride`, `MarkdownPaneSelector`, or any `if mode == \"markdown\"` / `if package == \"@clay/markdown\"` Rust input/state/config/layout branch",
        "Do not expose Masonry `Widget`, `WidgetId`, `WidgetPod`, native handles, event callbacks, focus callbacks, layout callbacks, Vello callbacks, Parley callbacks, renderer callbacks, or raw op names as package APIs",
        "Do not implement package input by delivering raw pointer/key/text events to package JavaScript or client-side JavaScript",
        "Do not run package validation, package parsing, configuration evaluation, JavaScript execution, blocking IPC, full-document serialization, or child mutation from Masonry paint/layout/pointer/scroll/key/text-event handlers",
        "Do not treat hidden config keys, raw CSS, raw style strings, raw colors, or arbitrary JSON state blobs as temporary package authoring APIs",
        "Do not promote `clay.ui.serverRegisterUiStateScope`, `clay.ui.serverSetLayoutOverride`, or `clay.configuration.setPackageOption` by wiring only inventory rows or raw ops",
        "raw native event callbacks",
        "raw arbitrary native events",
        "raw `Deno.core.ops`",
        "direct Masonry widget",
        "native widget",
        "raw CSS",
        "client-side JavaScript",
        "hidden JSON/TOML/ad hoc layout, style, input, theme, or package option keys",
        "None of these primitives grant filesystem, network, shell, extension loading, AI mutation, workspace mutation, package installation/enable/disable, package-manager execution, WASM",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.4 primitive review must reject unsafe/mode-specific implementation shape: {required}"
        );
    }
}

#[test]
fn phase18_8_transient_menu_command_execution_review_records_inventory_and_gaps() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let registry = primitives_registry();
    let shell_strategy = shell_layout_strategy();
    let review = phase18_8_transient_menu_command_execution_primitive_review();

    assert!(
        wiki_index
            .contains("modules/phase18.8-transient-menu-command-execution-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.8 primitive review"
    );
    assert!(
        primitive_architecture
            .contains("phase18.8-transient-menu-command-execution-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.8 primitive review"
    );
    assert!(
        wiki_index.contains("modules/transient-menu-session.md"),
        "docs/wiki/index.md must link the Transient Menu Session implementation wiki"
    );
    assert!(
        primitive_architecture.contains("transient-menu-session.md"),
        "primitive architecture wiki must link the Transient Menu Session implementation wiki"
    );
    assert!(
        wiki_index.contains("modules/control-center.md"),
        "docs/wiki/index.md must link the Control Center implementation wiki"
    );
    assert!(
        primitive_architecture.contains("control-center.md"),
        "primitive architecture wiki must link the Control Center implementation wiki"
    );

    for required in [
        "Existing Primitive Inventory",
        "Command metadata and behavior routes",
        "SDUI/action primitives",
        "Shell, slot, and transient overlay primitives",
        "Package UI, input, state, and configuration primitives",
        "Persistent server runtime primitives",
        "`src/packages/commands.rs::CommandRegistry`",
        "`runtime/js/commands.ts`",
        "`src/protocol/sdui.rs::SduiActionIntent`",
        "`src/shell/layout.rs`",
        "`src/shell/package_ui.rs::PackageUiRuntimeState`",
        "`src/server/js_runtime.rs::ClayJsRuntimeService`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.8 primitive review must record inventory text: {required}"
        );
    }

    for required in [
        "Generic Phase 18.8 Primitive Gaps",
        "### `CommandExecution`",
        "### `TransientMenuSession`",
        "Local Filtering vs Server-First Execution",
        "Command metadata listing snapshot",
        "Query update over installed menu items",
        "Activate selected item",
        "Server-first command execution",
        "SDUI actions, package UI actions, behavior-manifest keybindings, and transient-menu selections",
        "Control Center is the first consumer",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.8 primitive review must map generic gaps/hot-path split: {required}"
        );
    }

    for required in [
        "Do not add `ControlCenterWidget`",
        "Do not implement command activation separately for SDUI buttons, package UI actions, keybindings, and transient-menu selections",
        "Do not make `TransientOverlayContribution` alone carry active query",
        "raw op names",
        "No bottom transient menu path may run package JavaScript",
        "filesystem, network, shell, AI, WASM, package-manager, package installation, package enable/disable",
        "raw-op, client-side JavaScript",
        "The Phase 18.8 review introduces no new filesystem, network, shell, AI, WASM, native-widget, raw-op, client-side JavaScript, package-manager, package-install, or package-enable/disable authority",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.8 primitive review must reject unsafe or authority-expanding shape: {required}"
        );
    }

    for required in [
        "CommandExecution",
        "TransientMenuSession",
        "serverExecuteCommand",
        "serverOpenTransientMenu",
        "Phase 18.8 primitive review",
    ] {
        assert!(
            registry.contains(required) || shell_strategy.contains(required),
            "primitive registry or shell strategy must mention Phase 18.8 primitive term: {required}"
        );
    }
}

#[test]
fn phase18_8_package_guide_documents_command_execution_and_transient_menu_contract() {
    let guide = creating_packages_guide();

    for required in [
        "Phase 18.8 authoring contract: command execution and transient menus",
        "inert command intent",
        "clay.commands.serverRegisterCommand",
        "CommandExecution",
        "TransientMenuSession",
        "fixed panel",
        "transient overlay",
        "transient menu",
        "Control Center",
        "Command execution lifecycle",
        "server-owned execution path",
        "no callbacks or client-side handlers",
        "client-side JavaScript",
        "raw `Deno.core.ops`",
        "Masonry widgets",
        "bypass command permission",
        "package installation",
        "package enable/disable",
        "Ordinary typing remains client-first",
    ] {
        assert!(
            guide.contains(required),
            "package guide must document Phase 18.8 command execution/transient menu contract phrase: {required}"
        );
    }
}

#[test]
fn phase18_9_generic_text_code_modes_primitive_review_records_inventory_and_gaps() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let registry = primitives_registry();
    let review = phase18_9_generic_text_code_modes_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.9-generic-text-code-modes-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.9 primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.9-generic-text-code-modes-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.9 primitive review"
    );

    for required in [
        "Existing Primitive Inventory",
        "Document classification and major-mode activation",
        "Behavior manifest and generic text transforms",
        "Key routing and commands",
        "SDUI, status, and decoration surfaces",
        "Document open path",
        "`src/packages/modes.rs::ModeRegistry`",
        "`src/protocol/mod.rs::EditorBehaviorRules`",
        "`src/behavior/manifest.rs`",
        "`src/packages/commands.rs::CommandRegistry`",
        "`src/server/command_execution.rs::CommandExecutor`",
        "`src/server/control_center.rs`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.9 primitive review must record inventory text: {required}"
        );
    }

    for required in [
        "Generic Phase 18.9 Primitive Gaps",
        "### Built-in always-on `core.text` and `core.code` fallback major modes",
        "### Classification shebang and bounded leading-content probes",
        "### Electric characters",
        "### Mode discovery/listing commands",
        "precedence: user override > package-declared pattern",
        "core.code > core.text",
        "always-on",
        "require no `~/.config/clay/init.js` line and no package load step",
        "electric-character manifest kind",
        "`clay.modes.listActiveModes`",
        "clay.modes.explainActiveMode",
        "read-only `CommandDeclaration` consumers routed through `CommandExecutor`",
        "most generic key behavior already exists as manifest data",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.9 primitive review must map generic gaps/precedence/discovery: {required}"
        );
    }

    for required in [
        "Hot-Path Classification",
        "Open/reload-time, no-hot-path",
        "`ClientFirstPredictable` manifest data",
        "Server-first explicit command",
        "Reclassification after package reload/enable/disable",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.9 primitive review must record hot-path classification split: {required}"
        );
    }

    for required in [
        "Do not add `PlainTextMode`",
        "Do not implement `core.text`/`core.code` as first-party `@clay/core-text`/`@clay/core-code` JS packages requiring `loadPackage`",
        "Do not add a parallel `FallbackModeRegistry`",
        "Do not invent a `CommentContinuation` or `PairInsertion` primitive",
        "No new primitive column is required: built-in modes are `MajorModeActivation`/`DocumentClassification` data registered by Clay itself. The plan's `FallbackModeDeclaration` notion folds into built-in mode registration, not a separate primitive.",
        "Do not run filesystem scans, directory walks, arbitrary package predicates",
        "Do not implement mode discovery as a one-off SDUI panel",
        "The Phase 18.9 review introduces no new filesystem, network, shell, AI, WASM, native-widget, raw-op, client-side JavaScript, package-manager, package-install, or package-enable/disable authority",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.9 primitive review must reject unsafe/mode-specific/authority-expanding shape: {required}"
        );
    }

    for required in [
        "core.text",
        "core.code",
        "shebang and bounded leading-content probes",
        "precedence ladder",
        "electric-character manifest kind",
        "fallback command routing",
    ] {
        assert!(
            registry.contains(required),
            "primitive registry must mention Phase 18.9 primitive term: {required}"
        );
    }
}

#[test]
fn phase18_10_tree_sitter_grammar_primitive_review_records_inventory_and_gaps() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let registry = primitives_registry();
    let backlog = primitives_backlog();
    let review = phase18_10_tree_sitter_grammar_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.10-tree-sitter-grammar-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.10 primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.10-tree-sitter-grammar-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.10 primitive review"
    );

    for required in [
        "Existing Primitive Inventory",
        "Document classification and major-mode activation",
        "Package loading and manifest validation",
        "Parse coordinator and background work",
        "Decoration transport and style-token validation",
        "Docs registry and wiki coverage",
        "`src/packages/modes.rs::ModeRegistry`",
        "`src/packages/manifest.rs`",
        "`src/packages/permissions.rs`",
        "`src/server/parse_coordinator.rs`",
        "`src/server/decorations.rs`",
        "`runtime/js/parse.ts`",
        "`runtime/js/decorations.ts`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.10 primitive review must record existing inventory text: {required}"
        );
    }

    for required in [
        "Generic Phase 18.10 Primitive Gaps",
        "### `SyntaxGrammarContribution`",
        "### Grammar registry",
        "### Query/capture validation",
        "### Syntax provider selection",
        "active syntax grammar separate from active major mode",
        "active_major_mode: core.code",
        "active_syntax_grammar: rust",
        "@clay/rust",
        "@clay/typescript",
        "@clay/javascript",
        "grammar-only packages",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.10 primitive review must map generic grammar gaps: {required}"
        );
    }

    for required in [
        "Hot-Path Classification",
        "Package load / grammar validation",
        "Document open / reload / explicit reclassification",
        "Background parse/highlight work",
        "Paint/text-event/key hot path",
        "No Tree-sitter parsing",
        "no synchronous IPC before local paint",
        "`INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`",
        "`DECORATION_PAYLOAD_BUDGET_BYTES`",
        "`SYNTAX_CACHE_BUDGET_BYTES`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.10 primitive review must record hot-path split: {required}"
        );
    }

    for required in [
        "Security and Authority Boundary",
        "First-party-only grammar artifact scope",
        "Package-root path confinement",
        "No arbitrary native/third-party artifact loading",
        "No new filesystem, network, shell, AI, WASM, native-widget, raw-op, client-side JavaScript, package-manager, package-install, package-enable/disable, or package-control authority",
        "Do not add language-specific Rust parser/highlighter branches",
        "Do not implement `@clay/rust`, `@clay/typescript`, or `@clay/javascript` as full major modes",
        "Do not silently auto-load grammar packages",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.10 primitive review must reject unsafe/language-specific shape: {required}"
        );
    }

    for required in [
        "SyntaxGrammarContribution",
        "clay.syntax.serverRegisterSyntaxGrammar",
        "active syntax grammar",
        "first-party grammar-only packages",
        "no arbitrary native/third-party artifact loading",
    ] {
        assert!(
            registry.contains(required) || backlog.contains(required) || review.contains(required),
            "Phase 18.10 docs must mention grammar primitive term: {required}"
        );
    }

    assert!(
        registry.contains("SyntaxGrammarContribution"),
        "primitive registry must contain the SyntaxGrammarContribution row"
    );
    assert!(
        backlog.contains("SyntaxGrammarContribution"),
        "primitive backlog must contain the SyntaxGrammarContribution handoff row"
    );
}

#[test]
fn phase18_16_tiered_engine_primitive_review_linked_and_complete() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = phase18_16_tiered_engine_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.16-tiered-tree-sitter-engine-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.16 primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.16-tiered-tree-sitter-engine-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.16 primitive review"
    );

    for required in [
        "Existing Primitive Inventory",
        "Grammar registry and package grammar metadata",
        "Parse coordinator and open-time parse path",
        "Decoration transport and vocabulary/theme registry",
        "Package loading and JS parse bridge",
        "`src/server/syntax.rs::SyntaxGrammarRegistry`",
        "`src/server/parse_coordinator.rs::ParseCoordinator`",
        "`src/protocol/decorations.rs::DecorationSpan`",
        "`src/editor/theme.rs::StyleRegistry`",
        "`runtime/js/syntax.ts`",
        "`runtime/js/parse.ts`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.16 primitive review must inventory existing primitive: {required}"
        );
    }

    for required in [
        "Generic Phase 18.16 Gaps",
        "### `SyntaxEngineTier` and engine selection/provenance",
        "### Tier 1 native first-party descriptors",
        "### Tier 2 web-tree-sitter host adapter",
        "### Tier 3 JS parser fallback",
        "### One capture-to-vocabulary mapper",
        "### Open-parse diagnostics",
        "SyntaxEngineTier::Native",
        "SyntaxEngineTier::Wasm",
        "SyntaxEngineTier::JavaScriptFallback",
        "SyntaxGrammarContribution -> SyntaxEngineSelection -> ParseCoordinator -> DecorationSet(TokenType, Modifiers)",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.16 primitive review must map generic tier gaps: {required}"
        );
    }

    for required in [
        "Hot-Path Classification",
        "Package load / grammar validation",
        "Document open / reload / explicit reclassification",
        "Background parse/highlight work",
        "Paint/text-event/key/layout/scroll/pointer hot path",
        "No parser/query compilation",
        "no runtime configuration evaluation",
        "`INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`",
        "`DECORATION_PAYLOAD_BUDGET_BYTES`",
        "`SYNTAX_CACHE_BUDGET_BYTES`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.16 primitive review must record hot-path split: {required}"
        );
    }

    for required in [
        "Security and Authority Boundary",
        "First-party native grammars are compiled-in Clay-maintained grammar data",
        "Tier 2 WASM artifacts must be resolver-validated, package-root-confined `grammars/*.wasm` files",
        "Tier 3 JS parser fallback stays server-side through existing runtime handler tokens",
        "The client receives only inert `DecorationSet`/`RuntimeDiagnostic` data",
        "filesystem, network, shell, AI, workspace mutation, native-ui, package-control, package-manager, raw-ops, and client-runtime authority stay out of scope",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.16 primitive review must record security boundary: {required}"
        );
    }

    for required in [
        "Rejected Implementation Shapes",
        "Do not add `RustSyntaxHighlighter`, `TypeScriptSyntaxHighlighter`, `JavaScriptSyntaxHighlighter`, `MarkdownTreeSitterHighlighter`",
        "Do not add a second parse scheduler",
        "Do not run Tree-sitter, web-tree-sitter, package JavaScript, query compilation, or package loading in Masonry paint",
        "Do not silently let packages override Tier 1 native highlighting by load order or priority alone",
        "Do not add hidden JSON/TOML syntax-engine keys",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.16 primitive review must reject unsafe/language-specific shape: {required}"
        );
    }
}

#[test]
fn typography_primitive_is_registered_documented_and_indexed() {
    let docs_index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");
    let primitive_index = primitives_index();
    let registry = primitives_registry();
    let rendering = rendering_strategy();
    let security = package_security();
    let typography = typography_contract();

    assert!(docs_index.contains("reference/primitives/typography.md"));
    assert!(primitive_index.contains("typography.md"));
    assert!(registry.contains("| SemanticTypographyRole |"));
    for marker in [
        "defaultFontRole",
        "fontRole",
        "style.fontRole",
        "core.code",
        "core.text",
        "TypographyRegistry",
        "TYPOGRAPHY_PAYLOAD_BUDGET_BYTES",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "tests/typography_protocol.rs",
        "docs/development/launch-and-gui-smoke.md",
    ] {
        assert!(
            typography.contains(marker),
            "typography reference must document {marker}"
        );
    }
    assert!(rendering.contains("## Semantic Typography Roles"));
    assert!(security.contains("## Semantic Typography Authority Boundary"));
}

#[test]
fn range_diagnostics_reference_is_indexed_and_complete() {
    let docs_index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");
    let primitive_index = primitives_index();
    let registry = primitives_registry();
    let backlog = fs::read_to_string(repository_path("docs/reference/primitives/backlog.md"))
        .expect("read primitive backlog");
    let rendering = rendering_strategy();
    let parse_strategy = fs::read_to_string(repository_path(
        "docs/reference/primitives/parse-update-strategy.md",
    ))
    .expect("read parse update strategy");
    let security = package_security();
    let diagnostics = diagnostics_contract();
    let package_guide = fs::read_to_string(repository_path(
        "docs/reference/packages/creating-packages.md",
    ))
    .expect("read package author guide");
    let launch = fs::read_to_string(repository_path("docs/development/launch-and-gui-smoke.md"))
        .expect("read launch smoke docs");

    assert!(docs_index.contains("reference/primitives/diagnostics.md"));
    assert!(primitive_index.contains("diagnostics.md"));
    assert!(registry.contains("| DiagnosticSpan |"));
    assert!(backlog.contains("| DiagnosticSpan |"));
    assert!(rendering.contains("## Range Diagnostics"));
    assert!(parse_strategy.contains("DIAGNOSTIC_PAYLOAD_BUDGET_BYTES"));
    assert!(security.contains("## Range Diagnostics Authority Boundary"));
    assert!(package_guide.contains("### Phase 18.17 range diagnostics publication"));
    assert!(launch.contains("Phase 18.17 range diagnostics and syntax-error smoke"));

    for marker in [
        "DiagnosticSpan",
        "DiagnosticSet",
        "RuntimeDiagnostic",
        "DecorationSpan",
        "ERROR",
        "MISSING",
        "next UTF-8 scalar",
        "previous scalar",
        "diagnostic_update",
        "serverPublishDiagnostics",
        "diagnosticError",
        "diagnosticWarning",
        "diagnosticInfo",
        "DIAGNOSTIC_PAYLOAD_BUDGET_BYTES",
        "DIAGNOSTIC_MAX_SPANS_PER_SET",
        "DIAGNOSTIC_CACHE_BUDGET_BYTES",
        "render-decorations",
        "tests/range_diagnostics.rs",
        "additive",
    ] {
        assert!(
            diagnostics.contains(marker),
            "range diagnostics reference must document {marker}"
        );
    }
}

#[test]
fn range_diagnostics_implementation_wiki_is_linked_and_complete() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let page = fs::read_to_string(repository_path("docs/wiki/modules/range-diagnostics.md"))
        .expect("read range diagnostics implementation wiki");
    let review = phase18_17_range_diagnostics_primitive_review();

    assert!(
        wiki_index.contains("modules/range-diagnostics.md"),
        "docs/wiki/index.md must link the range diagnostics implementation page"
    );
    assert!(
        review.contains("range-diagnostics.md"),
        "Phase 18.17 primitive review must link the implementation wiki"
    );

    for required in [
        "## Overview",
        "## Responsibilities",
        "## How It Works",
        "Protocol and validation",
        "Parse side channel",
        "Tree-sitter extraction",
        "Server/client chunk lifecycle",
        "Package publication",
        "Theme and paint",
        "## Code Examples",
        "## Primitive Coverage",
        "## Invariants and Constraints",
        "## Tests",
        "## Related",
        "DiagnosticSpan",
        "DiagnosticSet",
        "DiagnosticChunkCache",
        "EditorDiagnosticState",
        "collect_syntax_diagnostics",
        "visible_scalar_range",
        "diagnostic_update",
        "serverPublishDiagnostics",
        "op_clay_diagnostics_publish_diagnostics",
        "paint_squiggle",
        "diagnostic_style",
        "diagnosticError",
        "DIAGNOSTIC_PAYLOAD_BUDGET_BYTES",
        "DIAGNOSTIC_CACHE_BUDGET_BYTES",
        "DIAGNOSTIC_MAX_SPANS_PER_SET",
        "render-decorations",
        "additive",
        "RuntimeDiagnostic",
        "layout_style_revision",
        "tests/range_diagnostics.rs",
        "tests/syntax_grammar.rs",
        "tests/editor_performance_invariants.rs",
        "diagnostics.md",
        "server-publish-diagnostics.md",
    ] {
        assert!(
            page.contains(required),
            "range diagnostics implementation wiki must document {required}"
        );
    }
}

#[test]
fn typography_documentation_checks_do_not_mutate_generated_files() {
    let generated = repository_path("docs/generated/clay-js-api-registry.json");
    let before = fs::read(&generated).expect("read generated registry before documentation checks");
    let _ = typography_contract();
    let _ = primitives_index();
    let _ = primitives_registry();
    let after = fs::read(&generated).expect("read generated registry after documentation checks");
    assert_eq!(before, after, "documentation checks must be read-only");
}

#[test]
fn typography_implementation_wiki_is_linked_and_complete() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let page = fs::read_to_string(repository_path(
        "docs/wiki/modules/typography-registry-and-font-roles.md",
    ))
    .expect("read typography implementation wiki");

    assert!(
        wiki_index.contains("modules/typography-registry-and-font-roles.md"),
        "docs/wiki/index.md must link the typography implementation page"
    );

    for required in [
        "## Overview",
        "## Responsibilities",
        "## How It Works",
        "Configuration and server state",
        "Protocol and delivery",
        "Client registry and resolution",
        "Editor layout and role normalization",
        "Geometry",
        "Native UI, SDUI, and accessibility",
        "Package component roles",
        "## Code Examples",
        "## Primitive Coverage",
        "## Invariants and Constraints",
        "## Tests",
        "## Related",
        "TypographyRegistry",
        "ResolvedFontProfile",
        "UiTextVariant",
        "UiTextMetrics",
        "ActiveTypography",
        "FontProfile",
        "DocumentFontRole",
        "document_line_height",
        "DOCUMENT_LINE_HEIGHT_MULTIPLIER",
        "layout_style_revision",
        "normalize_visible_text_style_runs",
        "font_role_precedes",
        "decoration_layer_rank",
        "font_role_rank",
        "with_presentation",
        "VisibleTextStyleRun",
        "install_active_typography",
        "ActiveTypographyState",
        "op_clay_theme_set_typography",
        "set-typography.md",
        "typography.md",
        "Only `Syntax` and `Semantic`",
        "monotonic",
        "`f32` sizes in `FontProfile`",
        "no package JavaScript, IPC, filesystem/network access, font download, or server-side installed-font discovery",
        "tests/typography_protocol.rs",
        "tests/editor_performance_invariants.rs",
        "tests/manual_smoke_docs.rs",
    ] {
        assert!(
            page.contains(required),
            "typography implementation wiki must document {required}"
        );
    }
}

#[test]
fn phase18_16_5_typography_primitive_review_is_linked_and_complete() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = phase18_16_5_typography_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.16.5-typography-primitive-review.md"),
        "wiki index must link the Phase 18.16.5 primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.16.5-typography-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.16.5 primitive review"
    );

    for required in [
        "Existing Primitive Inventory",
        "Configuration, bootstrap, and live client delivery",
        "Style registry and decoration transport",
        "Cached Parley editor layout and UTF-8 geometry",
        "Viewport, scrolling, and editor chrome geometry",
        "Native UI, SDUI, components, and accessibility",
        "Package validation and authority boundary",
        "`src/editor/layout.rs::LayoutState`",
        "`src/editor/theme.rs::StyleRegistry`",
        "`src/protocol/decorations.rs::DecorationSpan`",
        "`src/masonry_sdui.rs`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.16.5 primitive review must inventory {required}"
        );
    }

    for required in [
        "Generic Phase 18.16.5 Gaps",
        "Separate `ActiveTypography` snapshot and atomic configuration",
        "`TypographyRegistry` and semantic roles",
        "Normalized role-aware layout runs",
        "Typography-aware cache and conservative geometry",
        "`TypographyRegistry::document_line_height()`",
        "Shared UI typography metrics",
        "UiTextMetrics",
        "style.fontRole",
        "accessibility bounds",
        "document default first; then normalized Syntax/Semantic spans",
        "Diagnostic` and `SearchMatch` remain paint-only",
        "monospace",
        "proportional",
        "ui",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.16.5 primitive review must lock generic gap/rule {required}"
        );
    }

    for required in [
        "Hot-Path Classification",
        "No family parsing, installed-font discovery, package JavaScript, server IPC",
        "`DECORATION_PAYLOAD_BUDGET_BYTES`",
        "`SYNTAX_CACHE_BUDGET_BYTES`",
        "`SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`",
        "Security and Authority Boundary",
        "font-file/byte/path/URL authority",
        "filesystem, shell, AI, workspace mutation, native-ui, package-control, package-manager, raw-ops, or client-runtime authority",
        "Rejected Implementation Shapes",
        "Do not shape full documents merely to make scrolling exact",
        "Do not add hidden JSON/TOML typography keys or three independent profile setters",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.16.5 primitive review must preserve boundary {required}"
        );
    }
}

#[test]
fn phase18_17_range_diagnostics_primitive_review_is_linked_and_complete() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = phase18_17_range_diagnostics_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.17-range-diagnostics-primitive-review.md"),
        "wiki index must link the Phase 18.17 primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.17-range-diagnostics-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.17 primitive review"
    );

    for required in [
        "Existing Primitive Inventory",
        "Status diagnostics and diagnostic severity",
        "Decoration layers, transport, and client cache",
        "Style registry and native rendering",
        "Parse coordinator and incremental update transport",
        "Tiered syntax engine and Tree-sitter error nodes",
        "Package permissions and Clay JS publication",
        "`src/protocol/mod.rs::RuntimeDiagnostic`",
        "`src/protocol/mod.rs::DiagnosticSeverity`",
        "`src/protocol/decorations.rs::DecorationSpan`",
        "`src/server/parse_coordinator.rs::ParseCoordinator`",
        "`src/server/syntax.rs::TreeSitterSyntaxHandler`",
        "`src/editor/theme.rs::StyleRegistry`",
        "`src/editor/layout.rs::LayoutState`",
        "`runtime/js/decorations.ts`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.17 primitive review must inventory {required}"
        );
    }

    for required in [
        "What Existing Primitives Already Achieve",
        "Generic Phase 18.17 Gaps",
        "Distinct `DiagnosticSpan` and `DiagnosticSet`",
        "Central diagnostic validation and budgets",
        "Engine-neutral parse diagnostic side channel",
        "Source-keyed server/client chunk lifecycle",
        "Severity-aware theme resolution and native squiggle geometry",
        "Bounded package publication for future analyzers/LSP bridges",
        "next UTF-8 scalar",
        "previous scalar",
        "empty document",
        "Syntax, Semantic, Diagnostic, and Search remain additive layers",
        "RuntimeDiagnostic` unchanged for status failures",
        "diagnostic_update: Option<DiagnosticSet>",
        "clay:diagnostics.serverPublishDiagnostics",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.17 primitive review must lock generic gap/rule {required}"
        );
    }

    for required in [
        "Hot-Path Classification",
        "No parser/package JavaScript, IPC, server validation",
        "`INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`",
        "`DECORATION_PAYLOAD_BUDGET_BYTES`",
        "`SYNTAX_CACHE_BUDGET_BYTES`",
        "`SCROLL_LAYOUT_RENDER_ADJACENT_P95_BUDGET_MS`",
        "Security and Authority Boundary",
        "language-server subprocess",
        "Rejected Implementation Shapes",
        "Do not add `RustDiagnosticProvider`",
        "Do not encode message, code, source, or severity into `style_token`",
        "Do not add a second parse/diagnostic scheduler",
        "Do not let diagnostics choose font roles",
        "Do not implement LSP process spawning",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.17 primitive review must preserve boundary {required}"
        );
    }
}

#[test]
fn tiered_syntax_engine_docs_are_indexed_and_security_boundaries_recorded() {
    let docs_index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");
    let primitive_index = primitives_index();
    let registry = primitives_registry();
    let parse = parse_update_strategy();
    let rendering = rendering_strategy();
    let security = package_security();
    let backlog = primitives_backlog();
    let package_guide = creating_packages_guide();
    let development =
        fs::read_to_string(repository_path("docs/development/launch-and-gui-smoke.md"))
            .expect("read launch and GUI smoke docs");

    for link in [
        "reference/packages/creating-packages.md",
        "reference/primitives/registry.md",
        "reference/primitives/parse-update-strategy.md",
        "reference/primitives/rendering-strategy.md",
        "reference/primitives/package-security.md",
        "development/launch-and-gui-smoke.md",
        "reference/packages/rust.md",
        "reference/packages/typescript.md",
        "reference/packages/javascript.md",
        "reference/packages/markdown.md",
    ] {
        assert!(
            docs_index.contains(link),
            "docs/index.md must link tiered syntax documentation {link}"
        );
    }

    for required in [
        "Phase 18.16 authoring contract: tiered syntax engine",
        "Tier 1 — native first-party",
        "Tier 2 — web-tree-sitter WASM",
        "Tier 3 — package JavaScript fallback",
        "TokenType` + `Modifiers",
        "setSyntaxEnginePreference",
        "clay.parse.open_failed",
        "Open is enqueue-only",
        "INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "SYNTAX_CACHE_BUDGET_BYTES",
        "grammars/PROVENANCE.md",
        "third-party grammar/native trust is deferred to Phase 23",
    ] {
        assert!(
            package_guide.contains(required),
            "package author guide must document tiered syntax contract `{required}`"
        );
    }

    for (source, name) in [
        (&primitive_index, "primitive index"),
        (&registry, "primitive registry"),
        (&parse, "parse strategy"),
        (&rendering, "rendering strategy"),
        (&security, "package security"),
        (&backlog, "primitive backlog"),
        (&development, "development smoke docs"),
    ] {
        for required in [
            "Phase 18.16",
            "Tier 1",
            "Tier 2",
            "Tier 3",
            "setSyntaxEnginePreference",
            "TokenType",
            "Modifiers",
            "package-root-confined",
            "clay.parse.open_failed",
        ] {
            assert!(
                source.contains(required),
                "{name} must document tiered syntax marker `{required}`"
            );
        }
    }

    for required in [
        "no runtime downloads",
        "no shell/package-manager",
        "native-library",
        "client-side JavaScript",
        "Third-party grammar/native trust remains out of scope until Phase 23",
        "does not grant filesystem",
    ] {
        assert!(
            security.contains(required) || package_guide.contains(required),
            "tiered syntax docs must record security boundary `{required}`"
        );
    }
}

#[test]
fn phase18_11_completion_provider_primitive_review_records_inventory_and_gaps() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let registry = primitives_registry();
    let backlog = primitives_backlog();
    let review = phase18_11_completion_provider_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.11-completion-provider-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.11 completion provider primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.11-completion-provider-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.11 completion provider primitive review"
    );

    for required in [
        "Existing Primitive Inventory",
        "Behavior manifests and autocomplete trigger metadata",
        "Client behavior routing and local edit path",
        "Command registry, command execution, and manual completion trigger",
        "Transient menu session and overlay projection",
        "Mode registry, fallback modes, and classification",
        "Parse coordinator and background work",
        "Syntax grammar registry and package provenance",
        "Decoration transport and payload budgets",
        "Package loading, manifest validation, and permissions",
        "Performance budgets and protocol codec",
        "Docs registry and wiki coverage",
        "`src/protocol/mod.rs`",
        "`src/client/behavior.rs`",
        "`src/perf/budgets.rs`",
        "`src/shell/transient_menu.rs`",
        "`src/server/parse_coordinator.rs`",
        "`src/server/syntax.rs`",
        "`src/packages/permissions.rs`",
        "`src/packages/service.rs`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.11 primitive review must record existing inventory text: {required}"
        );
    }

    for required in [
        "Generic Phase 18.11 Primitive Gaps",
        "### `CompletionRequest` / `CompletionResultSet` / `CompletionItem`",
        "### `CompletionProviderRegistry`",
        "### `CompletionCoordinator` (cancellable UI-reactive lane)",
        "### Behavior-manifest trigger routing and manual trigger",
        "### `TransientMenuSession` completion display/accept adapter",
        "### Built-in buffer-word provider",
        "### Clay JS completion provider registration API",
        "CompletionTriggerAndResult",
        "clay.completion.serverRegisterCompletionProvider",
        "completion-provider",
        "TransientMenuSession",
        "built-in buffer-word provider",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.11 primitive review must map generic completion gaps: {required}"
        );
    }

    for required in [
        "Hot-Path Classification",
        "Trigger classification / local edit",
        "Request enqueue",
        "Provider execution / result computation",
        "Menu render / selection / accept",
        "`ClientFirstPredictable`",
        "`UiReactivePriority`",
        "cancellable",
        "No synchronous IPC before local paint",
        "`COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES`",
        "`BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.11 primitive review must record hot-path split: {required}"
        );
    }

    for required in [
        "Security and Authority Boundary",
        "Completion provider permission required",
        "Inert result items only",
        "No new default authority",
        "Package provenance and loading boundary",
        "Trigger metadata is manifest data only",
        "completion-provider",
        "no filesystem access beyond already-open Clay-provided document snapshots, network, shell, AI, workspace index, WASM, raw ops, native widgets, client-side JavaScript, package-manager execution, package enable/disable, or side-effectful accept actions",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.11 primitive review must record permission/security boundary: {required}"
        );
    }

    for required in [
        "Do not add language-specific Rust completion branches",
        "Do not add a completion-only menu widget",
        "Do not run provider JavaScript or completion computation in Masonry paint, layout, keypress, pointer, scroll, or text-event handlers",
        "Do not block local typing/rendering on synchronous IPC",
        "Do not silently auto-load completion provider packages",
        "Do not implement LSP, AI, workspace-index, snippet-expansion, shell/tool, or network-backed providers in this phase",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.11 primitive review must reject unsafe/language-specific shape: {required}"
        );
    }

    assert!(
        registry.contains("CompletionTriggerAndResult"),
        "primitive registry must contain the CompletionTriggerAndResult row"
    );
    assert!(
        backlog.contains("CompletionTriggerAndResult"),
        "primitive backlog must contain the CompletionTriggerAndResult handoff row"
    );
    assert!(
        backlog.contains("Phase-18.11-completion"),
        "primitive backlog must record the Phase-18.11-completion priority tier"
    );
    let deferred_start = backlog
        .find("## Deferred")
        .expect("backlog must have a Deferred section");
    let deferred_end = backlog[deferred_start + 1..]
        .find("\n## ")
        .map(|rel| deferred_start + 1 + rel)
        .unwrap_or(backlog.len());
    assert!(
        !backlog[deferred_start..deferred_end].contains("CompletionTriggerAndResult"),
        "primitive backlog must move CompletionTriggerAndResult out of the Deferred section into Phase-18.11-completion"
    );
}

#[test]
fn phase18_12_workspace_discovery_primitive_review_records_inventory_and_gaps() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = phase18_12_workspace_discovery_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.12-workspace-discovery-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.12 primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.12-workspace-discovery-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.12 primitive review"
    );

    for required in [
        "Existing Primitive Inventory",
        "Workspace roots and file authority",
        "Shell layout and slots",
        "Command execution and transient menus",
        "Package UI components and action intents",
        "Document classification and fallback modes",
        "`src/server/workspace.rs::WorkspaceState`",
        "`WorkspaceState::add_root`",
        "`WorkspaceState::open_existing_file`",
        "`WorkspaceState::open_selected_file`",
        "`src/server/ops/workspace.rs::op_clay_workspace_list_roots`",
        "`runtime/js/workspace.ts::serverListWorkspaceRoots`",
        "`src/shell/layout.rs`",
        "`FixedSlotId::Left`",
        "`FixedSlotId::Bottom`",
        "`src/masonry_shell.rs::ClayShellWidget`",
        "`src/server/command_execution.rs::CommandExecutor`",
        "`src/shell/transient_menu.rs::TransientMenuSession`",
        "`src/server/control_center.rs`",
        "`runtime/js/ui.ts`",
        "`UiActionIntent`",
        "`src/packages/modes.rs::ModeRegistry`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.12 primitive review must record inventory text: {required}"
        );
    }

    for required in [
        "Generic Phase 18.12 Primitive Gaps",
        "### Server-owned workspace-root discovery",
        "### Bounded server file tree/list service",
        "### File browser UI is a composition, not a new primitive",
        "KNOWN_PROJECT_MARKERS",
        "discover_root_for_path",
        "add_explicit_user_grant",
        "FileListRequest",
        "FileListEntry",
        "FixedSlotId::Left",
        "TransientMenuSession",
        "CommandExecution",
        "clay.workspace.openFile",
        "clay.workspace.revealInTree",
        "composition",
        "not a new primitive",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.12 primitive review must map generic gaps and composition: {required}"
        );
    }

    for required in [
        "Hot-Path Classification",
        "Root discovery at startup",
        "Root discovery on open",
        "Directory listing",
        "Tree rendering",
        "Fuzzy filtering",
        "File activation",
        "Reveal-in-tree",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.12 primitive review must record hot-path split: {required}"
        );
    }

    for required in [
        "Rejected Implementation Shapes",
        "Do not add a `FileBrowserWidget`",
        "Do not implement client-side workspace discovery",
        "Do not allow packages to add workspace roots",
        "Do not implement a full nested `.gitignore` parser",
        "Do not pass raw client-chosen paths straight to an open op",
        "Do not make the file tree a package contribution",
        "Do not add file-browser-specific Rust rendering branches",
        "File browser UI is Clay-owned",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.12 primitive review must reject file-browser-specific shapes: {required}"
        );
    }

    for required in [
        "Security and Authority Boundary",
        "no broad client or package filesystem authority",
        "Explicit user grants are the only path that broadens authority",
        "Directory listing is scoped to known workspace roots",
        "Packages cannot list arbitrary paths",
        "No package may add roots, markers, ignore rules, or listing scopes",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.12 primitive review must record security boundary: {required}"
        );
    }
}

#[test]
fn end_to_end_file_browser_workflow_primitive_review_records_inventory_and_gaps() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = end_to_end_file_browser_workflow_primitive_review();

    assert!(
        wiki_index.contains("modules/end-to-end-file-browser-workflow-primitive-review.md"),
        "docs/wiki/index.md must link the end-to-end file browser workflow primitive review"
    );
    assert!(
        primitive_architecture.contains("end-to-end-file-browser-workflow-primitive-review.md"),
        "primitive architecture wiki must link the end-to-end file browser workflow primitive review"
    );

    for required in [
        "Existing Primitive Inventory",
        "Workspace roots and bounded listing",
        "File browser SDUI and command execution",
        "Client UI prompts and selected-file authority",
        "Language activation and package behavior",
        "Editor selection state",
        "`WorkspaceState`",
        "`FileListRequest` / `FileListPage`",
        "`FileBrowserState::to_sdui_tree`",
        "`CommandExecutor::execute_workspace`",
        "`TransientMenuSession`",
        "`clay.documents.clientOpenFileDialog`",
        "`FileOpenCapabilityPool`",
        "`classify_open_document`",
        "`@clay/rust`, `@clay/typescript`, and `@clay/javascript`",
        "`SelectionState`",
        "`EditorSurface`",
        "`EditorBuffer::text_range`",
    ] {
        assert!(
            review.contains(required),
            "end-to-end primitive review must record inventory text: {required}"
        );
    }

    for required in [
        "Generic Workflow Primitive Gaps",
        "### Selected-folder client UI grant",
        "### File-browser directory navigation",
        "### Generic open-document follow-ups",
        "### Client copy-selection clipboard write",
        "clay.workspace.clientOpenFolderDialog",
        "single-use selected-path capability",
        "Directory rows must route to a directory-navigation command",
        "reuse `WorkspaceState::list_directory`",
        "Promote selected-file-only follow-ups into a generic open-document helper",
        "No paste, cut, clipboard read, server clipboard op, package clipboard op, or arbitrary clipboard write",
    ] {
        assert!(
            review.contains(required),
            "end-to-end primitive review must map generic gaps: {required}"
        );
    }

    for required in [
        "Hot-Path Classification",
        "Folder picker",
        "Selected-folder grant",
        "Directory listing/navigation",
        "File opening",
        "Language activation",
        "Clipboard copy",
        "Editor typing/paint/layout",
        "No filesystem scans, native dialogs, IPC waits, JavaScript, full-document serialization, shell, network, AI, or clipboard work",
    ] {
        assert!(
            review.contains(required),
            "end-to-end primitive review must record hot-path split: {required}"
        );
    }

    for required in [
        "Rejected Implementation Shapes",
        "Do not add `FileBrowserWidget`",
        "Do not implement client-side workspace scans or file listing",
        "Do not let packages add workspace roots",
        "Do not pass raw client-chosen paths directly",
        "Do not shell out for native folder picking",
        "Do not run package JavaScript, parser work, filesystem listing, modal UI, or clipboard work",
        "Do not add paste/cut",
    ] {
        assert!(
            review.contains(required),
            "end-to-end primitive review must reject workflow-specific shapes: {required}"
        );
    }

    for required in [
        "Security and Authority Boundary",
        "no broad client or package filesystem authority",
        "Server owns workspace roots",
        "Client owns native prompts",
        "Selected folder/file paths are untrusted strings",
        "Packages cannot list arbitrary paths",
        "Clipboard support is write-only for the current editor selection",
    ] {
        assert!(
            review.contains(required),
            "end-to-end primitive review must record security boundary: {required}"
        );
    }
}

#[test]
fn manual_file_browser_workflow_bugfix_primitive_review_records_root_causes() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = manual_file_browser_workflow_bugfix_primitive_review();

    assert!(
        wiki_index.contains("modules/manual-file-browser-workflow-bugfix-primitive-review.md"),
        "docs/wiki/index.md must link the manual file browser bugfix primitive review"
    );
    assert!(
        primitive_architecture.contains("manual-file-browser-workflow-bugfix-primitive-review.md"),
        "primitive architecture wiki must link the manual file browser bugfix primitive review"
    );

    for required in [
        "Existing Primitive Inventory",
        "Keybinding route and behavior manifests",
        "Client UI command route",
        "FileBrowserState and bounded workspace APIs",
        "StaticSduiState validation",
        "SduiNativeState rendering and local action regions",
        "PaneSlotLayout and editor region computation",
        "EditorSurface visual scroll and paint chrome",
        "Open-document follow-ups",
        "`KeyRoutingOverride`",
        "`ClientUiCommandRoute`",
        "`FileBrowserState`",
        "`StaticSduiState`",
        "`SduiNativeState`",
        "`PaneSlotLayout`",
        "`EditorSurface`",
        "`WorkspaceState`",
    ] {
        assert!(
            review.contains(required),
            "manual file browser bugfix review must record inventory text: {required}"
        );
    }

    for required in [
        "Generic Fix Map",
        "`Ctrl+Shift+O` does not open folder picker",
        "Nested `src/main.rs` fails with `ActionSourceMismatch`",
        "Browser actions fail after Markdown activation",
        "`clay.parse.open_activation_timeout` hangs workflow",
        "Second file does not replace first",
        "Editor overlaps file browser",
        "Purple circle and visible card padding",
        "File browser cannot scroll",
        "Main text area lacks scroller",
        "Row ID and action source item ID match exactly",
        "Open-time package outputs cannot replace Clay-owned workspace browser validation state",
        "Visible Clay-owned left slot reserves editor region independent of the active document ID",
    ] {
        assert!(
            review.contains(required),
            "manual file browser bugfix review must map root causes: {required}"
        );
    }

    for required in [
        "Rejected Approaches",
        "Do not add Markdown-specific Rust branches",
        "Do not add Rust/TypeScript/JavaScript-specific file-open branches",
        "Do not relax `StaticSduiState::validate_action`",
        "Do not route file-browser scrolling through server relisting",
        "Do not add hidden JSON/TOML/ad hoc config keys",
        "raw `Deno.core.ops`",
        "Hot-Path and Security Boundaries",
        "Client hot paths remain local",
        "Server owns workspace/file authority",
        "Client owns native rendering/input state",
        "Packages still cannot read clipboard contents",
    ] {
        assert!(
            review.contains(required),
            "manual file browser bugfix review must record boundaries: {required}"
        );
    }
}

#[test]
fn phase18_13_git_discovery_primitive_review_records_inventory_and_gaps() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = phase18_13_git_discovery_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.13-git-discovery-primitive-review.md"),
        "wiki index must link the Phase 18.13 Git discovery primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.13-git-discovery-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.13 primitive review"
    );

    for required in [
        "Existing Primitive Inventory",
        "Workspace roots and file authority",
        "Command execution and Control Center",
        "Transient menu and picker UI",
        "Package loading and first-party package defaults",
        "Package UI, status items, panels, and action intents",
        "Configuration and documentation registry",
        "Existing process/timeout helpers",
        "`src/server/workspace.rs::WorkspaceState`",
        "`WorkspaceRootDiscovery`",
        "`BoundedFileListService`",
        "`runtime/js/workspace.ts::serverListWorkspaceRoots`",
        "`src/server/command_execution.rs::CommandExecutor`",
        "`src/server/control_center.rs::ControlCenter`",
        "`src/shell/transient_menu.rs::TransientMenuSession`",
        "`runtime/js/packages.ts::loadPackage`",
        "`@clay/git`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.13 primitive review must record inventory text: {required}"
        );
    }

    for required in [
        "What `@clay/git` Can Achieve With Existing Primitives",
        "Generic Phase 18.13 Primitive Gaps",
        "### `GitDiscoveryService`",
        "### `GitStatusCache`",
        "### `clay:git` read-only facades",
        "GitStatusSnapshot",
        "GitDiscoveryCommand",
        "RepositoryRoot",
        "StatusShort",
        "serverListGitStatuses",
        "serverRefreshGitStatus",
        "workspaceRootId",
        "closed command enum",
        "per-workspace cache",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.13 primitive review must map generic Git gaps: {required}"
        );
    }

    for required in [
        "Hot-Path Classification",
        "Detect workspace roots",
        "Run `git` CLI",
        "Read Git status for UI",
        "Explicit refresh",
        "Status item/panel render",
        "Branch/action picker filtering",
        "Picker/action activation",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.13 primitive review must record hot-path split: {required}"
        );
    }

    for required in [
        "Rejected Implementation Shapes",
        "Do not add a Git-specific native widget",
        "Do not let `@clay/git` spawn `git`",
        "Do not add a generic shell API",
        "Do not accept raw Git subcommands",
        "Do not auto-load `@clay/git`",
        "Do not implement checkout",
        "Do not use package-manager process code as a runtime shell escape hatch",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.13 primitive review must reject unsafe/overbuilt Git shapes: {required}"
        );
    }

    for required in [
        "Security and Authority Boundary",
        "read-only Git status authority only",
        "Server-owned, read-only `git` CLI calls through a closed command table",
        "`cwd` rooted in a known `WorkspaceRootId`",
        "Not allowed:",
        "Arbitrary shell execution",
        "Network/remotes/fetch/push/pull",
        "Mutating Git operations",
        "Package process authority",
        "Client-side Git execution",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.13 primitive review must record security boundary: {required}"
        );
    }
}

#[test]
fn phase18_14_language_package_expansion_primitive_review_records_inventory_and_gaps() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = phase18_14_language_package_expansion_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.14-language-package-expansion-primitive-review.md"),
        "wiki index must link the Phase 18.14 language package expansion primitive review"
    );
    assert!(
        primitive_architecture
            .contains("phase18.14-language-package-expansion-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.14 language package expansion primitive review"
    );

    for required in [
        "Existing Primitive Inventory",
        "Document classification and major-mode activation",
        "Behavior manifests and text transforms",
        "Command declaration and execution",
        "Syntax grammar contribution",
        "Completion trigger and result providers",
        "Parse handler bridge and incremental parse updates",
        "Decoration transport",
        "Package UI contributions",
        "Package configuration and layout overrides",
        "Package loading and provenance",
        "Workspace discovery and Git status",
        "`src/packages/modes.rs::ModeRegistry`",
        "`runtime/js/modes.ts`",
        "`runtime/js/commands.ts`",
        "`runtime/js/completion.ts`",
        "`runtime/js/syntax.ts`",
        "`runtime/js/parse.ts`",
        "`runtime/js/decorations.ts`",
        "`runtime/js/ui.ts`",
        "`runtime/js/configuration.ts`",
        "`runtime/js/packages.ts`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.14 primitive review must record existing inventory text: {required}"
        );
    }

    for required in [
        "Generic Phase 18.14 Primitive Gaps",
        "### Mode-scoped command/action metadata helper",
        "### Language-package behavior-manifest presets",
        "### Parse-handler lifecycle for language modes",
        "### Completion provider for language keywords/snippets",
        "### Symbol/outline panel contribution",
        "### Status-item contribution",
        "active syntax grammar separate from active major mode",
        "@clay/rust",
        "@clay/typescript",
        "@clay/javascript",
        "grammar-only packages",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.14 primitive review must map generic language-package gaps: {required}"
        );
    }

    for required in [
        "Hot-Path Classification",
        "Package load / mode registration",
        "Document open / reload / reclassification",
        "Background parse / completion work",
        "Command execution",
        "Configuration / layout override evaluation",
        "Paint/text-event/key hot path",
        "No package JavaScript",
        "synchronous IPC in Masonry",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.14 primitive review must record hot-path split: {required}"
        );
    }

    for required in [
        "Security and Authority Boundary",
        "No language-specific Rust branches",
        "No arbitrary file IO",
        "No client-side JavaScript",
        "No direct Masonry/native widget access",
        "LSP",
        "full language-server protocol integration",
        "workspace-wide symbol indexes",
        "Rejected Implementation Shapes",
        "Do not add Rust server/client branches",
        "Do not implement language-specific parser branches",
        "Do not create language-specific native widgets",
        "Do not run language package JavaScript in Masonry paint",
        "Do not auto-load language packages",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.14 primitive review must reject unsafe/language-specific shape: {required}"
        );
    }
}

#[test]
fn phase18_13_git_wiki_pages_document_final_implementation() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let discovery_wiki = fs::read_to_string(repository_path(
        "docs/wiki/modules/git-discovery-service.md",
    ))
    .expect("read git discovery service wiki");
    let package_wiki = fs::read_to_string(repository_path("docs/wiki/modules/package-git.md"))
        .expect("read package-git wiki");

    assert!(
        wiki_index.contains("modules/git-discovery-service.md"),
        "wiki index must link the Git Discovery Service implementation page"
    );
    assert!(
        wiki_index.contains("modules/package-git.md"),
        "wiki index must link the first-party @clay/git package implementation page"
    );

    for required in [
        "GitDiscoveryService",
        "GitStatusCache",
        "closed command table",
        "cwd must canonicalize under a known workspace root",
        "coalesces",
        "Notify",
        "ETXTBSY",
    ] {
        assert!(
            discovery_wiki.contains(required),
            "git discovery service wiki must document `{required}`"
        );
    }

    for required in [
        "loadPackage(\"@clay/git\")",
        "permissions: []",
        "serverListGitStatuses",
        "clay:git",
        "clay:sdui",
        "git.status",
        "no action targets",
        "Future Mutation Authority",
        "clay.git.listStatuses",
        "clay.git.refreshStatus",
    ] {
        assert!(
            package_wiki.contains(required),
            "package-git wiki must document `{required}`"
        );
    }
}

#[test]
fn phase18_12_workspace_file_browser_wiki_documents_implementation() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let workspace_wiki = workspace_file_browser_wiki();
    let server_workspace = fs::read_to_string(repository_path(
        "docs/wiki/modules/server-file-workspace.md",
    ))
    .expect("read server workspace wiki");
    let command_registry =
        fs::read_to_string(repository_path("docs/wiki/modules/command-registry.md"))
            .expect("read command registry wiki");
    let maintenance = fs::read_to_string(repository_path(
        "docs/wiki/modules/maintenance-validation.md",
    ))
    .expect("read maintenance validation wiki");

    assert!(
        wiki_index.contains("modules/workspace-file-browser.md"),
        "wiki index must link the Phase 18.12 workspace file-browser implementation page"
    );

    for required in [
        "WorkspaceState::discover_root_for_path",
        "WorkspaceState::list_directory",
        "serverListDirectory",
        "serverCreateListingCancelToken",
        "FileBrowserState::to_sdui_tree",
        "TransientMenuSession",
        "CommandExecutor::execute_workspace",
        "selected-file grants",
        "no broad client or package filesystem authority",
        "Linux is the primary validation platform",
    ] {
        assert!(
            workspace_wiki.contains(required),
            "workspace file-browser wiki must document `{required}`"
        );
    }

    assert!(
        server_workspace
            .contains("Phase 18.12 bounded listing uses `list_directory(FileListRequest)`")
            && server_workspace.contains("KNOWN_PROJECT_MARKERS"),
        "server workspace wiki must document Phase 18.12 root discovery and bounded listing"
    );
    assert!(
        command_registry.contains(
            "validates that the document is open through `WorkspaceState::document_metadata`"
        ),
        "command registry wiki must document reveal command validation"
    );
    assert!(
        maintenance.contains("Linux is the required host platform")
            && maintenance.contains("Windows remains a long-term target"),
        "maintenance wiki must document Linux-primary, Windows-long-term validation policy"
    );
}

#[test]
fn phase18_5_markdown_replan_primitive_review_records_existing_inventory() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let review = phase18_5_markdown_replan_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18.5-markdown-replan-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.5 Markdown replan primitive review"
    );
    assert!(
        primitive_architecture.contains("phase18.5-markdown-replan-primitive-review.md"),
        "primitive architecture wiki must link the Phase 18.5 Markdown replan primitive review"
    );

    for required in [
        "Existing Generic Primitive Inventory",
        "`WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`",
        "`src/shell/layout.rs`, `src/masonry_shell.rs`",
        "`PanelContribution` (`serverRegisterPanelContribution`)",
        "`ComponentContribution` (`serverRegisterComponentContribution`)",
        "`TransientOverlayContribution` (`serverRegisterTransientOverlayContribution`)",
        "`PackageThemeTokenDeclaration` (`serverRegisterThemeToken`)",
        "`PackageInputContribution` (`serverRegisterInputContribution`)",
        "`PackageUiStateScope` (`serverRegisterUiStateScope`)",
        "`PackageLayoutOverride` (`serverSetLayoutOverride`)",
        "`PackageOwnedConfiguration` (`setPackageOption`)",
        "`MajorModeActivation` and `DocumentClassification`",
        "`CommandDeclaration` and behavior-manifest commands",
        "`serverRegisterParseHandler` and parse coordinator",
        "`serverPublishDecorations` and decoration transport",
        "`serverLoadPackage(packageJson)`",
        "`loadPackage(\"@clay/*\")` (`clay.packages.loadPackage`)",
        "Implemented by Plan 029",
        "Open-document activation (`clientOpenFileDialog` binding, `open_selected_file`, `open_document_followup_messages`)",
        "`~/.config/clay/init.js` configuration runtime",
        "Package manifest, permissions, conflict, provenance validation",
        "Clay JS API inventory, docs registry, generated registry",
        "Structural observability",
        "`src/packages/modes.rs`, `src/server/ops/modes.rs`, `runtime/js/modes.ts`",
        "`runtime/js/parse.ts`, `src/server/parse_coordinator.rs`, `packages/markdown/dist/parser.js`",
        "`runtime/js/decorations.ts`, `src/server/ops/decorations.rs`",
        "`runtime/js/packages.ts`, `src/server/ops/packages.rs`, `src/packages/service.rs`",
        "`bindKey(\"Ctrl+O\", \"clay.documents.clientOpenFileDialog\", { scope: \"editor\" })`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.5 Markdown replan primitive review must record existing inventory text: {required}"
        );
    }
}

#[test]
fn phase18_5_markdown_replan_primitive_review_maps_markdown_to_generic_primitives() {
    let review = phase18_5_markdown_replan_primitive_review();

    for required in [
        "Markdown Needs Mapped to Generic Primitives",
        "Markdown need                            -> Generic primitive (status)",
        "Main editor placement                  -> PaneSlotLayout.main (implemented)",
        "Optional preview panel                 -> PanelContribution targeting `right` slot (implemented)",
        "No default side panel                  -> Do not publish PanelContribution by default (configuration choice)",
        "Mode classification                    -> DocumentClassification (implemented)",
        "Major-mode activation + behavior       -> MajorModeActivation + BehaviorManifest (implemented)",
        "Package commands and key bindings      -> CommandDeclaration + behavior-manifest keymaps (implemented)",
        "Client-first editor rules              -> ContinueLineMarkers / PairRule / PreserveFenceBodyIndent (implemented)",
        "Background parse handler               -> serverRegisterParseHandler (implemented)",
        "Syntax decorations                     -> serverPublishDecorations (implemented)",
        "User configuration override            -> setPackageOption / serverSetLayoutOverride (implemented)",
        "Theme tokens for preview styling       -> PackageThemeTokenDeclaration + ThemeTokenResolver (implemented)",
        "Selected-file open activation          -> bindKey(\"Ctrl+O\", \"clay.documents.clientOpenFileDialog\") + open_selected_file (implemented)",
        "One-line end-user package loading      -> loadPackage(\"@clay/markdown\") (implemented by Plan 029)",
        "Every Markdown need now maps onto an implemented generic primitive, including one-line end-user package loading, which Plan 029 closed",
        "No Markdown-specific Rust editor/parser/render/shell branch is required",
        "Hot-Path Classification",
        "Configuration/load time",
        "Package validation time",
        "Explicit command/UI update time",
        "Background parse/decor time",
        "Behavior-manifest update work",
        "Editor hot-path work",
        "The no-hot-path-package-JS rule is preserved",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.5 Markdown replan primitive review must map Markdown needs to generic primitives: {required}"
        );
    }
}

#[test]
fn phase20_markdown_plan_assumes_generic_load_package_is_available() {
    let review = phase18_5_markdown_replan_primitive_review();

    // Plan 029 closed the former `loadPackage` gap. The review must record the
    // gap as closed, reject any Markdown-specific loader, and preserve the
    // package-root confinement rationale while noting that Plan 035 supersedes
    // the first-party-only implementation limit.
    for required in [
        "Generic Phase 18.5 Primitive Gaps — `loadPackage` (closed by Plan 029)",
        "Status (2026-06-16): CLOSED.",
        "Plan 029 closed this gap.",
        "the constrained first-party `loadPackage(\"@clay/*\")` resolver is implemented",
        "Candidate public API target: `clay.packages.loadPackage` / `loadPackage(\"@clay/markdown\")`",
        "`runtime/js/packages.ts`",
        "`op_clay_packages_load_package_by_specifier` in `src/server/ops/packages.rs`",
        "first-party-only scope is superseded by the unified package authority decision",
        "Plan 029 deliberately implemented a constrained `@clay/*` resolver for Phase 18.6",
        "Plan 035 replaces it with source-aware loading for user-authorized packages",
        "No Markdown-specific loader primitive is proposed",
        "the Markdown package does not require a `MarkdownLoader`, `LoadMarkdown`, or any `if package == \"@clay/markdown\"` Rust branch",
        "Do not add `MarkdownLoader`, `MarkdownLoadEntry`, `MarkdownSidebar`, `MarkdownPreviewPanel`, `MarkdownModeDefault`, `MarkdownPanelVisibility`, `MarkdownPaneSelector`, or any `if mode == \"markdown\"` / `if package == \"@clay/markdown\"` Rust editor/parser/render/shell branch",
        "Do not implement the one-line loader as a Markdown-specific resolver",
        "Do not keep the inline package manifest object (`const markdownPackage = { ... }`), the manual per-facade registration imports",
        "Do not present `serverLoadPackage(packageJson)` as the ordinary end-user one-line setup",
        "None of these primitives grant filesystem (outside already-open document content and the config root), network, shell, extension loading, AI mutation, workspace mutation, package-control, package-manager execution, WASM",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.5 Markdown replan primitive review must record that Plan 029 closed the loadPackage gap and reject Markdown-specific loaders: {required}"
        );
    }
}

fn replan_023_plan() -> String {
    fs::read_to_string(repository_path(
        "plans/023-Phase20-Markdown-Mode-End-User-Loading-and-UI-Cleanup.md",
    ))
    .expect("read replanned Plan 023")
}

#[test]
fn replan_023_has_no_markdown_specific_rust_ui_branches() {
    let plan = replan_023_plan();

    // The replanned plan must explicitly reject Markdown-specific Rust
    // editor/parser/render/shell branches and fixture-only UI assumptions.
    for required in [
        "Rejected Implementation Shapes",
        "Do not add `MarkdownLoader`, `MarkdownSidebar`, `MarkdownPreviewPanel`, `MarkdownPaneLayout`, `MarkdownModeDefault`",
        "if mode == \"markdown\"",
        "if package == \"@clay/markdown\"",
        "The one-line loader must be a generic `loadPackage(specifier)`",
        "Do not keep the inline package manifest object (`const markdownPackage = { ... }`)",
        "fixture-only `publishTree(...)` call",
        "Do not publish a default fixed or transient Markdown panel on load",
        "No Markdown-specific Rust editor/parser/render/shell branch is introduced by any task below",
        "no Markdown-specific Rust parser, editor, or shell branch",
    ] {
        assert!(
            plan.contains(required),
            "replanned Plan 023 must reject Markdown-specific Rust UI branches and fixture-only UI assumptions: {required}"
        );
    }
}

#[test]
fn replan_023_references_generic_clay_ui_primitives() {
    let plan = replan_023_plan();

    // Every task must reference generic clay:ui, shell, configuration, and
    // package-loading primitives promoted in Phases 18.1-18.4 instead of
    // Markdown-specific UI assumptions.
    for required in [
        "`PanelContribution`",
        "`ComponentContribution`",
        "`TransientOverlayContribution`",
        "`PackageThemeTokenDeclaration`",
        "`PackageInputContribution`",
        "`PackageUiStateScope`",
        "`PackageLayoutOverride`",
        "`PackageOwnedConfiguration`",
        "`WorkingAreaLayout`, `PaneSplitTree`, `PaneSlotLayout`",
        "`PaneSlotLayout`",
        "mandatory `main` slot",
        "`setPackageOption`",
        "`serverSetLayoutOverride`",
        "`serverRegisterPanelContribution`",
        "`MajorModeActivation`",
        "`DocumentClassification`",
        "`CommandDeclaration`",
        "`serverRegisterParseHandler`",
        "`serverPublishDecorations`",
        "`defaultVisibility: \"hidden\"`",
        "targeting a Clay slot such as `right`",
        "consumes the generic shell/package UI primitives promoted in",
    ] {
        assert!(
            plan.contains(required),
            "replanned Plan 023 must reference generic clay:ui / shell / configuration primitives: {required}"
        );
    }
}

#[test]
fn replan_023_preserves_one_line_load_and_ctrl_o_separation() {
    let plan = replan_023_plan();

    // The replanned plan must preserve the one-line package loading target and
    // the explicit bindKey("Ctrl+O", ...) separation from package loading.
    for required in [
        "`loadPackage(\"@clay/markdown\")`",
        "import { loadPackage } from \"clay:packages\";",
        "await loadPackage(\"@clay/markdown\")",
        "bindKey(\"Ctrl+O\", \"clay.documents.clientOpenFileDialog\", { scope: \"editor\" })",
        "Keep `bindKey(\"Ctrl+O\", \"clay.documents.clientOpenFileDialog\", { scope: \"editor\" })` as the user-configured Windows file-open binding, separate from package loading",
        "separation is preserved",
        "package-owned fallback",
        "Phase 18.5 Replan",
        "plans/028-Phase18.5-Replan-Markdown-End-User-Loading-After-Shell-Layout-Work.md",
    ] {
        assert!(
            plan.contains(required),
            "replanned Plan 023 must preserve the one-line loadPackage target and the explicit Ctrl+O bindKey separation: {required}"
        );
    }
}

#[test]
fn shell_layout_strategy_doc_linked_from_docs_indexes() {
    let docs_index = fs::read_to_string(repository_path("docs/index.md")).expect("read docs index");
    let primitive_index = primitives_index();
    let package_guide = fs::read_to_string(repository_path(
        "docs/reference/packages/creating-packages.md",
    ))
    .expect("read package guide");

    for (name, text, link) in [
        (
            "docs/index.md",
            docs_index.as_str(),
            "reference/primitives/shell-layout-strategy.md",
        ),
        (
            "docs/reference/primitives/index.md",
            primitive_index.as_str(),
            "shell-layout-strategy.md",
        ),
        (
            "docs/reference/packages/creating-packages.md",
            package_guide.as_str(),
            "../primitives/shell-layout-strategy.md",
        ),
    ] {
        assert!(
            text.contains(link),
            "{name} must link the shell/layout architecture reference {link}"
        );
    }
}

#[test]
fn shell_layout_strategy_defines_required_vocabulary() {
    let strategy = shell_layout_strategy();

    for required in [
        "Application Shell",
        "Working Area",
        "Pane/Split Tree",
        "pane/window layout",
        "mandatory `main`",
        "`left`",
        "`right`",
        "`top`",
        "`bottom`",
        "Fixed Panels",
        "Transient Panels and Overlays",
        "Components and Elements",
        "Action Intents",
        "Package State Scopes",
        "`package-global`",
        "`user-config`",
        "`workspace`",
        "`document`",
        "`pane`",
        "`component`",
        "`transient-overlay`",
        "Style and Theme Tokens",
        "WorkingArea",
        "PaneSplitTree",
        "PaneSlotLayout",
        "PanelContribution",
        "ComponentContribution",
        "TransientOverlayContribution",
        "PackageThemeTokenDeclaration",
    ] {
        assert!(
            strategy.contains(required),
            "shell layout strategy must define required vocabulary: {required}"
        );
    }
}

#[test]
fn shell_layout_strategy_records_masonry_boundary_and_prohibitions() {
    let strategy = shell_layout_strategy();

    for required in [
        "Masonry remains an implementation substrate",
        "not stable public package APIs",
        "RenderRoot",
        "Widget",
        "Split",
        "Flex",
        "Grid",
        "ZStack",
        "Portal",
        "typed widget properties",
        "Masonry actions",
        "no package logic runs during Masonry paint, layout, pointer, scroll, keypress, or text-event handlers",
        "raw CSS",
        "arbitrary client JavaScript",
        "raw `Deno.core.ops`",
        "direct Masonry widget handles",
        "Masonry widget constructors",
        "native widget IDs",
        "native widget handles",
        "Vello callbacks",
        "Parley callbacks",
        "filesystem",
        "network",
        "shell",
        "AI mutation",
        "WASM execution",
        "unregistered action targets",
    ] {
        assert!(
            strategy.contains(required),
            "shell layout strategy must record Masonry boundary/prohibition text: {required}"
        );
    }
}

#[test]
fn shell_layout_strategy_records_precedence_order() {
    let strategy = shell_layout_strategy();
    let package_guide = fs::read_to_string(repository_path(
        "docs/reference/packages/creating-packages.md",
    ))
    .expect("read package guide");
    let security = package_security();

    let precedence = [
        "1. Clay shell safety invariants and hard prohibitions",
        "2. User configuration through documented Clay JS APIs",
        "3. Active major mode layout defaults",
        "4. Compatible minor mode contributions",
        "5. Global package contributions",
        "6. Package fallback/defaults",
    ];

    let mut previous = 0;
    for marker in precedence {
        let position = strategy
            .find(marker)
            .unwrap_or_else(|| panic!("shell layout strategy missing precedence marker {marker}"));
        assert!(
            position >= previous,
            "shell layout strategy precedence marker {marker} is out of order"
        );
        previous = position;

        for (name, text) in [
            ("package guide", package_guide.as_str()),
            ("package security", security.as_str()),
        ] {
            assert!(
                text.contains(marker),
                "{name} must repeat shell/layout precedence marker {marker}"
            );
        }
    }

    for required in [
        "No package wins a shell/layout conflict by load order alone",
        "duplicate slots",
        "duplicate component IDs",
        "duplicate overlay IDs",
        "duplicate commands/actions",
        "undeclared permissions",
        "unregistered action targets",
        "unsupported state scopes",
        "unknown style tokens",
        "oversize layout/component/state payloads",
        "package/user override bypass attempts",
    ] {
        assert!(
            security.contains(required),
            "package security must record shell/layout conflict diagnostic category {required}"
        );
    }
}

#[test]
fn shell_layout_strategy_records_state_scopes_and_action_validation() {
    let strategy = shell_layout_strategy();
    let package_guide = fs::read_to_string(repository_path(
        "docs/reference/packages/creating-packages.md",
    ))
    .expect("read package guide");

    for scope in [
        "`package-global`",
        "`user-config`",
        "`workspace`",
        "`document`",
        "`pane`",
        "`component`",
        "`transient-overlay`",
    ] {
        assert!(
            strategy.contains(scope),
            "shell layout strategy must record planned state scope {scope}"
        );
        assert!(
            package_guide.contains(scope.trim_matches('`')),
            "package guide must record author-facing state scope {scope}"
        );
    }

    for required in [
        "State keys and IDs should be package-prefixed",
        "Unsupported state scopes",
        "hidden state keys",
        "command IDs are registered before UI action targets become active",
        "UI actions are inert command intents",
        "action arguments are bounded primitive data",
        "callbacks, raw op names, native handles, executable code",
        "stale action intents are rejected or disabled",
    ] {
        assert!(
            package_guide.contains(required),
            "package guide must record state/action validation text: {required}"
        );
    }

    for required in [
        "Input and Action Contract",
        "Every action target must resolve to a registered command before the UI declaration becomes active",
        "Package command IDs must use the package prefix",
        "Action arguments must be bounded primitive data",
        "callbacks, raw op names, native handles, filesystem paths",
        "Stale action intents are rejected or disabled",
    ] {
        assert!(
            strategy.contains(required),
            "shell layout strategy must record input/action contract text: {required}"
        );
    }
}

#[test]
fn shell_layout_strategy_records_style_token_contract() {
    let strategy = shell_layout_strategy();
    let package_guide = fs::read_to_string(repository_path(
        "docs/reference/packages/creating-packages.md",
    ))
    .expect("read package guide");
    let security = package_security();

    for required in [
        "Style and Theme Token Contract",
        "typed tokens and typed component style variables",
        "Package-owned token names must use the package prefix",
        "optional fallback token of the same type",
        "Unknown style tokens",
        "type-incompatible fallbacks",
        "raw CSS",
        "native renderer callbacks",
        "raw colors without a typed token contract",
    ] {
        assert!(
            strategy.contains(required),
            "shell layout strategy must record style/theme token contract text: {required}"
        );
    }

    for required in [
        "Package-owned token names should use the package prefix",
        "Token declarations should include a semantic description, a token type, an optional same-type fallback, and package provenance",
        "Unknown style tokens",
        "duplicate package token names",
        "type-incompatible fallbacks",
        "native renderer callbacks",
        "raw colors without a typed token contract",
    ] {
        assert!(
            package_guide.contains(required),
            "package guide must record style/theme authoring text: {required}"
        );
    }

    for required in [
        "Unknown style/theme token",
        "unknown style tokens",
        "type-incompatible token fallbacks",
        "raw CSS",
        "raw style strings",
        "Vello/Parley/native renderer callbacks",
    ] {
        assert!(
            security.contains(required),
            "package security must record style/theme rejection text: {required}"
        );
    }
}

#[test]
fn creating_packages_docs_cover_shell_layout_contract() {
    let guide = creating_packages_guide();

    for required in [
        "Status markers used in this guide",
        "Implemented/runtime-backed",
        "Implemented/internal runtime",
        "Planned/target",
        "Fixture-only/current limitation",
        "Masonry is Clay's internal widget/layout/rendering substrate, not a package author API",
        "Performance authoring rule",
        "package load, package validation, configuration evaluation, explicit command handling, or explicit UI update time",
        "Typing, Masonry paint, Masonry layout, scroll, pointer, keypress, and text-event paths",
        "Phase 18.2 shell/layout runtime and Phase 18.3 slot-aware package UI",
        "Clay-owned `ClayShellWidget` root above `EditorWidget`",
        "Internal `WorkingAreaLayout` state",
        "Internal `PaneSplitTree` state",
        "Internal `PaneSlotLayout` state",
        "working area",
        "Pane/split tree",
        "`main` — mandatory",
        "`left`",
        "`right`",
        "`top`",
        "`bottom`",
        "fixed panel",
        "transient panel",
        "Clay components",
        "UI actions are inert command intents",
        "Target state scopes",
        "Styling and Themes",
        "Expected shell/layout/package guide updates by phase",
        "Phase 18.2",
        "Phase 18.3",
        "Phase 18.4",
        "Phase 18.5",
    ] {
        assert!(
            guide.contains(required),
            "package guide must cover shell/layout authoring contract text: {required}"
        );
    }
}

#[test]
fn creating_packages_docs_mark_examples_by_status() {
    let guide = creating_packages_guide();

    for required in [
        "**Implemented end-user default:** users explicitly load packages from `~/.config/clay/init.js`",
        "Current implemented package API status",
        "serverLoadPackage(packageJson)",
        "Implemented package-record validation helper",
        "not an end-user install, enable/disable, package-manager, or package-code execution wrapper",
        "**Implemented default loader shape**",
        "**Implemented/runtime-backed SDUI example**",
        "clay.sdui.publishTree",
        "The current `clay:sdui` helpers publish bounded inert node trees through server validation",
        "`clay:ui` inventory targets for the shell/layout contract",
        "clay.ui.serverRegisterPanelContribution",
        "clay.ui.serverSetLayoutOverride",
        "**Implemented/runtime-backed Phase 18.3 slot panel and token example:**",
        "**Implemented/runtime-backed Phase 18.3 transient overlay example:**",
        "**Implemented/runtime-backed Phase 18.4 input contribution example:**",
        "**Planned configuration examples**",
        "clay.configuration.setPackageOption` and `clay.ui.serverSetLayoutOverride` are inventory stubs,",
        "not public runtime-backed shell/layout configuration APIs in Phase 18.3",
        "**Implemented/runtime-backed theme-token declaration example**",
        "PackageThemeTokenDeclaration` / `clay.ui.serverRegisterThemeToken`",
        "**Implemented/runtime-backed component style example**",
        "**Implemented/runtime-backed default user setup**",
        "fixtures are validation tools, not the long-term user setup or shell/layout authoring convention",
        "**Implemented/package-owned fallback alias** (Phase 18.5, retained after `loadPackage` shipped)",
        "**Implemented/runtime-backed Phase 18.5 no-default-panel example**",
        "markdownLoadMode",
        "Phase 18.5 authoring contract: no-default-panel, optional preview, generic primitive consumption",
    ] {
        assert!(
            guide.contains(required),
            "package guide must mark example status accurately: {required}"
        );
    }
}

#[test]
fn creating_packages_docs_reject_package_ui_antipatterns() {
    let guide = creating_packages_guide();

    for required in [
        "direct native widget access",
        "raw Deno ops",
        "native widget handles",
        "raw `Deno.core.ops`",
        "Execute package JavaScript in the Rust client",
        "Create or mutate Masonry widgets directly from package code",
        "Provide CSS, HTML, script, draw callbacks, or native handles",
        "filesystem/network/shell/AI/WASM work without an approved permissioned API",
        "Add Markdown-specific Rust UI/layout branches for package behavior",
        "Publish a default fixed panel from the package load path",
        "Use the SDUI `publishTree` left-slot bridge as a user-facing panel authoring pattern",
        "Hard-code a side panel position or width",
        "planned working-area/split-tree/slot-layout/state/override `clay:ui` snippets or planned configuration helpers as callable runtime code",
        "Treat `serverLoadPackage` as ordinary end-user package installation, enablement, or execution authority",
        "raw CSS, raw style strings, raw ops, native widget handles, Masonry widget constructors, client-side JavaScript, and native renderer callbacks",
        "It cannot grant permissions, bypass slot safety, expose native widgets, accept raw CSS, or run package JavaScript in the client",
        "does not grant broad authority",
        "arbitrary filesystem paths, network, shell, AI mutation, WASM, native widget handles, raw Deno ops, or client-side JavaScript by default",
    ] {
        assert!(
            guide.contains(required),
            "package guide must reject package UI/layout anti-pattern: {required}"
        );
    }
}

#[test]
fn phase18_3_package_guide_documents_slot_ui_component_panel_and_theme_apis() {
    let guide = creating_packages_guide();

    for required in [
        "Runtime-backed public APIs in `clay:ui` for Phase 18.3 inert package UI contributions",
        "serverRegisterPanelContribution(manifest, declaration)",
        "serverRegisterComponentContribution(manifest, declaration)",
        "serverRegisterTransientOverlayContribution(manifest, declaration)",
        "serverRegisterThemeToken(manifest, declaration)",
        "clay.contributions.ui.panels",
        "ui.components",
        "ui.overlays",
        "themeTokens",
        "Implemented/runtime-backed Phase 18.3 slot panel and token example",
        "Implemented/runtime-backed Phase 18.3 transient overlay example",
        "Phase 18.3 component catalog status",
        "table` | Planned/deferred",
        "dropdown` | Planned/deferred",
        "collapse` | Planned/deferred",
        "modal` | Planned/deferred",
        "No package wins a layout conflict by load order alone",
        "slot placement, fixed/transient panel behavior, overlay geometry, action validation, and observability privacy",
    ] {
        assert!(
            guide.contains(required),
            "package guide must document Phase 18.3 slot UI authoring phrase: {required}"
        );
    }
}

#[test]
fn phase18_3_primitives_docs_mark_slot_ui_rows_runtime_backed() {
    let strategy = shell_layout_strategy();
    let registry = primitives_registry();
    let backlog = primitives_backlog();
    let primitives_index = primitives_index();
    let inventory = api_inventory_text();

    for required in [
        "Phase 18.3 runtime-backed package UI contribution progress",
        "**Implemented/runtime-backed public APIs in Phase 18.3:**",
        "`PanelContribution` / `serverRegisterPanelContribution`",
        "`ComponentContribution` / `serverRegisterComponentContribution`",
        "`TransientOverlayContribution` / `serverRegisterTransientOverlayContribution`",
        "`PackageThemeTokenDeclaration` / `serverRegisterThemeToken`",
        "registry_public = true",
        "Accepted fixed panels compose into `PaneSlotLayout` geometry",
    ] {
        assert!(
            strategy.contains(required),
            "shell-layout strategy must mark Phase 18.3 runtime-backed status: {required}"
        );
    }

    for required in [
        "Phase 18.3 runtime-backed public API in `runtime/js/ui.ts`",
        "generated public registry page exists under `docs/reference/clay-js-api/ui/`",
        "Exists/Extend",
        "same-type core fallback",
        "Unknown/deferred component kind rejection",
    ] {
        assert!(
            registry.contains(required),
            "primitive registry must mark Phase 18.3 rows runtime-backed: {required}"
        );
    }

    for required in [
        "Implemented runtime-backed `runtime/js/ui.ts` facade",
        "Implemented runtime-backed component schema/catalog validation",
        "Implemented runtime-backed overlay descriptor validation",
        "Implemented runtime-backed theme token registry and resolver",
        "runtime-backed public APIs with facade/op/validator coverage, per-API Markdown docs, and generated registry entries",
    ] {
        assert!(
            backlog.contains(required),
            "primitive backlog must mark Phase 18.3 rows implemented/runtime-backed: {required}"
        );
    }

    assert!(
        primitives_index.contains("Phase 18.3 package UI primitives")
            && primitives_index.contains("runtime-backed inventory APIs through `clay:ui`"),
        "primitives index must summarize Phase 18.3 runtime-backed slot UI primitives"
    );

    for id in [
        "clay.ui.serverRegisterPanelContribution",
        "clay.ui.serverRegisterComponentContribution",
        "clay.ui.serverRegisterTransientOverlayContribution",
        "clay.ui.serverRegisterThemeToken",
    ] {
        let block = api_inventory_entry_block(&inventory, id);
        assert!(block.contains("status = \"runtime-backed\""));
        assert!(block.contains("registry_public = true"));
        assert!(block.contains("runtime/js/ui.ts"));
        assert!(block.contains("src/server/ops/ui.rs"));
        assert!(block.contains("src/server/ui.rs"));
    }
}

#[test]
fn phase18_3_docs_preserve_security_and_hot_path_contract() {
    let guide = creating_packages_guide();
    let strategy = shell_layout_strategy();
    let security = package_security();

    for source in [guide.as_str(), strategy.as_str(), security.as_str()] {
        for required in [
            "raw CSS",
            "raw style strings",
            "client-side JavaScript",
            "native widget handles",
            "Masonry",
            "renderer callbacks",
            "unregistered action",
            "no package JavaScript",
        ] {
            assert!(
                source.contains(required),
                "Phase 18.3 docs must preserve security/hot-path phrase `{required}`"
            );
        }
    }

    for required in [
        "validation/publication timing",
        "package load, package validation, configuration evaluation, explicit command handling, or explicit UI update time",
        "Masonry paint/layout, pointer, scroll, keypress, text-event handling, and ordinary editor hot paths read already-validated inert state only",
        "raw Deno ops",
        "raw `Deno.core.ops`",
        "raw colors without typed token contracts",
        "duplicate fixed slot claims",
        "unsupported typed style variables",
    ] {
        assert!(
            guide.contains(required) || strategy.contains(required) || security.contains(required),
            "Phase 18.3 docs must preserve hot-path/security/detail phrase: {required}"
        );
    }
}

#[test]
fn phase18_3_docs_mark_layout_override_surfaces_planned_and_state_scope_promoted_later() {
    let guide = creating_packages_guide();
    let strategy = shell_layout_strategy();
    let inventory = api_inventory_text();

    for required in [
        "`PackageUiStateScope` | `clay.ui.serverRegisterUiStateScope` | Implemented/runtime-backed public API for inert UI state schema/lifecycle declarations",
        "`PackageLayoutOverride` | `clay.ui.serverSetLayoutOverride` | Planned for documented user/package layout overrides.",
        "user-visible panel visibility/default-slot/theme-token override APIs remain planned inventory stubs",
        "Durable package UI state values, user/package layout overrides, persisted panel visibility, user theme-token remaps",
    ] {
        assert!(
            guide.contains(required) || strategy.contains(required),
            "Phase 18.3 docs must mark deferred state/override surface: {required}"
        );
    }

    for id in [
        "clay.ui.serverSetLayoutOverride",
        "clay.ui.serverRegisterWorkingAreaLayout",
        "clay.ui.serverRegisterPaneSplitTree",
        "clay.ui.serverSetPaneSlotLayout",
    ] {
        let block = api_inventory_entry_block(&inventory, id);
        assert!(block.contains("status = \"planned\""));
        assert!(block.contains("op_clay_runtime_unavailable"));
        assert!(block.contains("registry_public = false"));
    }

    let state_scope = api_inventory_entry_block(&inventory, "clay.ui.serverRegisterUiStateScope");
    assert!(state_scope.contains("status = \"runtime-backed\""));
    assert!(state_scope.contains("op_clay_ui_register_ui_state_scope"));
    assert!(state_scope.contains("registry_public = true"));
}

#[test]
fn phase18_3_package_ui_configuration_surfaces_are_planned_or_documented() {
    let configuration_doc = fs::read_to_string(repository_path(
        "docs/reference/clay-js-api/configuration.md",
    ))
    .expect("read configuration overview");
    let configuration_wiki = fs::read_to_string(repository_path(
        "docs/wiki/modules/configuration-runtime.md",
    ))
    .expect("read configuration runtime wiki");
    let strategy = shell_layout_strategy();
    let guide = creating_packages_guide();
    let inventory = api_inventory_text();

    for required in [
        "Phase 18.3 promotes package UI declaration APIs",
        "does not promote user-visible panel visibility, default-slot, component-style, theme-token override, or layout behavior configuration APIs",
        "not user-visible override APIs",
        "Configuration evaluation for shell/layout remains startup, package-load, configuration-change, or explicit setting-change work",
        "Masonry paint/layout, pointer, scroll, keypress, text-event handling, and editor hot paths read already-validated inert state",
    ] {
        assert!(
            configuration_doc.contains(required),
            "configuration overview must record Phase 18.3 package UI configuration status: {required}"
        );
    }

    for required in [
        "`clay:ui` contribution APIs exist for package declarations",
        "not user-visible configuration override APIs for default slots, panel visibility, component style overrides, theme-token remapping, or layout behavior",
        "`clay.ui.serverSetLayoutOverride` and `clay.configuration.setPackageOption` stay non-registry-public inventory rows",
    ] {
        assert!(
            configuration_wiki.contains(required),
            "configuration wiki must record Phase 18.3 package UI configuration boundary: {required}"
        );
    }

    for required in [
        "Phase 18.3 package UI configuration surfaces are declarations only",
        "user-visible panel visibility/default-slot/theme-token override APIs remain planned inventory stubs",
        "not public runtime-backed shell/layout configuration APIs in Phase 18.3",
        "Configuration and User Override Surfaces",
    ] {
        assert!(
            guide.contains(required) || strategy.contains(required),
            "package guide/shell strategy must describe planned-vs-documented configuration surface: {required}"
        );
    }

    for id in [
        "clay.ui.serverSetLayoutOverride",
        "clay.configuration.setPackageOption",
    ] {
        let block = api_inventory_entry_block(&inventory, id);
        assert!(
            block.contains("status = \"planned\""),
            "{id} must remain planned"
        );
        assert!(
            block.contains("registry_public = false"),
            "{id} must not be public registry-backed before full configuration docs/tests"
        );
        assert!(
            block.contains("key_bindings = []") && block.contains("custom_properties = ["),
            "{id} must preserve Clay JS API schema metadata even while planned"
        );
    }
}

#[test]
fn phase18_3_docs_reject_hidden_panel_style_and_layout_config_keys() {
    let configuration_doc = fs::read_to_string(repository_path(
        "docs/reference/clay-js-api/configuration.md",
    ))
    .expect("read configuration overview");
    let configuration_wiki = fs::read_to_string(repository_path(
        "docs/wiki/modules/configuration-runtime.md",
    ))
    .expect("read configuration runtime wiki");
    let strategy = shell_layout_strategy();
    let guide = creating_packages_guide();

    for source in [
        configuration_doc.as_str(),
        configuration_wiki.as_str(),
        strategy.as_str(),
        guide.as_str(),
    ] {
        let lower_source = source.to_ascii_lowercase();
        assert!(
            lower_source.contains("hidden json/toml/ad hoc")
                || source.contains("Do not add hidden JSON/TOML/ad hoc keys"),
            "Phase 18.3 docs/wiki must reject hidden panel/style/layout keys"
        );
        assert!(
            source.contains("documented Clay JS APIs")
                || source.contains("documented `~/.config/clay/init.js` Clay JS APIs"),
            "Phase 18.3 docs/wiki must route configuration through documented Clay JS APIs"
        );
        for denied in [
            "raw CSS",
            "native widget",
            "client-side JavaScript",
            "raw Deno ops",
            "renderer callbacks",
        ] {
            assert!(
                source.contains(denied),
                "Phase 18.3 docs/wiki must deny {denied} authority for package UI configuration"
            );
        }
    }

    for hidden_key in [
        "preview.position",
        "layout.preview.defaultSlot",
        "preview.defaultVisibility",
        "layout.preview.defaultVisibility",
        "theme.markdown.heading.1",
        "raw token override keys",
        "ad hoc style keys",
    ] {
        assert!(
            configuration_doc.contains(hidden_key)
                && configuration_wiki.contains(hidden_key)
                && (strategy.contains(hidden_key) || guide.contains(hidden_key)),
            "Phase 18.3 docs/wiki must identify hidden/ad hoc key `{hidden_key}` as rejected or planned-only"
        );
    }
}

#[test]
fn phase18_4_package_guide_documents_input_action_state_and_configuration_apis() {
    let guide = creating_packages_guide();
    let configuration_doc = fs::read_to_string(repository_path(
        "docs/reference/clay-js-api/configuration.md",
    ))
    .expect("read configuration overview");

    for required in [
        "PackageInputContribution",
        "serverRegisterInputContribution",
        "component-scoped action routing",
        "PackageUiStateScope",
        "serverRegisterUiStateScope",
        "schema/lifecycle metadata only",
        "PackageLayoutOverride",
        "serverSetLayoutOverride",
        "clay.configuration.setPackageOption",
        "layout.defaultVisibility",
        "layout.defaultSlot",
        "layout.splitRatio",
        "input.default",
        "action.default",
        "themeTokenRemap",
        "fallback",
        "package-configuration",
        "diagnostics",
        "Phase 18.5 Markdown replanning",
        "loadPackage(\"@clay/markdown\")",
    ] {
        assert!(
            guide.contains(required),
            "package guide must document Phase 18.4 authoring contract phrase: {required}"
        );
    }

    for required in [
        "Implemented Phase 18.4 configuration APIs",
        "clay.configuration.setPackageOption",
        "clay.ui.serverSetLayoutOverride",
        "runtime-backed",
        "registered input/action/theme-token references",
        "Deferred surfaces remain explicit",
    ] {
        assert!(
            configuration_doc.contains(required),
            "configuration overview must document Phase 18.4 configuration phrase: {required}"
        );
    }
}

#[test]
fn phase18_4_primitives_docs_mark_state_config_rows_runtime_backed_or_planned() {
    let strategy = shell_layout_strategy();
    let registry = primitives_registry();
    let backlog = primitives_backlog();
    let index = primitives_index();
    let inventory = api_inventory_text();

    for required in [
        "Phase 18.4 runtime-backed package input/state/configuration progress",
        "PackageInputContribution",
        "PackageUiStateScope",
        "PackageLayoutOverride",
        "PackageOwnedConfiguration",
        "serverSetLayoutOverride",
        "setPackageOption",
        "Runtime-backed public API; `op_clay_ui_set_layout_override`; registry-public with per-API docs.",
        "working-area, split-tree, and direct pane-slot mutation",
    ] {
        assert!(
            strategy.contains(required),
            "shell-layout strategy must mark Phase 18.4 runtime/planned status: {required}"
        );
    }

    for required in [
        "Phase 18.4 runtime-backed public API",
        "src/server/ui.rs::PackageUiRegistry::set_layout_override",
        "src/server/configuration.rs::ConfigurationRuntime::set_package_option",
        "per-API Markdown docs and generated registry entry",
        "Exists/Extend",
    ] {
        assert!(
            registry.contains(required),
            "primitive registry must mark Phase 18.4 rows runtime-backed: {required}"
        );
    }

    for required in [
        "Implemented runtime-backed `runtime/js/ui.ts`, `src/server/ops/ui.rs`, and `src/server/ui.rs::PackageUiRegistry::set_layout_override`",
        "Implemented runtime-backed `runtime/js/configuration.ts`, `src/server/ops/configuration.rs`, and `src/server/configuration.rs::ConfigurationRuntime::set_package_option`",
        "remaining direct shell mutation, durable persistence, pane selector, multi-panel ordering, overlay z-order, cross-window layout, and package enable/disable surfaces stay deferred",
    ] {
        assert!(
            backlog.contains(required),
            "primitive backlog must mark Phase 18.4 implementation/deferred status: {required}"
        );
    }

    assert!(
        index.contains(
            "Phase 18.4 runtime-backed package input/state/layout-override/configuration primitives"
        ),
        "primitive index must summarize Phase 18.4 runtime-backed primitives"
    );

    for (id, deno_op) in [
        (
            "clay.ui.serverRegisterInputContribution",
            "op_clay_ui_register_input_contribution",
        ),
        (
            "clay.ui.serverRegisterUiStateScope",
            "op_clay_ui_register_ui_state_scope",
        ),
        (
            "clay.ui.serverSetLayoutOverride",
            "op_clay_ui_set_layout_override",
        ),
        (
            "clay.configuration.setPackageOption",
            "op_clay_configuration_set_package_option",
        ),
    ] {
        let block = api_inventory_entry_block(&inventory, id);
        assert!(block.contains("status = \"runtime-backed\""));
        assert!(block.contains("registry_public = true"));
        assert!(block.contains(deno_op));
        assert!(block.contains("custom_properties = ["));
    }

    for id in [
        "clay.ui.serverRegisterWorkingAreaLayout",
        "clay.ui.serverRegisterPaneSplitTree",
        "clay.ui.serverSetPaneSlotLayout",
    ] {
        let block = api_inventory_entry_block(&inventory, id);
        assert!(block.contains("status = \"planned\""));
        assert!(block.contains("registry_public = false"));
    }
}

#[test]
fn phase18_4_docs_preserve_security_and_hot_path_contract() {
    let guide = creating_packages_guide();
    let strategy = shell_layout_strategy();
    let security = package_security();
    let package_loading = fs::read_to_string(repository_path(
        "docs/reference/primitives/package-loading.md",
    ))
    .expect("read package loading");

    for source in [
        guide.as_str(),
        strategy.as_str(),
        security.as_str(),
        package_loading.as_str(),
    ] {
        for required in [
            "raw CSS",
            "client-side JavaScript",
            "native widget",
            "raw ops",
            "renderer callbacks",
            "hidden",
            "unregistered actions",
            "package enable/disable",
        ] {
            assert!(
                source.contains(required),
                "Phase 18.4 docs must preserve security phrase `{required}`"
            );
        }
        assert!(
            source.contains("no package JavaScript") || source.contains("No package JavaScript"),
            "Phase 18.4 docs must preserve no-package-JS hot-path wording"
        );
    }

    for required in [
        "startup, package load, configuration reload, explicit command handling, or explicit UI update time",
        "Masonry paint/layout/pointer/scroll/key/text-event hot paths",
        "validation/publication/configuration work",
    ] {
        assert!(
            guide.contains(required),
            "package guide must document Phase 18.4 timing phrase: {required}"
        );
    }
}

#[test]
fn phase18_4_docs_mark_deferred_persistence_pane_selector_and_package_enable_surfaces() {
    let guide = creating_packages_guide();
    let strategy = shell_layout_strategy();
    let configuration_doc = fs::read_to_string(repository_path(
        "docs/reference/clay-js-api/configuration.md",
    ))
    .expect("read configuration overview");

    for source in [
        guide.as_str(),
        strategy.as_str(),
        configuration_doc.as_str(),
    ] {
        for required in [
            "durable state-value mutation",
            "pane selector",
            "multi-panel ordering",
            "overlay z-order",
            "cross-window layout",
            "package enable/disable",
            "planned/deferred",
        ] {
            assert!(
                source.contains(required),
                "Phase 18.4 docs must mark deferred surface `{required}`"
            );
        }
    }
}

#[test]
fn phase18_2_shell_runtime_docs_mark_implemented_and_planned_surfaces() {
    let strategy = shell_layout_strategy();
    let registry = primitives_registry();
    let backlog = primitives_backlog();

    for required in [
        "## Phase 18.2/18.3 Runtime Status",
        "**Implemented/runtime-internal in Phase 18.2:**",
        "`src/main.rs` starts a Clay-owned `ClayShellWidget` as the native root widget",
        "`src/shell/layout.rs` owns internal Rust `WorkingAreaLayout` state",
        "`PaneSplitTree` supports the default one-leaf tree plus generic horizontal/vertical split nodes",
        "`PaneSlotLayout` keeps exactly one mandatory `main` slot and optional fixed `left`, `right`, `top`, and `bottom` slots",
        "**Implemented/runtime-backed public APIs in Phase 18.3:**",
        "`PanelContribution` / `serverRegisterPanelContribution`",
        "`ComponentContribution` / `serverRegisterComponentContribution`",
        "`TransientOverlayContribution` / `serverRegisterTransientOverlayContribution`",
        "`PackageThemeTokenDeclaration` / `serverRegisterThemeToken`",
        "**Still planned/package-facing after Phase 18.3:**",
        "Implemented/runtime-internal Rust shape, not a package-facing JavaScript API",
    ] {
        assert!(
            strategy.contains(required),
            "shell layout strategy must mark implemented/planned Phase 18.2 surface: {required}"
        );
    }

    for required in [
        "Phase 18.2 internal runtime in `src/shell/layout.rs` / `src/masonry_shell.rs`",
        "Phase 18.2 internal runtime in `src/shell/layout.rs`",
        "Phase 18.3 runtime-backed public API",
        "generated public registry page exists under `docs/reference/clay-js-api/ui/`",
    ] {
        assert!(
            registry.contains(required),
            "primitive registry must mark Phase 18.2 runtime/internal status: {required}"
        );
    }

    for required in [
        "## Phase 18.2 Shell Runtime Implementation Status",
        "`WorkingAreaLayout` is implemented as an internal runtime foundation",
        "`PaneSplitTree` is implemented internally with default one-leaf state",
        "`PaneSlotLayout` is implemented internally with a mandatory `main` slot",
        "runtime-backed public APIs with facade/op/validator coverage, per-API Markdown docs, and generated registry entries",
    ] {
        assert!(
            backlog.contains(required),
            "primitive backlog must record Phase 18.2 implementation/planned status: {required}"
        );
    }
}

#[test]
fn creating_packages_docs_cover_phase18_5_shell_layout_examples() {
    let guide = creating_packages_guide();

    for required in [
        "Phase 18.5 authoring contract: no-default-panel, optional preview, generic primitive consumption",
        "No default fixed panel",
        "defaultVisibility: \"hidden\"",
        "PaneSlotLayout.main",
        "Optional preview as a `PanelContribution`",
        "Theme token usage for panel styling",
        "`setPackageOption` and `serverSetLayoutOverride` for customization",
        "Package-owned fallback alias retained after `loadPackage` shipped",
        "Implemented/runtime-backed Phase 18.5 no-default-panel example",
        "markdown.preview",
        "slot: \"right\"",
        "kind: \"fixed\"",
        "defaultVisibility: \"hidden\"",
        "markdown.togglePreview",
        "editor occupies `PaneSlotLayout.main`",
        "does not publish a default fixed panel on load",
        "`PanelContribution` targeting the `right` slot",
        "user can enable preview through `setPackageOption` or `serverSetLayoutOverride`",
        "preview panel styling uses `PackageThemeTokenDeclaration`",
        "consumes only generic shell/layout/UI/configuration primitives",
        "no Markdown-specific Rust branches",
    ] {
        assert!(
            guide.contains(required),
            "package guide must cover Phase 18.5 shell/layout authoring contract: {required}"
        );
    }
}

#[test]
fn creating_packages_docs_cover_phase18_2_shell_runtime_status() {
    let guide = creating_packages_guide();

    for required in [
        "current implemented public behavior",
        "Phase 18.2 internal shell runtime behavior",
        "Phase 18.3 runtime-backed slot UI contribution behavior",
        "planned package-facing shell/layout/configuration behavior",
        "Implemented/internal runtime",
        "Phase 18.2 shell/layout runtime and Phase 18.3 slot-aware package UI",
        "Phase 18.2 has implemented internally",
        "Phase 18.3 now adds runtime-backed public APIs",
        "Still planned for package authors",
        "Current Phase 18.3 runtime behavior",
        "Packages cannot create working areas, mutate pane split ratios, directly set pane-slot layouts, change shell configuration, persist UI state, or override user layout/theme choices through `clay:ui` in Phase 18.3",
        "Internal Rust runtime implemented; public callable layout-default API planned/unavailable",
        "Implemented/runtime-backed public API with per-API Markdown and generated registry coverage",
        "not public runtime-backed shell/layout configuration APIs in Phase 18.3",
    ] {
        assert!(
            guide.contains(required),
            "package guide must cover Phase 18.2 shell runtime status: {required}"
        );
    }
}

#[test]
fn phase18_2_shell_docs_preserve_security_and_hot_path_contract() {
    let strategy = shell_layout_strategy();
    let guide = creating_packages_guide();

    for required in [
        "no package JavaScript runs in Masonry paint, layout, pointer, scroll, keypress, or text-event handlers",
        "Masonry paint/layout, pointer, scroll, keypress, text-event handling, and ordinary editor hot paths read already-validated inert state only",
        "raw CSS",
        "arbitrary client JavaScript",
        "raw `Deno.core.ops`",
        "direct Masonry widget handles",
        "native widget handles",
        "Vello callbacks",
        "Parley callbacks",
        "unregistered action targets",
        "oversize payloads",
    ] {
        assert!(
            strategy.contains(required),
            "shell layout strategy must preserve hot-path/security contract: {required}"
        );
    }

    for required in [
        "Typing, Masonry paint, Masonry layout, scroll, pointer, keypress, and text-event paths read already-validated inert state",
        "do not run package JavaScript, package parsing, raw IPC waits, or package-authored native widget mutation",
        "raw `Deno.core.ops`",
        "native widget handles",
        "raw CSS, raw style strings, raw ops, native widget handles, Masonry widget constructors, client-side JavaScript, and native renderer callbacks",
        "It cannot grant permissions, bypass slot safety, expose native widgets, accept raw CSS, or run package JavaScript in the client",
        "Unsupported state scopes, ad hoc package keys, and package/user override bypass attempts are rejected before state affects the shell",
    ] {
        assert!(
            guide.contains(required),
            "package guide must preserve hot-path/security contract: {required}"
        );
    }
}

#[test]
fn phase18_3_slot_aware_package_ui_wiki_covers_final_implementation() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let slot_ui_wiki = fs::read_to_string(repository_path(
        "docs/wiki/modules/slot-aware-package-ui.md",
    ))
    .expect("read slot-aware package UI wiki");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let facade_wiki = fs::read_to_string(repository_path(
        "docs/wiki/modules/clay-js-facade-skeleton.md",
    ))
    .expect("read Clay JS facade wiki");
    let registry_wiki =
        fs::read_to_string(repository_path("docs/wiki/modules/clay-js-doc-registry.md"))
            .expect("read Clay JS doc registry wiki");

    assert!(
        wiki_index.contains("modules/slot-aware-package-ui.md"),
        "docs/wiki/index.md must link the Phase 18.3 slot-aware package UI implementation page"
    );
    assert!(
        primitive_architecture.contains("slot-aware-package-ui.md"),
        "primitive architecture wiki must link the slot-aware package UI implementation page"
    );

    for required in [
        "`runtime/js/ui.ts`",
        "`src/server/ops/ui.rs`",
        "`src/server/ui.rs`",
        "`src/shell/components.rs`",
        "`src/shell/theme.rs`",
        "`src/shell/package_ui.rs`",
        "`src/masonry_sdui.rs`",
        "serverRegisterPanelContribution",
        "serverRegisterComponentContribution",
        "serverRegisterTransientOverlayContribution",
        "serverRegisterThemeToken",
        "PackageUiRegistry",
        "PackageUiRegistrySnapshot::runtime_update",
        "PackageUiRuntimeState::slot_layout",
        "PanelContribution",
        "ComponentContribution",
        "TransientOverlayContribution",
        "PackageThemeTokenDeclaration",
        "table`, `dropdown`, `collapse`, and `modal` fail with planned/deferred diagnostics",
        "Masonry hot paths read already-validated inert package UI state only",
        "Package UI declarations grant no filesystem, network, shell, AI mutation, WASM",
        "raw Deno op, native widget, client-side JavaScript",
        "Observability helpers are crate-internal and omit document text",
        "User-visible layout overrides, default-slot overrides, persisted panel visibility",
        "CARGO_TARGET_DIR=target/pi-verify cargo test --test clay_js_api_inventory --quiet",
    ] {
        assert!(
            slot_ui_wiki.contains(required),
            "slot-aware package UI wiki must explain final implementation detail: {required}"
        );
    }

    for required in [
        "`clay:ui` facade",
        "serverRegisterPanelContribution",
        "op_clay_ui_*",
    ] {
        assert!(
            facade_wiki.contains(required) && registry_wiki.contains("`clay:ui`"),
            "Clay JS facade/registry wikis must record Phase 18.3 UI docs/API coverage: {required}"
        );
    }
}

#[test]
fn phase18_4_package_input_state_configuration_wiki_covers_final_implementation() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let implementation_wiki = fs::read_to_string(repository_path(
        "docs/wiki/modules/package-input-state-configuration.md",
    ))
    .expect("read Phase 18.4 implementation wiki");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let slot_ui_wiki = fs::read_to_string(repository_path(
        "docs/wiki/modules/slot-aware-package-ui.md",
    ))
    .expect("read slot-aware package UI wiki");
    let configuration_wiki = fs::read_to_string(repository_path(
        "docs/wiki/modules/configuration-runtime.md",
    ))
    .expect("read configuration runtime wiki");
    let package_loading_wiki =
        fs::read_to_string(repository_path("docs/wiki/modules/package-loading.md"))
            .expect("read package loading wiki");
    let command_registry_wiki =
        fs::read_to_string(repository_path("docs/wiki/modules/command-registry.md"))
            .expect("read command registry wiki");

    assert!(
        wiki_index.contains("modules/package-input-state-configuration.md"),
        "docs/wiki/index.md must link the Phase 18.4 implementation wiki"
    );
    assert!(
        primitive_architecture.contains("package-input-state-configuration.md"),
        "primitive architecture wiki must link the Phase 18.4 implementation wiki"
    );

    for required in [
        "`runtime/js/ui.ts`",
        "`runtime/js/configuration.ts`",
        "`src/server/ops/ui.rs`",
        "`src/server/ops/configuration.rs`",
        "`src/server/ui.rs`",
        "`src/server/configuration.rs`",
        "`src/shell/package_ui.rs`",
        "`src/masonry_sdui.rs`",
        "`src/packages/record.rs`",
        "`src/packages/conflict.rs`",
        "clay.ui.serverRegisterInputContribution",
        "clay.ui.serverRegisterUiStateScope",
        "clay.ui.serverSetLayoutOverride",
        "clay.configuration.setPackageOption",
        "PackageInputRouting",
        "PackageInputContribution",
        "PackageUiStateScope",
        "PackageLayoutOverride",
        "PackageOwnedConfiguration",
        "component-scoped action routing",
        "behavior-manifest compatibility",
        "layout.defaultVisibility",
        "layout.defaultSlot",
        "layout.splitRatio",
        "input.default",
        "action.default",
        "themeTokenRemap",
        "Durable workspace/document/component state-value persistence",
        "pane selector APIs",
        "multi-panel ordering",
        "overlay z-order",
        "package enable/disable authority",
        "Masonry hot paths read already-validated inert package UI/input/configuration state only",
        "do not run package JavaScript/config evaluation",
        "do not mutate Masonry children during layout",
        "Hidden configuration keys are rejected",
        "raw Masonry/native widget construction, raw CSS, raw Deno ops, renderer callbacks, and client-side JavaScript",
        "Observability remains crate-internal and privacy-preserving",
        "CARGO_TARGET_DIR=target/pi-verify cargo test --test performance_budgets --quiet",
        "tests/manual_smoke_docs.rs",
    ] {
        assert!(
            implementation_wiki.contains(required),
            "Phase 18.4 implementation wiki must explain final implementation detail: {required}"
        );
    }

    for (page_name, page) in [
        ("slot-aware package UI", slot_ui_wiki.as_str()),
        ("configuration runtime", configuration_wiki.as_str()),
        ("package loading", package_loading_wiki.as_str()),
        ("command registry", command_registry_wiki.as_str()),
    ] {
        assert!(
            page.contains("package-input-state-configuration.md"),
            "{page_name} wiki must link Phase 18.4 input/state/config implementation coverage"
        );
    }
}

#[test]
fn phase18_2_shell_runtime_wiki_covers_final_implementation() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let shell_wiki = fs::read_to_string(repository_path("docs/wiki/modules/masonry-shell.md"))
        .expect("read masonry shell wiki");
    let primitive_architecture = fs::read_to_string(repository_path(
        "docs/wiki/modules/primitive-architecture.md",
    ))
    .expect("read primitive architecture wiki");
    let editor_wiki = fs::read_to_string(repository_path("docs/wiki/modules/masonry-editor.md"))
        .expect("read masonry editor wiki");
    let sdui_wiki = fs::read_to_string(repository_path("docs/wiki/modules/server-driven-ui.md"))
        .expect("read server-driven UI wiki");
    let configuration_wiki = fs::read_to_string(repository_path(
        "docs/wiki/modules/configuration-runtime.md",
    ))
    .expect("read configuration runtime wiki");

    assert!(
        wiki_index.contains("modules/masonry-shell.md"),
        "docs/wiki/index.md must link the Phase 18.2 Masonry shell runtime page"
    );
    assert!(
        primitive_architecture.contains("masonry-shell.md"),
        "primitive architecture wiki must link the Masonry shell runtime implementation page"
    );

    for required in [
        "`src/masonry_shell.rs`",
        "`src/shell/layout.rs`",
        "`src/main.rs`",
        "`WorkingAreaLayout`",
        "`PaneSplitTree`",
        "`PaneSlotLayout`",
        "WorkingAreaLayoutUpdate",
        "ShellObservableSnapshot",
        "Public API, Configuration, and Package Authoring Boundary",
        "no package JavaScript",
        "does not mutate the Masonry child tree",
        "planned/unavailable inventory rows",
        "There are no hidden JSON/TOML/ad hoc split, slot, panel, preview-position, or shell style keys in Phase 18.2",
        "document text",
        "native handles",
        "Command: `CARGO_TARGET_DIR=target/pi-verify cargo test --lib shell --quiet`",
        "Command: `CARGO_TARGET_DIR=target/pi-verify cargo test --lib masonry_shell --quiet`",
    ] {
        assert!(
            shell_wiki.contains(required),
            "Masonry shell wiki must explain final implementation detail: {required}"
        );
    }

    for required in [
        "`EditorWidget` is no longer the top-level application layout",
        "Act as the shell-owned editor component under `ClayShellWidget`",
        "shell layout validation",
        "Masonry Shell Runtime",
    ] {
        assert!(
            editor_wiki.contains(required),
            "Masonry editor wiki must record shell-owned editor component boundary: {required}"
        );
    }

    for required in [
        "internal `PaneSlotLayout` bridge in `src/shell/layout.rs`",
        "temporary left side panel",
        "Phase 18.3 package-facing panel contributions",
    ] {
        assert!(
            sdui_wiki.contains(required),
            "SDUI wiki must record shell slot bridge behavior: {required}"
        );
    }

    assert!(
        configuration_wiki.contains("`clay:ui` contribution APIs exist for package declarations")
            && configuration_wiki.contains("no `clay:ui` configuration override API")
            && configuration_wiki.contains("hidden split/slot/panel/style key system"),
        "configuration wiki must record that Phase 18.3 package UI declarations are not user-visible configuration overrides"
    );
}

#[test]
fn shell_layout_primitives_are_recorded_in_registry_and_backlog() {
    let registry = primitives_registry();
    let backlog = primitives_backlog();

    let phase18_2 = ["WorkingAreaLayout", "PaneSplitTree", "PaneSlotLayout"];
    let phase18_3 = [
        "PanelContribution",
        "ComponentContribution",
        "TransientOverlayContribution",
        "PackageThemeTokenDeclaration",
    ];
    let phase18_4 = ["PackageUiStateScope", "PackageLayoutOverride"];

    for primitive in phase18_2.into_iter().chain(phase18_3).chain(phase18_4) {
        assert!(
            registry.contains(primitive),
            "primitive registry must contain Phase 18.1 shell/layout primitive {primitive}"
        );
        assert!(
            backlog.contains(primitive),
            "primitive backlog must contain Phase 18.1 shell/layout primitive {primitive}"
        );
    }

    for (phase, primitives) in [
        ("Phase-18.2-shell-runtime", phase18_2.as_slice()),
        ("Phase-18.3-slot-ui", phase18_3.as_slice()),
        ("Phase-18.4-state-config", phase18_4.as_slice()),
    ] {
        assert!(
            backlog.contains(phase),
            "backlog must define priority tier {phase}"
        );
        for primitive in primitives {
            let primitive_pos = backlog
                .find(primitive)
                .unwrap_or_else(|| panic!("missing backlog primitive {primitive}"));
            let phase_pos = backlog[..primitive_pos]
                .rfind(phase)
                .unwrap_or_else(|| panic!("{primitive} must be listed under {phase}"));
            assert!(
                phase_pos < primitive_pos,
                "{primitive} must appear after its phase heading {phase}"
            );
        }
    }

    for trace in [
        "shell-layout-strategy.md",
        "phase18.1-shell-layout-primitive-review.md",
        "api-inventory.toml",
        "Phase 18.1 Shell/Layout Handoff Checklist",
    ] {
        assert!(
            backlog.contains(trace),
            "shell/layout backlog must cite or checklist {trace}"
        );
    }
}

#[test]
fn shell_layout_primitives_record_hot_path_policy_and_security() {
    let registry = primitives_registry();
    let backlog = primitives_backlog();
    let security = package_security();

    for required in [
        "layout-state",
        "SDUI/component-state",
        "package-ui/state-data",
        "configuration-data",
        "no-hot-path",
        "Payload:",
        "package provenance",
        "conflict/precedence metadata",
        "deterministic rejection",
        "raw ops",
        "native widgets",
        "raw CSS",
        "client JS",
        "direct Masonry mutation",
        "duplicate slot/component/action IDs",
        "unknown theme tokens",
        "unsupported state scopes",
        "oversize layout/component/state payloads",
    ] {
        assert!(
            registry.contains(required),
            "shell/layout registry rows must record policy/security text: {required}"
        );
    }

    for required in [
        "no package JavaScript in Masonry paint/layout/input handlers",
        "raw `Deno.core.ops`",
        "direct Masonry/native widgets",
        "raw CSS",
        "client-side JavaScript",
        "Vello/Parley callbacks",
        "unknown tokens/scopes",
        "duplicate IDs/slots/actions",
        "oversize payloads",
    ] {
        assert!(
            backlog.contains(required),
            "shell/layout backlog must record hot-path/security text: {required}"
        );
    }

    for required in [
        "WorkingAreaLayout` / `PaneSplitTree` / `PaneSlotLayout",
        "PanelContribution` / `ComponentContribution` / `TransientOverlayContribution",
        "PackageThemeTokenDeclaration",
        "PackageUiStateScope",
        "PackageLayoutOverride",
        "Duplicate shell slot claim",
        "Duplicate component or overlay ID",
        "Unknown style/theme token",
        "Unsupported UI state scope",
        "direct Masonry widget constructors",
        "raw CSS, raw style strings, or HTML/script injection",
    ] {
        assert!(
            security.contains(required),
            "package security doc must record shell/layout validation text: {required}"
        );
    }
}

#[test]
fn shell_layout_planned_api_inventory_entries_are_traceable() {
    let inventory = api_inventory_text();
    let registry = primitives_registry();
    let backlog = primitives_backlog();

    let planned = [
        (
            "clay.ui.serverRegisterWorkingAreaLayout",
            "WorkingAreaLayout",
            "serverRegisterWorkingAreaLayout",
        ),
        (
            "clay.ui.serverRegisterPaneSplitTree",
            "PaneSplitTree",
            "serverRegisterPaneSplitTree",
        ),
        (
            "clay.ui.serverSetPaneSlotLayout",
            "PaneSlotLayout",
            "serverSetPaneSlotLayout",
        ),
        (
            "clay.ui.serverSetLayoutOverride",
            "PackageLayoutOverride",
            "serverSetLayoutOverride",
        ),
    ];

    let runtime_backed = [
        (
            "clay.ui.serverRegisterPanelContribution",
            "PanelContribution",
            "serverRegisterPanelContribution",
            "op_clay_ui_register_panel_contribution",
        ),
        (
            "clay.ui.serverRegisterComponentContribution",
            "ComponentContribution",
            "serverRegisterComponentContribution",
            "op_clay_ui_register_component_contribution",
        ),
        (
            "clay.ui.serverRegisterTransientOverlayContribution",
            "TransientOverlayContribution",
            "serverRegisterTransientOverlayContribution",
            "op_clay_ui_register_transient_overlay_contribution",
        ),
        (
            "clay.ui.serverRegisterInputContribution",
            "PackageInputContribution",
            "serverRegisterInputContribution",
            "op_clay_ui_register_input_contribution",
        ),
        (
            "clay.ui.serverRegisterUiStateScope",
            "PackageUiStateScope",
            "serverRegisterUiStateScope",
            "op_clay_ui_register_ui_state_scope",
        ),
        (
            "clay.ui.serverRegisterThemeToken",
            "PackageThemeTokenDeclaration",
            "serverRegisterThemeToken",
            "op_clay_ui_register_theme_token",
        ),
    ];

    for (id, primitive, js_export) in planned {
        let block = api_inventory_entry_block(&inventory, id);
        assert!(
            registry.contains(primitive),
            "{id} must trace to {primitive} in registry.md"
        );
        assert!(backlog.contains(id), "{id} must trace to primitive backlog");
        assert!(
            backlog.contains(primitive),
            "{id} must trace to backlog primitive {primitive}"
        );

        for required in [
            "visibility = \"public\"",
            "status = \"planned\"",
            "js_module = \"clay:ui\"",
            "runtime_path = ",
            "planned",
            "op_clay_runtime_unavailable",
            "documentation_path = \"docs/reference/primitives/shell-layout-strategy.md\"",
            "key_bindings = []",
            "custom_properties = [",
            "security_notes = ",
            "current_rust_owner = ",
            "registry_public = false",
            "does not grant filesystem",
            "network",
            "shell",
            "AI mutation",
            "WASM",
            "client-side JavaScript",
            "raw Deno ops",
            "direct Masonry widgets",
            "native widget handles",
            "raw CSS",
        ] {
            assert!(
                block.contains(required),
                "{id} planned stub is missing {required}"
            );
        }
        assert!(
            block.contains(&format!("js_export = \"{js_export}\"")),
            "{id} must keep the planned JS export {js_export}"
        );
    }

    for (id, primitive, js_export, deno_op) in runtime_backed {
        let block = api_inventory_entry_block(&inventory, id);
        assert!(registry.contains(primitive));
        assert!(backlog.contains(id));
        assert!(backlog.contains(primitive));
        for required in [
            "visibility = \"public\"",
            "status = \"runtime-backed\"",
            "js_module = \"clay:ui\"",
            "runtime_path = \"server-first-op-wrapper-runtime\"",
            "src/server/ui.rs::PackageUiRegistry",
            "documentation_path = \"docs/reference/clay-js-api/ui/",
            "key_bindings = []",
            "custom_properties = [",
            "security_notes = ",
            "Runtime-backed Clay JS API",
            "registry_public = true",
            "does not grant filesystem",
            "network",
            "shell",
            "AI mutation",
            "WASM",
            "client-side JavaScript",
            "raw Deno ops",
            "direct Masonry widgets",
            "native widget handles",
            "raw CSS",
        ] {
            assert!(
                block.contains(required),
                "{id} runtime-backed entry is missing {required}"
            );
        }
        assert!(block.contains(&format!("js_export = \"{js_export}\"")));
        assert!(block.contains(&format!("deno_op = \"{deno_op}\"")));
    }
    assert!(
        inventory.contains("docs/reference/clay-js-api/ui/"),
        "Phase 18.3 public clay:ui API docs are linked from runtime-backed inventory entries"
    );
}

#[test]
fn phase18_markdown_primitive_review_records_existing_inventory() {
    let index = fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let review = phase18_markdown_primitive_review();

    assert!(
        index.contains("modules/phase18-markdown-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18 Markdown primitive review"
    );
    for required in [
        "Package identity, permissions, and provenance",
        "Document classification",
        "Major-mode activation",
        "Command declaration and key routing",
        "Inert text transforms",
        "Parse handler registration and scheduling",
        "Decoration publication and rendering",
        "SDUI preview/status",
        "Configuration surfaces",
        "Documentation and registry coverage",
        "mode-registration",
        "mode-activation",
        "command-registration",
        "parse-document",
        "render-decorations",
        "no keypress, paint, scroll, layout, or text-event work",
        "Background only",
        "viewport-prioritized",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "Token-Stream Adapter Primitive Verification",
        "@clay/python",
        "keyword.control",
        "derive the mode from the package API prefix",
    ] {
        assert!(
            review.contains(required),
            "Phase 18 Markdown primitive review must record inventory/hot-path/security text: {required}"
        );
    }
}

#[test]
fn phase18_markdown_primitive_review_records_generic_gaps_only() {
    let review = phase18_markdown_primitive_review();

    for required in [
        "No Markdown-specific Rust parser, renderer, token mapper, heading/list/fence branch, or style map",
        "Complete generic text-transform engines",
        "language-neutral parse-input/range-snapshot/line-index primitive",
        "Remove mode-specific fallback defaults from generic package ops",
        "generic decoration/theme registry",
        "Acceptable names include `ParseRangeSnapshot`, `ParseLineIndex`, `StyleTokenRegistry`, or a completed `ContinueLineMarkers` engine",
        "Rejected names include `MarkdownParser`, `MarkdownHeading`, `MarkdownFence`, `MarkdownItToken`, or any `if mode == \"markdown\"` parser/rendering path",
    ] {
        assert!(
            review.contains(required),
            "Phase 18 Markdown primitive review must record generic-only gap guidance: {required}"
        );
    }
}

#[test]
fn phase18_large_file_markdown_review_records_generic_parse_window_gaps() {
    let review = phase18_large_file_markdown_primitive_review();

    for required in [
        "Parse coordinator and parse protocol",
        "Server document storage",
        "Viewport primitives",
        "Decoration transport and rendering",
        "Configuration surfaces",
        "Benchmark and budget primitives",
        "`ParseEditNotification` carries metadata only",
        "There is no `ParseWindowSnapshot`",
        "no server-canonical range snapshot helper",
        "no retained syntax cache accounting",
        "`EditorSurface::apply_decoration_set` replaces one span set",
        "no generic chunk key",
        "no LRU chunk cache",
        "`ParseWindowSnapshot` / `ParseRangeSnapshot`",
        "`ParseWindowRequest` / `ParsePolicy`",
        "`SyntaxCacheBudget` / memory accounting",
        "`DecorationChunk` / `SyntaxChunkCache`",
        "`ViewportRangeReport`",
        "30 MiB",
        "future large-file modes can reuse",
        "Markdown-specific handling remains in `packages/markdown/dist/parser.js`",
    ] {
        assert!(
            review.contains(required),
            "Phase 18.5 large-file primitive review must record generic gap text: {required}"
        );
    }
}

#[test]
fn phase18_large_file_review_links_reference_and_wiki_docs() {
    let wiki_index =
        fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let parse_strategy = parse_update_strategy();
    let rendering_strategy = rendering_strategy();
    let review = phase18_large_file_markdown_primitive_review();

    assert!(
        wiki_index.contains("modules/phase18-large-file-markdown-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 18.5 large-file primitive review"
    );
    for (doc_name, doc) in [
        ("parse-update-strategy.md", parse_strategy.as_str()),
        ("rendering-strategy.md", rendering_strategy.as_str()),
    ] {
        for required in [
            "phase18-large-file-markdown-primitive-review.md",
            "SyntaxCacheBudget",
        ] {
            assert!(
                doc.contains(required),
                "{doc_name} must link/reference the Phase 18.5 large-file primitive review and generic gap {required}"
            );
        }
    }
    assert!(
        parse_strategy.contains("ParseWindowSnapshot"),
        "parse-update-strategy.md must document the generic parse-window gap"
    );
    assert!(
        rendering_strategy.contains("DecorationChunk"),
        "rendering-strategy.md must document the generic decoration-chunk gap"
    );
    for required in [
        "docs/reference/primitives/parse-update-strategy.md",
        "docs/reference/primitives/rendering-strategy.md",
        "[Parse Coordinator](parse-coordinator.md)",
        "[Decoration Transport](decoration-transport.md)",
    ] {
        assert!(
            review.contains(required),
            "large-file primitive review must link related reference/wiki doc: {required}"
        );
    }
}

#[test]
fn phase19_file_open_primitive_review_records_existing_inventory() {
    let index = fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let review = phase19_windows_file_open_primitive_review();

    assert!(
        index.contains("modules/phase19-windows-file-open-primitive-review.md"),
        "docs/wiki/index.md must link the Phase 19 Windows file-open primitive review"
    );
    for required in [
        "Keybinding and configuration",
        "Behavior manifests and client key routing",
        "Client command routing and GUI event bridge",
        "IPC document open messages",
        "Server workspace validation",
        "Client snapshot/document replacement",
        "Mode activation",
        "Parse handler registration and adapter scheduling",
        "Decoration transport and native rendering",
        "SDUI and status",
        "Markdown package adapters",
        "Configuration-time",
        "Explicit UI-command time",
        "Server file-open time",
        "Document-open/background time",
        "Hot-path typing/paint/text-event work",
        "Windows file dialog",
        "selected-file IPC request",
        "server single-file grant/open",
        "no socket reads or writes in paint/text handlers",
        "full-text transfer limited to initial open/resync snapshots",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
    ] {
        assert!(
            review.contains(required),
            "Phase 19 file-open primitive review must record inventory/timing text: {required}"
        );
    }
}

#[test]
fn phase19_file_open_primitive_review_records_generic_gaps_only() {
    let review = phase19_windows_file_open_primitive_review();

    for required in [
        "ClientUiCommandIntent",
        "SelectedFileOpenRequest",
        "SelectedFileGrant",
        "DocumentOpenApplied",
        "DocumentOpenActivation",
        "ParserAdapterExecution",
        "ClientFileDialogBackend",
        "clay.documents.clientOpenFileDialog",
        "not to a hard-coded `Ctrl+O` branch",
        "at most that canonical file",
        "must not add the parent directory as a workspace root",
        "same GUI-safe snapshot replacement boundary used by startup/resync",
        "not by `if extension == \".md\"` or `if mode == \"markdown\"` branches",
        "Do not add `MarkdownOpenParser`",
        "MarkdownSelectedFileGrant",
        "MarkdownItToken",
        "Windows-specific code is acceptable only inside the dialog backend/module",
        "ordinary edits must continue through `Edit` deltas and bounded queues",
    ] {
        assert!(
            review.contains(required),
            "Phase 19 file-open primitive review must record generic-only gap guidance: {required}"
        );
    }
}

#[test]
fn rust_large_file_primitives_have_no_markdown_token_branches() {
    let primitive_sources = [
        "src/protocol/parse.rs",
        "src/server/parse_coordinator.rs",
        "src/server/document.rs",
        "src/protocol/decorations.rs",
        "src/server/decorations.rs",
        "src/editor/surface.rs",
        "src/client/mod.rs",
        "src/perf/budgets.rs",
    ];
    let forbidden = [
        "heading_open",
        "list_item_open",
        "bullet_list_open",
        "ordered_list_open",
        "strong_open",
        "em_open",
        "code_inline",
        "MarkdownParser",
        "MarkdownItToken",
        "MarkdownHeading",
        "MarkdownFence",
        "if mode == \"markdown\"",
        "if mode_id == \"markdown\"",
    ];

    for path in primitive_sources {
        let source = fs::read_to_string(repository_path(path)).expect("read primitive source");
        for marker in forbidden {
            assert!(
                !source.contains(marker),
                "{path} must not contain Markdown/markdown-it parser branch marker `{marker}`"
            );
        }
    }
}

#[test]
fn primitive_public_api_stubs_exist_with_required_phase16_metadata() {
    let inventory = api_inventory_text();
    for (id, doc_path) in [
        (
            "clay.decorations.serverPublishDecorations",
            "docs/reference/clay-js-api/decorations/server-publish-decorations.md",
        ),
        (
            "clay.parse.serverRegisterParseHandler",
            "docs/reference/clay-js-api/parse/server-register-parse-handler.md",
        ),
    ] {
        let block = api_inventory_entry_block(&inventory, id);
        assert!(
            block.contains("status = \"runtime-backed\""),
            "{id} must be runtime-backed"
        );
        assert!(
            block.contains("registry_public = true"),
            "{id} must be registry public"
        );
        assert!(
            std::path::Path::new(doc_path).exists(),
            "{id} docs must exist at {doc_path}"
        );
    }

    let stubs = [
        (
            "clay.folding.serverPublishFoldingRanges",
            "FoldingRange",
            "render-folding",
            "serverPublishFoldingRanges",
        ),
        (
            "clay.configuration.setPackageOption",
            "PackageOwnedConfiguration",
            "package-configuration",
            "setPackageOption",
        ),
    ];

    let registry = primitives_registry();
    let backlog = primitives_backlog();

    for (id, primitive, security_phrase, js_export) in stubs {
        let block = api_inventory_entry_block(&inventory, id);
        assert!(
            registry.contains(primitive),
            "{id} must trace to {primitive} in registry.md"
        );
        assert!(
            backlog.contains(id),
            "{id} must trace to the primitive backlog"
        );
        for required in [
            "visibility = \"public\"",
            "status = \"planned\"",
            "key_bindings = []",
            "custom_properties = [",
            "security_notes = ",
            "current_rust_owner = ",
            "registry_public = false",
            "does not grant filesystem",
            "network",
            "shell",
            "AI mutation",
            "WASM",
            "client-side JavaScript",
            "raw Deno ops",
        ] {
            assert!(
                block.contains(required),
                "{id} planned stub is missing {required}"
            );
        }
        assert!(
            block.contains(&format!("js_export = \"{js_export}\"")),
            "{id} must keep the planned JS export {js_export}"
        );
        assert!(
            block.contains(security_phrase),
            "{id} security notes must include {security_phrase}"
        );
    }
}

#[test]
fn phase18_5_markdown_wiki_documents_default_load_and_no_default_panel() {
    // Phase 18.5 (plans/028 Task 11) updates the Markdown implementation wiki
    // so the default load path, the deferred loadPackage gap, the
    // no-default-panel/optional-preview contract, and generic primitive
    // consumption cannot be orphaned from the code wiki.
    let markdown_package = fs::read_to_string(repository_path(
        "docs/wiki/modules/first-party-markdown-package.md",
    ))
    .expect("read first-party-markdown-package wiki");
    let markdown_activation = fs::read_to_string(repository_path(
        "docs/wiki/modules/markdown-mode-activation.md",
    ))
    .expect("read markdown-mode-activation wiki");
    let index = fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");

    // The Markdown package wiki documents the package-owned one-line fallback
    // and the deferred generic loadPackage gap with its decision log.
    for required in [
        "markdownLoadMode",
        "import { markdownLoadMode } from \"@clay/markdown\"",
        "await markdownLoadMode();",
        "loadPackage(\"@clay/markdown\")",
        "decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md",
        "ClayModuleLoader",
        "canonical_local_file",
        "security-critical module-loader bridge",
    ] {
        assert!(
            markdown_package.contains(required),
            "first-party-markdown-package wiki must document Phase 18.5 load path: {required}"
        );
    }

    // The Markdown package wiki documents the no-default-panel / optional
    // preview contract and generic primitive consumption.
    for required in [
        "PanelContribution",
        "defaultVisibility: \"hidden\"",
        "right",
        "main",
        "PaneSlotLayout",
        "setPackageOption",
        "serverSetLayoutOverride",
        "no default side panel",
        "no Markdown-specific Rust editor/parser/render/shell branch",
    ] {
        assert!(
            markdown_package.contains(required),
            "first-party-markdown-package wiki must document Phase 18.5 no-default-panel contract and generic primitive consumption: {required}"
        );
    }

    // The Markdown package wiki links the Phase 18.5 primitive review.
    assert!(
        markdown_package.contains("phase18.5-markdown-replan-primitive-review.md"),
        "first-party-markdown-package wiki must link the Phase 18.5 primitive review"
    );

    // The mode-activation wiki records the shared package-owned fallback
    // loader entry and the deferral decision log.
    for required in [
        "markdownLoadMode()",
        "import { markdownLoadMode } from \"@clay/markdown\"",
        "loadPackage(\"@clay/markdown\")",
        "decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md",
    ] {
        assert!(
            markdown_activation.contains(required),
            "markdown-mode-activation wiki must reference the shared fallback loader and deferral: {required}"
        );
    }

    // The master index links both updated pages and mentions the Phase 18.5
    // fallback / no-default-panel contract in the package page description.
    assert!(
        index.contains("modules/first-party-markdown-package.md")
            && index.contains("modules/markdown-mode-activation.md"),
        "wiki index must link both Markdown implementation pages"
    );
    assert!(
        index.contains("markdownLoadMode") && index.contains("no-default-panel"),
        "wiki index description for the Markdown package page must mention the Phase 18.5 fallback and no-default-panel contract"
    );
}

#[test]
fn phase18_14_language_package_expansion_wiki_documents_implementation() {
    // Phase 18.14 (plans/042 Task 11) updates the code wiki with the actual
    // implementation of the first-party Rust/TypeScript/JavaScript language
    // package expansion so the work cannot be orphaned from the wiki index.
    let index = fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let language_packages = fs::read_to_string(repository_path(
        "docs/wiki/modules/first-party-language-packages.md",
    ))
    .expect("read first-party-language-packages wiki");
    // The implementation wiki links the Phase 18.14 primitive review.
    assert!(
        language_packages.contains("phase18.14-language-package-expansion-primitive-review.md"),
        "first-party-language-packages wiki must link the Phase 18.14 primitive review"
    );

    // The implementation wiki documents the three expanded packages and their
    // generic primitive reuse.
    for required in [
        "@clay/rust",
        "@clay/typescript",
        "@clay/javascript",
        "buildCodeEditingManifest",
        "completionTriggerCharactersFromEditorRules",
        "serverListCompletionProvidersForTrigger",
        "serverRegisterModePattern",
        "serverRegisterCommand",
        "serverRegisterCompletionProvider",
        "serverRegisterComponentContribution",
        "statusItem",
        "toggleLineComment",
        "core.code",
        "core.text",
    ] {
        assert!(
            language_packages.contains(required),
            "first-party-language-packages wiki must document `{required}`"
        );
    }

    // The implementation wiki records source paths, tests, and security boundaries.
    for required in [
        "packages/rust/dist/load.js",
        "packages/typescript/dist/load.js",
        "packages/javascript/dist/load.js",
        "src/server/ops/modes.rs",
        "src/server/ops/completion.rs",
        "src/packages/record.rs",
        "mode-registration",
        "command-registration",
        "completion-provider",
        "parse-document",
        "render-decorations",
        "No LSP",
        "workspace-wide symbol indexes",
        "No client-side JavaScript",
        "tests/fixtures/configuration/language-packages/init.js",
        "rust_package_expansion_registers_mode_command_completion_and_status",
        "typescript_package_expansion_registers_mode_command_completion_and_status",
        "javascript_package_expansion_registers_mode_command_completion_and_status",
        "language_package_classification_is_deterministic_across_load_orders",
    ] {
        assert!(
            language_packages.contains(required),
            "first-party-language-packages wiki must document implementation detail `{required}`"
        );
    }

    // The master index links the implementation page.
    assert!(
        index.contains("modules/first-party-language-packages.md"),
        "wiki index must link the Phase 18.14 language package implementation page"
    );

    // The primitive review and implementation page are cross-linked from the index.
    assert!(
        index.contains("phase18.14-language-package-expansion-primitive-review.md")
            && index.contains("first-party-language-packages.md"),
        "wiki index must link both Phase 18.14 primitive review and implementation pages"
    );
}

#[test]
fn phase18_8_command_execution_implementation_wiki_covers_final_implementation() {
    let index = fs::read_to_string(repository_path("docs/wiki/index.md")).expect("read wiki index");
    let command_registry =
        fs::read_to_string(repository_path("docs/wiki/modules/command-registry.md"))
            .expect("read command registry wiki");
    let transient_menu = fs::read_to_string(repository_path(
        "docs/wiki/modules/transient-menu-session.md",
    ))
    .expect("read transient menu session wiki");
    let control_center = fs::read_to_string(repository_path("docs/wiki/modules/control-center.md"))
        .expect("read control center wiki");

    // Master index must link all three Phase 18.8 implementation pages.
    for linked in [
        "modules/command-registry.md",
        "modules/transient-menu-session.md",
        "modules/control-center.md",
    ] {
        assert!(
            index.contains(linked),
            "docs/wiki/index.md must link the Phase 18.8 implementation page `{linked}`"
        );
    }

    // Command registry wiki must explain the server-owned command execution
    // flow, source/test paths, authority boundaries, and confirm packages
    // cannot execute handlers, run package JS, or broaden authority.
    for required in [
        "Phase 18.8 adds the server-owned command execution foundation",
        "`CommandExecutor::execute`",
        "`CommandExecutionRequest`",
        "built-in server command table",
        "`ClayOpState::execute_command`",
        "`src/server/connection.rs` also normalizes inbound",
        "does not execute package JavaScript, install command handlers, grant filesystem/workspace/AI/shell/network authority",
        "Command registration does not grant execution authority",
        "tests/command_execution.rs",
    ] {
        assert!(
            command_registry.contains(required),
            "command registry wiki must document Phase 18.8 detail: {required}"
        );
    }

    // Transient menu session wiki must document the generic session model,
    // bounds, inert action contract, integration with CommandExecutor, and
    // must not present pub(crate) types as a public importable API.
    for required in [
        "`src/shell/transient_menu.rs`",
        "`TransientMenuSession`",
        "`MAX_ITEMS` (256)",
        "`MAX_QUERY_CHARS`",
        "`MAX_LABEL_CHARS`",
        "inert `TransientMenuAction`",
        "`CommandExecutionRequest` through the Phase 18.8 `CommandExecutor`",
        "routes it through `CommandExecutor`",
        "`TransientMenuSession` does not own rendering, focus restoration, or command execution semantics",
        "are `pub(crate)` and are not part of the public Clay JS API surface",
    ] {
        assert!(
            transient_menu.contains(required),
            "transient menu session wiki must document implementation detail: {required}"
        );
    }

    // Control Center wiki must document the open/filter/execute/cancel
    // workflow, source/test paths, security boundaries, and no-hot-path rule.
    for required in [
        "Phase 18.8 Task 7",
        "`ControlCenter::open",
        "`ControlCenter::set_query",
        "built-in server commands",
        "`CommandExecutor` validation",
        "Client-first and client-ui commands are excluded",
        "no callbacks, native handles, raw ops, or executable package code",
        "The Control Center does not consume fixed-slot geometry",
        "src/server/control_center.rs",
    ] {
        assert!(
            control_center.contains(required),
            "control center wiki must document implementation detail: {required}"
        );
    }
}
