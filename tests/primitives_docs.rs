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

fn package_security() -> String {
    fs::read_to_string(repository_path(
        "docs/reference/primitives/package-security.md",
    ))
    .expect("read package security")
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
    assert_eq!(COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES, 4096);
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
        "filesystem outside document content already open in Clay",
        "network",
        "shell",
        "AI mutation",
        "remote listeners",
        "WASM execution",
        "direct Masonry/widget mutation",
        "Any future exception requires a new approved decision log",
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
