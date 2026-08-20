use std::collections::BTreeSet;

use clay::docs::registry::{
    ClayJsApiRegistry, UPDATE_COMMAND, check_generated_registry_current, registry_source_paths,
    repository_root,
};

fn denied_configuration_authorities() -> [&'static str; 9] {
    [
        "filesystem",
        "network",
        "shell",
        "extension loading",
        "AI mutation",
        "workspace",
        "package",
        "WASM",
        "client-side JavaScript",
    ]
}

fn frontmatter_security(text: &str) -> Option<&str> {
    text.lines()
        .find_map(|line| line.strip_prefix("security: "))
        .map(str::trim)
}

#[test]
fn generated_registry_is_current() {
    let root = repository_root();
    check_generated_registry_current(&root).unwrap_or_else(|error| {
        panic!("{error}\nRepair command: {UPDATE_COMMAND}");
    });
}

#[test]
fn set_typography_api_doc_has_required_configuration_metadata() {
    let root = repository_root();
    let registry = ClayJsApiRegistry::from_docs(&root).expect("build registry from docs");
    let typography = registry
        .by_id("theme.setTypography")
        .expect("setTypography API is generated from docs");

    assert_eq!(typography.js_module, "clay:theme");
    assert_eq!(typography.js_export, "setTypography");
    assert_eq!(typography.key_bindings, Vec::<String>::new());
    assert_eq!(typography.permissions, Vec::<String>::new());
    for property in [
        "monospace.families",
        "monospace.size",
        "proportional.families",
        "proportional.size",
        "ui.families",
        "ui.size",
    ] {
        assert!(
            typography
                .custom_properties
                .iter()
                .any(|custom_property| custom_property.name == property),
            "setTypography registry entry must preserve custom property {property}"
        );
    }
}

#[test]
fn set_typography_api_is_linked_and_generated_registry_is_current() {
    let root = repository_root();
    let indexed_paths: BTreeSet<_> = registry_source_paths(&root)
        .expect("registry source paths")
        .into_iter()
        .collect();
    assert!(indexed_paths.contains("docs/reference/clay-js-api/theme/set-typography.md"));

    check_generated_registry_current(&root).unwrap_or_else(|error| {
        panic!("{error}\nRepair command: {UPDATE_COMMAND}");
    });
}

#[test]
fn invalid_init_typography_reports_actionable_validation_error() {
    let root = repository_root();
    let doc =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/theme/set-typography.md"))
            .expect("read setTypography API doc");
    for phrase in [
        "theme.invalid_typography",
        "does not partially install",
        "previous complete server state active",
        "generic fallback",
        "Removing the call",
    ] {
        assert!(
            doc.contains(phrase),
            "setTypography docs must explain {phrase:?}"
        );
    }
}

#[test]
fn generated_registry_contains_all_indexed_public_apis() {
    let root = repository_root();
    let indexed_paths: BTreeSet<_> = registry_source_paths(&root)
        .expect("registry source paths")
        .into_iter()
        .collect();
    let registry = ClayJsApiRegistry::from_docs(&root).expect("build generated registry from docs");
    let registry_paths: BTreeSet<_> = registry
        .entries
        .iter()
        .map(|entry| entry.documentation_path.clone())
        .collect();
    let registry_ids: BTreeSet<_> = registry
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect();

    assert_eq!(
        indexed_paths, registry_paths,
        "every docs/index.md registry source link must appear exactly once in generated registry data"
    );
    assert_eq!(
        registry_ids.len(),
        registry.entries.len(),
        "generated registry entries must have unique stable IDs"
    );
}

#[test]
fn phase28_folding_api_uses_reserved_core_domain() {
    assert!(
        clay::packages::manifest::RESERVED_CORE_API_DOMAINS.contains(&"folding"),
        "folding must remain reserved for the core Clay API"
    );
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let folding = registry
        .by_id("folding.serverPublishFoldingRanges")
        .expect("folding publication API must be discoverable");
    assert_eq!(folding.js_module, "clay:folding");
    assert_eq!(folding.js_export, "serverPublishFoldingRanges");
    assert_eq!(folding.permissions, vec!["render-folding"]);
}

#[test]
fn phase28_editor_command_apis_are_documented_and_facaded() {
    let root = repository_root();
    let docs_index = std::fs::read_to_string(root.join("docs/index.md")).expect("read docs index");
    let facade =
        std::fs::read_to_string(root.join("runtime/js/editor.js")).expect("read editor facade");
    let declarations = std::fs::read_to_string(root.join("runtime/js/editor.d.ts"))
        .expect("read editor declarations");
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let expected = [
        (
            "editor.toggleComment",
            "toggleComment",
            "docs/reference/clay-js-api/editor/toggle-comment.md",
            vec!["Ctrl+/".to_string()],
        ),
        (
            "editor.toggleListMarker",
            "toggleListMarker",
            "docs/reference/clay-js-api/editor/toggle-list-marker.md",
            Vec::new(),
        ),
        (
            "editor.rotateHeading",
            "rotateHeading",
            "docs/reference/clay-js-api/editor/rotate-heading.md",
            Vec::new(),
        ),
        (
            "editor.clientToggleFold",
            "clientToggleFold",
            "docs/reference/clay-js-api/editor/client-toggle-fold.md",
            Vec::new(),
        ),
        (
            "editor.toggleInlayHints",
            "toggleInlayHints",
            "docs/reference/clay-js-api/editor/toggle-inlay-hints.md",
            Vec::new(),
        ),
    ];

    for (id, export, docs_path, key_bindings) in expected {
        let entry = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("generated registry is missing Phase 28 API {id}"));
        assert_eq!(entry.js_module, "clay:editor");
        assert_eq!(entry.js_export, export);
        assert_eq!(entry.documentation_path, docs_path);
        assert_eq!(
            entry.key_bindings, key_bindings,
            "{id} key bindings drifted"
        );
        assert!(
            entry.custom_properties.is_empty(),
            "{id} has hidden options"
        );
        assert!(
            entry.app_visible && entry.help_visible,
            "{id} must be discoverable"
        );
        assert_eq!(entry.stability, "runtime-backed-command");
        assert!(docs_index.contains(docs_path.trim_start_matches("docs/")));
        assert!(facade.contains(&format!("export function {export}")));
        assert!(declarations.contains(&format!("function {export}")));
        assert_eq!(
            entry.deno_op, "op_clay_keybindings_bind_key",
            "{id} must use the binding validation boundary"
        );
        for denied in denied_configuration_authorities() {
            assert!(
                entry.security.contains(denied),
                "{id} security metadata must deny {denied} authority"
            );
        }
    }

    assert!(!facade.contains("Deno.core.ops.op_"));
}

#[test]
fn phase28_configuration_apis_have_documented_bindings_and_closed_options() {
    let root = repository_root();
    let configuration =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration API guide");
    let bind_key =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/keybindings/bind-key.md"))
            .expect("read bindKey API doc");
    let declarations = std::fs::read_to_string(root.join("runtime/js/configuration.d.ts"))
        .expect("read configuration declarations");
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for marker in [
        "## Phase 28 editor command configuration review",
        "default `Ctrl+/`",
        "editorRules.chrome.inlayHints",
        "Phase 28 adds no package option",
        "editor.commentPrefix",
        "editor.inlayHints.enabled",
    ] {
        assert!(
            configuration.contains(marker),
            "configuration guide must record Phase 28 marker {marker}"
        );
    }
    for command in [
        "editor.toggleComment",
        "editor.toggleListMarker",
        "editor.rotateHeading",
        "editor.clientToggleFold",
        "editor.toggleInlayHints",
    ] {
        assert!(
            bind_key.contains(command),
            "bindKey docs must list Phase 28 command {command}"
        );
    }

    let package_option = registry
        .by_id("configuration.setPackageOption")
        .expect("setPackageOption must remain a public configuration API");
    let source = package_option
        .custom_properties
        .iter()
        .find(|property| property.name == "source")
        .expect("setPackageOption source must remain a custom property");
    assert!(
        source.description.contains("ui-session"),
        "setPackageOption source metadata must include ui-session"
    );
    assert!(
        declarations.contains("| \"ui-session\""),
        "configuration.d.ts must expose the persisted ui-session source"
    );
}

#[test]
fn planned_shell_layout_apis_are_not_generated_registry_entries() {
    let root = repository_root();
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let docs_index = std::fs::read_to_string(root.join("docs/index.md")).expect("read docs index");

    for id in [
        "ui.serverRegisterWorkingAreaLayout",
        "ui.serverRegisterPaneSplitTree",
        "ui.serverSetPaneSlotLayout",
    ] {
        assert!(
            registry.by_id(id).is_none(),
            "planned shell/layout API {id} must not appear in generated public registry before public API implementation"
        );
    }

    for id in [
        "ui.serverRegisterPanelContribution",
        "ui.serverRegisterComponentContribution",
        "ui.serverRegisterTransientOverlayContribution",
        "ui.serverRegisterInputContribution",
        "ui.serverRegisterUiStateScope",
        "ui.serverRegisterThemeToken",
        "ui.serverSetLayoutOverride",
    ] {
        let entry = registry.by_id(id).unwrap_or_else(|| {
            panic!(
                "Phase 18.3 runtime-backed clay:ui API {id} must appear in the generated registry"
            )
        });
        assert_eq!(entry.js_module, "clay:ui");
        assert!(
            entry.documentation_path.contains("/clay-js-api/ui/")
                || entry.documentation_path.contains("clay-js-api/ui/")
        );
    }

    for planned_doc in [
        "reference/clay-js-api/ui/server-register-working-area-layout.md",
        "reference/clay-js-api/ui/server-register-pane-split-tree.md",
        "reference/clay-js-api/ui/server-set-pane-slot-layout.md",
    ] {
        assert!(
            !docs_index.contains(planned_doc),
            "docs/index.md registry source section must not link planned clay:ui API docs before public API implementation: {planned_doc}"
        );
    }
}

#[test]
fn syntax_engine_api_docs_registry_are_fresh() {
    let root = repository_root();
    check_generated_registry_current(&root).unwrap_or_else(|error| {
        panic!("{error}\nRepair command: {UPDATE_COMMAND}");
    });
    let docs_index = std::fs::read_to_string(root.join("docs/index.md")).expect("read docs index");
    let syntax_facade =
        std::fs::read_to_string(root.join("runtime/js/syntax.js")).expect("read syntax facade");
    let syntax_ops =
        std::fs::read_to_string(root.join("src/server/ops/syntax.rs")).expect("read syntax ops");
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for (id, export, docs_path, required_properties) in [
        (
            "syntax.serverRegisterSyntaxGrammar",
            "serverRegisterSyntaxGrammar",
            "docs/reference/clay-js-api/syntax/server-register-syntax-grammar.md",
            [
                "packagePrefix",
                "languageId",
                "grammar",
                "queries",
                "styleMap",
            ]
            .as_slice(),
        ),
        (
            "syntax.setSyntaxEnginePreference",
            "setSyntaxEnginePreference",
            "docs/reference/clay-js-api/syntax/set-syntax-engine-preference.md",
            ["target", "tier"].as_slice(),
        ),
    ] {
        let entry = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("generated registry is missing Phase 18.16 syntax API {id}"));
        assert_eq!(entry.js_module, "clay:syntax");
        assert_eq!(entry.js_export, export);
        assert_eq!(entry.documentation_path, docs_path);
        assert_eq!(entry.stability, "runtime-backed");
        assert!(entry.app_visible);
        assert!(entry.help_visible);
        assert!(entry.key_bindings.is_empty(), "{id} has no default key");
        assert!(docs_index.contains(docs_path.trim_start_matches("docs/")));
        assert!(syntax_facade.contains(export));
        assert!(syntax_ops.contains(&entry.deno_op));
        assert!(registry.by_js_export("clay:syntax", export).is_some());
        for property in required_properties {
            assert!(
                entry
                    .custom_properties
                    .iter()
                    .any(|custom_property| custom_property.name == *property),
                "{id} must preserve custom property {property}"
            );
        }
        for tag in ["js-api", "syntax"] {
            assert!(
                entry.lookup_tags.iter().any(|lookup_tag| lookup_tag == tag),
                "{id} must preserve lookup tag {tag}"
            );
        }
        for denied in [
            "filesystem",
            "network",
            "shell",
            "extension loading",
            "AI mutation",
            "client-side JavaScript",
        ] {
            assert!(
                entry.security.contains(denied),
                "{id} security metadata must deny {denied} authority"
            );
        }
    }

    assert!(!syntax_facade.contains("Deno.core.ops.op_"));
    assert!(!syntax_facade.contains("rawOps("));
}

#[test]
fn generated_registry_contains_phase18_4_public_apis() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for (id, module, export, docs_path, required_properties, required_tags) in [
        (
            "ui.serverRegisterInputContribution",
            "clay:ui",
            "serverRegisterInputContribution",
            "docs/reference/clay-js-api/ui/server-register-input-contribution.md",
            [
                "id",
                "scope",
                "componentId",
                "pointer.click",
                "actionTargets",
            ]
            .as_slice(),
            ["input", "action-routing", "phase18.4"].as_slice(),
        ),
        (
            "ui.serverRegisterUiStateScope",
            "clay:ui",
            "serverRegisterUiStateScope",
            "docs/reference/clay-js-api/ui/server-register-ui-state-scope.md",
            [
                "id",
                "scope",
                "owner",
                "lifetime",
                "persistence",
                "valueSchema.kind",
            ]
            .as_slice(),
            ["state", "lifecycle", "phase18.4"].as_slice(),
        ),
        (
            "ui.serverSetLayoutOverride",
            "clay:ui",
            "serverSetLayoutOverride",
            "docs/reference/clay-js-api/ui/server-set-layout-override.md",
            ["targetId", "property", "value", "source"].as_slice(),
            ["layout-overrides", "configuration", "phase18.4"].as_slice(),
        ),
        (
            "configuration.setPackageOption",
            "clay:configuration",
            "setPackageOption",
            "docs/reference/clay-js-api/configuration/set-package-option.md",
            ["packagePrefix", "option", "value", "source"].as_slice(),
            ["package-options", "configuration", "phase18.4"].as_slice(),
        ),
    ] {
        let entry = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("generated registry is missing Phase 18.4 API {id}"));
        assert_eq!(entry.js_module, module);
        assert_eq!(entry.js_export, export);
        assert_eq!(entry.documentation_path, docs_path);
        assert_eq!(entry.stability, "runtime-backed");
        assert!(entry.app_visible, "{id} must be app visible");
        assert!(entry.help_visible, "{id} must be help visible");
        assert!(entry.key_bindings.is_empty(), "{id} has no default key");
        assert!(registry.by_js_export(module, export).is_some());
        for property in required_properties {
            assert!(
                entry
                    .custom_properties
                    .iter()
                    .any(|custom_property| custom_property.name == *property),
                "{id} generated registry entry must preserve custom property {property}"
            );
            assert!(
                registry
                    .by_custom_property(property)
                    .iter()
                    .any(|entry| entry.id == id),
                "{id} must be discoverable by custom property {property}"
            );
        }
        for tag in required_tags {
            assert!(
                entry.lookup_tags.iter().any(|lookup_tag| lookup_tag == tag),
                "{id} generated registry entry must preserve lookup tag {tag}"
            );
            assert!(
                registry
                    .by_lookup_tag(tag)
                    .iter()
                    .any(|entry| entry.id == id),
                "{id} must be discoverable by lookup tag {tag}"
            );
        }
        for denied in [
            "filesystem",
            "network",
            "shell",
            "extension loading",
            "AI mutation",
            "WASM",
            "client-side JavaScript",
            "raw Deno ops",
        ] {
            assert!(
                entry.security.contains(denied),
                "{id} security metadata must deny {denied} authority"
            );
        }
    }

    for planned_id in [
        "ui.serverRegisterWorkingAreaLayout",
        "ui.serverRegisterPaneSplitTree",
        "ui.serverSetPaneSlotLayout",
    ] {
        assert!(
            registry.by_id(planned_id).is_none(),
            "planned Phase 18 shell/layout API {planned_id} must stay out of generated registry"
        );
    }
}

#[test]
fn large_file_parse_public_surfaces_have_clay_js_api_docs() {
    let root = repository_root();
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    let parse = registry
        .by_id("parse.serverRegisterParseHandler")
        .expect("large-file parse handler API is generated");
    assert_eq!(parse.js_module, "clay:parse");
    assert_eq!(parse.js_export, "serverRegisterParseHandler");
    assert_eq!(parse.key_bindings, Vec::<String>::new());
    assert!(
        parse
            .permissions
            .iter()
            .any(|permission| permission == "parse-document")
    );
    for property in [
        "module",
        "exportName",
        "modeId",
        "parseUnit",
        "viewportPriority",
        "timeoutMs",
        "maxWindowBytes",
        "guardBytes",
        "memoryBudgetBytes",
        "resultBudgetBytes",
    ] {
        assert!(
            parse
                .custom_properties
                .iter()
                .any(|custom_property| custom_property.name == property),
            "parse API registry entry must preserve custom property {property}"
        );
    }

    let decorations = registry
        .by_id("decorations.serverPublishDecorations")
        .expect("large-file decoration publication API is generated");
    assert_eq!(decorations.js_module, "clay:decorations");
    assert_eq!(decorations.js_export, "serverPublishDecorations");
    assert_eq!(decorations.key_bindings, Vec::<String>::new());
    assert!(
        decorations
            .permissions
            .iter()
            .any(|permission| permission == "render-decorations")
    );
    for property in [
        "documentId",
        "documentVersion",
        "viewportByteRange",
        "spans",
        "packagePrefix",
    ] {
        assert!(
            decorations
                .custom_properties
                .iter()
                .any(|custom_property| custom_property.name == property),
            "decoration API registry entry must preserve custom property {property}"
        );
    }

    let diagnostics = registry
        .by_id("diagnostics.serverPublishDiagnostics")
        .expect("range diagnostic publication API is generated");
    assert_eq!(diagnostics.js_module, "clay:diagnostics");
    assert_eq!(diagnostics.js_export, "serverPublishDiagnostics");
    assert_eq!(diagnostics.key_bindings, Vec::<String>::new());
    assert!(
        diagnostics
            .permissions
            .iter()
            .any(|permission| permission == "render-decorations")
    );
    for property in [
        "documentId",
        "documentVersion",
        "viewport",
        "source",
        "spans",
        "packagePrefix",
    ] {
        assert!(
            diagnostics
                .custom_properties
                .iter()
                .any(|custom_property| custom_property.name == property),
            "diagnostics API registry entry must preserve custom property {property}"
        );
    }

    for (path, required) in [
        (
            "docs/reference/clay-js-api/parse/server-register-parse-handler.md",
            [
                "bounded parse-window snapshots",
                "cancellable",
                "viewport-prioritized",
                "memoryBudgetBytes",
                "server-issued token",
                "runtime.timeout",
                "Do not expose internal parse-window snapshot structs",
            ],
        ),
        (
            "docs/reference/clay-js-api/decorations/server-publish-decorations.md",
            [
                "visible or near-viewport chunks",
                "SYNTAX_CACHE_BUDGET_BYTES",
                "30 MiB",
                "stale-version rejection",
                "Do not expose or call internal chunk-cache helpers",
                "DECORATION_PAYLOAD_BUDGET_BYTES",
                "server-side package code",
            ],
        ),
        (
            "docs/reference/clay-js-api/diagnostics/server-publish-diagnostics.md",
            [
                "DIAGNOSTIC_PAYLOAD_BUDGET_BYTES",
                "DIAGNOSTIC_CACHE_BUDGET_BYTES",
                "render-decorations",
                "source-keyed",
                "server-side package code",
                "language-server process",
                "Do not expose or call internal chunk-cache helpers",
            ],
        ),
    ] {
        let text = std::fs::read_to_string(root.join(path)).expect("read large-file API doc");
        for phrase in required {
            assert!(
                text.contains(phrase),
                "{path} must document large-file API policy phrase {phrase:?}"
            );
        }
    }
}

#[test]
fn syntax_grammar_public_api_has_docs_registry_and_security_metadata() {
    let root = repository_root();
    let indexed_paths: BTreeSet<_> = registry_source_paths(&root)
        .expect("registry source paths")
        .into_iter()
        .collect();
    assert!(
        indexed_paths
            .contains("docs/reference/clay-js-api/syntax/server-register-syntax-grammar.md"),
        "docs/index.md must link syntax grammar registration API docs"
    );

    let registry = ClayJsApiRegistry::from_docs(&root).expect("build registry from docs");
    let syntax = registry
        .by_id("syntax.serverRegisterSyntaxGrammar")
        .expect("syntax grammar API is generated from docs");
    assert_eq!(syntax.js_module, "clay:syntax");
    assert_eq!(syntax.js_export, "serverRegisterSyntaxGrammar");
    assert_eq!(syntax.key_bindings, Vec::<String>::new());
    for permission in ["parse-document", "render-decorations"] {
        assert!(
            syntax.permissions.iter().any(|value| value == permission),
            "syntax API registry entry must preserve {permission} permission"
        );
    }
    for property in [
        "packageManifest",
        "packageName",
        "packagePrefix",
        "permissions",
        "syntaxGrammar",
        "languageId",
        "filePatterns",
        "grammar",
        "queries",
        "styleMap",
        "budgets",
    ] {
        assert!(
            syntax
                .custom_properties
                .iter()
                .any(|custom_property| custom_property.name == property),
            "syntax API registry entry must preserve custom property {property}"
        );
    }
    let doc = std::fs::read_to_string(
        root.join("docs/reference/clay-js-api/syntax/server-register-syntax-grammar.md"),
    )
    .expect("read syntax API doc");
    for phrase in [
        "tree-sitter-wasm",
        "package-root-confined",
        "first-party-only",
        "third-party/native grammar artifact loading",
        "raw Deno ops",
        "Background",
        "no-hot-path",
        "DECORATION_PAYLOAD_BUDGET_BYTES",
        "SYNTAX_CACHE_BUDGET_BYTES",
    ] {
        assert!(doc.contains(phrase), "syntax API doc must mention {phrase}");
    }
}

#[test]
fn large_file_api_docs_are_linked_from_index() {
    let root = repository_root();
    let indexed_paths: BTreeSet<_> = registry_source_paths(&root)
        .expect("registry source paths")
        .into_iter()
        .collect();

    for path in [
        "docs/reference/clay-js-api/parse/server-register-parse-handler.md",
        "docs/reference/clay-js-api/decorations/server-publish-decorations.md",
        "docs/reference/clay-js-api/diagnostics/server-publish-diagnostics.md",
    ] {
        assert!(
            indexed_paths.contains(path),
            "docs/index.md must link large-file public API doc {path}"
        );
    }
}

#[test]
fn large_file_generated_registry_is_fresh() {
    let root = repository_root();
    check_generated_registry_current(&root).unwrap_or_else(|error| {
        panic!("{error}\nRepair command: {UPDATE_COMMAND}");
    });

    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    for id in [
        "parse.serverRegisterParseHandler",
        "decorations.serverPublishDecorations",
        "diagnostics.serverPublishDiagnostics",
    ] {
        assert!(
            registry.by_id(id).is_some(),
            "fresh generated registry must contain {id}"
        );
    }
}

#[test]
fn generated_registry_preserves_configuration_metadata() {
    let root = repository_root();
    let registry = ClayJsApiRegistry::from_docs(&root).expect("build generated registry from docs");

    let cursor_style = registry
        .entries
        .iter()
        .find(|entry| entry.id == "editor.clientSetCursorStyle")
        .expect("cursor style configuration API is generated");
    assert_eq!(cursor_style.js_module, "clay:editor");
    assert_eq!(cursor_style.js_export, "clientSetCursorStyle");
    assert_eq!(
        cursor_style.js_facade,
        "runtime/js/editor.js::clientSetCursorStyle"
    );
    assert_eq!(
        cursor_style.backing_rust,
        "src/editor/surface/mod.rs::EditorSurface::set_caret_style_override"
    );
    assert_eq!(cursor_style.deno_op, "op_clay_editor_set_cursor_style");
    assert!(cursor_style.permissions.is_empty());
    assert!(cursor_style.key_bindings.is_empty());
    assert!(cursor_style.lookup_tags.iter().any(|tag| tag == "editor"));
    assert!(cursor_style.security.contains("does not grant filesystem"));
    for property in ["shape", "blink", "stopBlinkOnTyping"] {
        assert!(
            cursor_style
                .custom_properties
                .iter()
                .any(|custom_property| custom_property.name == property),
            "cursor style registry entry must preserve custom property {property}"
        );
    }

    let bind_key = registry
        .entries
        .iter()
        .find(|entry| entry.id == "keybindings.bindKey")
        .expect("bindKey configuration API is generated");
    assert_eq!(bind_key.js_module, "clay:keybindings");
    assert_eq!(bind_key.js_export, "bindKey");
    assert!(bind_key.key_bindings.is_empty());
    for property in ["key", "command", "scope", "when"] {
        assert!(
            bind_key
                .custom_properties
                .iter()
                .any(|custom_property| custom_property.name == property),
            "bindKey registry entry must preserve custom property {property}"
        );
    }

    let quit = registry
        .entries
        .iter()
        .find(|entry| entry.id == "application.quit")
        .expect("quit API is generated");
    assert_eq!(quit.key_bindings, vec!["Escape".to_string()]);
}

#[test]
fn generated_registry_contains_phase9_file_workspace_apis() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let expected = [
        (
            "documents.serverOpenDocument",
            "clay:documents",
            "serverOpenDocument",
        ),
        (
            "documents.serverSaveDocument",
            "clay:documents",
            "serverSaveDocument",
        ),
        (
            "documents.serverReloadDocument",
            "clay:documents",
            "serverReloadDocument",
        ),
        (
            "documents.serverGetDocumentStatus",
            "clay:documents",
            "serverGetDocumentStatus",
        ),
        (
            "documents.serverListDocuments",
            "clay:documents",
            "serverListDocuments",
        ),
        (
            "workspace.serverListWorkspaceRoots",
            "clay:workspace",
            "serverListWorkspaceRoots",
        ),
    ];

    for (id, js_module, js_export) in expected {
        let entry = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("generated registry is missing {id}"));
        assert_eq!(entry.js_module, js_module);
        assert_eq!(entry.js_export, js_export);
        assert!(entry.lookup_tags.iter().any(|tag| tag == "workspace"));
        assert!(entry.security.contains("path traversal rejection"));
        assert!(entry.security.contains("does not grant filesystem"));
    }

    assert!(
        registry
            .by_lookup_tag("dirty-state")
            .iter()
            .any(|entry| entry.id == "documents.serverSaveDocument"),
        "dirty-state lookup should find Phase 9 save/status/reload APIs"
    );
}

#[test]
fn generated_registry_contains_phase18_12_workspace_file_browser_apis() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    for (id, js_module, js_export, security_needle) in [
        (
            "workspace.serverAddWorkspaceRoot",
            "clay:workspace",
            "serverAddWorkspaceRoot",
            "selected-file grants",
        ),
        (
            "workspace.serverDiscoverWorkspaceRootForPath",
            "clay:workspace",
            "serverDiscoverWorkspaceRootForPath",
            "closed Clay-owned marker set",
        ),
        (
            "workspace.serverListDirectory",
            "clay:workspace",
            "serverListDirectory",
            "bounded ignore/depth/count rules",
        ),
        (
            "workspace.serverCreateListingCancelToken",
            "clay:workspace",
            "serverCreateListingCancelToken",
            "opaque cancellation token",
        ),
        (
            "workspace.serverCancelListing",
            "clay:workspace",
            "serverCancelListing",
            "opaque token",
        ),
        (
            "commands.serverExecuteCommand",
            "clay:commands",
            "serverExecuteCommand",
            "selected-file grants",
        ),
        (
            "commands.serverOpenFile",
            "clay:commands",
            "serverOpenFile",
            "selected-file single-file grants",
        ),
        (
            "commands.serverRevealInTree",
            "clay:commands",
            "serverRevealInTree",
            "open server workspace metadata",
        ),
    ] {
        let entry = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("generated registry missing {id}"));
        assert_eq!(entry.js_module, js_module);
        assert_eq!(entry.js_export, js_export);
        assert!(entry.lookup_tags.iter().any(|tag| tag == "phase18.12"));
        assert!(
            entry.security.contains(security_needle),
            "{id} security metadata must mention {security_needle}"
        );
        assert!(entry.security.contains("raw Deno ops"));
        assert!(entry.security.contains("client-side JavaScript"));
    }
}

#[test]
fn generated_registry_contains_client_open_file_dialog_command_api() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let entry = registry
        .by_id("documents.clientOpenFileDialog")
        .expect("generated registry is missing clientOpenFileDialog");

    assert_eq!(entry.js_module, "clay:documents");
    assert_eq!(entry.js_export, "clientOpenFileDialog");
    assert_eq!(
        entry.js_facade,
        "runtime/js/documents.js::clientOpenFileDialog"
    );
    assert_eq!(entry.stability, "runtime-backed-command");
    assert!(entry.key_bindings.is_empty());
    assert!(entry.custom_properties.is_empty());
    assert!(entry.permissions.is_empty());
    for required in [
        "explicit user key routing",
        "single-file grants",
        "raw Deno ops",
        "broad filesystem/workspace authority",
        "client-side JavaScript",
    ] {
        assert!(
            entry.security.contains(required),
            "clientOpenFileDialog security metadata must mention {required:?}"
        );
    }
    assert!(
        registry
            .by_js_export("clay:documents", "clientOpenFileDialog")
            .is_some()
    );
    assert!(
        registry
            .by_lookup_tag("open-dialog")
            .iter()
            .any(|entry| entry.id == "documents.clientOpenFileDialog"),
        "open-dialog lookup should find the file dialog command API"
    );
    assert!(
        registry
            .by_lookup_tag("keybindings")
            .iter()
            .any(|entry| entry.id == "documents.clientOpenFileDialog"),
        "keybinding lookup should find the bindable file dialog command API"
    );
}

#[test]
fn generated_registry_contains_file_browser_workflow_command_apis() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for (id, module, export, tag, security_needles) in [
        (
            "workspace.clientOpenFolderDialog",
            "clay:workspace",
            "clientOpenFolderDialog",
            "open-folder",
            [
                "explicit user key routing",
                "selected-path capability",
                "raw Deno ops",
            ],
        ),
        (
            "editor.clientCopySelection",
            "clay:editor",
            "clientCopySelection",
            "clipboard",
            [
                "current non-empty native editor selection",
                "separate documented command IDs",
                "raw Deno ops",
            ],
        ),
        (
            "editor.clientCutSelection",
            "clay:editor",
            "clientCutSelection",
            "clipboard",
            [
                "ordinary local edit",
                "arbitrary clipboard text writes",
                "raw Deno ops",
            ],
        ),
        (
            "editor.clientPasteClipboard",
            "clay:editor",
            "clientPasteClipboard",
            "clipboard",
            [
                "ordinary local edit",
                "clipboard-contents inspection",
                "raw Deno ops",
            ],
        ),
        (
            "editor.clientUndo",
            "clay:editor",
            "clientUndo",
            "undo",
            ["ordinary inverse edit", "editable lease", "raw Deno ops"],
        ),
        (
            "editor.clientRedo",
            "clay:editor",
            "clientRedo",
            "redo",
            ["ordinary inverse edit", "editable lease", "raw Deno ops"],
        ),
        (
            "editor.clientShowOpenDocuments",
            "clay:editor",
            "clientShowOpenDocuments",
            "multi-document",
            [
                "retained client sessions",
                "filesystem/workspace expansion",
                "raw Deno ops",
            ],
        ),
        (
            "editor.clientRequestResync",
            "clay:editor",
            "clientRequestResync",
            "resync",
            [
                "ordinary inverse edit",
                "package/configuration/AI mutation authority",
                "raw Deno ops",
            ],
        ),
        (
            "editor.clientDismissRecovery",
            "clay:editor",
            "clientDismissRecovery",
            "recovery",
            [
                "runtime diagnostics",
                "package/configuration/AI mutation authority",
                "raw Deno ops",
            ],
        ),
        (
            "commands.serverOpenDirectory",
            "clay:commands",
            "serverOpenDirectory",
            "open-directory",
            ["root-relative", "known workspace root", "raw Deno ops"],
        ),
    ] {
        let entry = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("generated registry is missing {id}"));
        assert_eq!(entry.js_module, module);
        assert_eq!(entry.js_export, export);
        assert!(entry.app_visible);
        assert!(entry.help_visible);
        assert!(entry.custom_properties.is_empty());
        for required in security_needles {
            assert!(
                entry.security.contains(required),
                "{id} security metadata must mention {required:?}"
            );
        }
        assert!(registry.by_js_export(module, export).is_some());
        assert!(
            registry
                .by_lookup_tag(tag)
                .iter()
                .any(|entry| entry.id == id),
            "{tag} lookup should find {id}"
        );
    }
}

#[test]
fn generated_registry_contains_primitive_gate_runtime_apis() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for (id, module, export, permission) in [
        (
            "packages.serverValidatePackageManifest",
            "clay:packages",
            "serverValidatePackageManifest",
            None,
        ),
        (
            "packages.serverValidatePackagePermissions",
            "clay:packages",
            "serverValidatePackagePermissions",
            None,
        ),
        (
            "packages.serverLoadPackage",
            "clay:packages",
            "serverLoadPackage",
            None,
        ),
        (
            "modes.serverRegisterModePattern",
            "clay:modes",
            "serverRegisterModePattern",
            Some("mode-registration"),
        ),
        (
            "modes.serverClassifyDocument",
            "clay:modes",
            "serverClassifyDocument",
            None,
        ),
        (
            "modes.serverActivateMajorMode",
            "clay:modes",
            "serverActivateMajorMode",
            Some("mode-activation"),
        ),
        (
            "commands.serverRegisterCommand",
            "clay:commands",
            "serverRegisterCommand",
            Some("command-registration"),
        ),
        (
            "commands.serverListCommands",
            "clay:commands",
            "serverListCommands",
            None,
        ),
    ] {
        let entry = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("generated registry is missing {id}"));
        assert_eq!(entry.js_module, module);
        assert_eq!(entry.js_export, export);
        assert_eq!(entry.stability, "runtime-backed");
        assert!(entry.key_bindings.is_empty());
        assert!(entry.security.contains("server validation"));
        assert!(entry.security.contains("raw Deno ops"));
        if let Some(permission) = permission {
            assert!(
                entry.permissions.iter().any(|value| value == permission),
                "{id} must preserve required permission {permission}"
            );
        }
    }

    assert!(
        registry
            .by_id("modes.serverSelectDocumentManifest")
            .is_none()
    );
    assert!(
        registry
            .by_lookup_tag("packages")
            .iter()
            .any(|entry| entry.id == "packages.serverValidatePackageManifest")
    );
}

#[test]
fn generated_registry_contains_phase13_sdui_runtime_apis() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let expected = [
        ("sdui.definePanel", "definePanel", false),
        ("sdui.defineLabel", "defineLabel", false),
        ("sdui.defineButton", "defineButton", false),
        ("sdui.defineList", "defineList", false),
        ("sdui.defineEditorView", "defineEditorView", false),
        ("sdui.defineFlex", "defineFlex", false),
        ("sdui.defineStack", "defineStack", false),
        ("sdui.publishTree", "publishTree", true),
    ];

    for (id, js_export, is_async) in expected {
        let entry = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("generated registry is missing {id}"));
        assert_eq!(entry.js_module, "clay:sdui");
        assert_eq!(entry.js_export, js_export);
        assert_eq!(entry.stability, "runtime-backed");
        assert_eq!(entry.is_async, is_async, "{id} async metadata is wrong");
        assert!(entry.key_bindings.is_empty());
        assert!(entry.lookup_tags.iter().any(|tag| tag == "sdui"));
        assert!(entry.security.contains("inert declarative UI metadata"));
        for denied in denied_configuration_authorities() {
            assert!(
                entry.security.contains(denied),
                "{id} must deny implicit {denied} authority"
            );
        }
    }

    assert!(
        registry
            .by_lookup_tag("server-driven-ui")
            .iter()
            .any(|entry| entry.id == "sdui.defineEditorView"),
        "SDUI helpers should be discoverable by server-driven-ui lookup tag"
    );
    assert!(
        registry
            .by_custom_property("documentId")
            .iter()
            .any(|entry| entry.id == "sdui.defineEditorView"),
        "editor-view helper should be discoverable by documentId custom property"
    );
    assert!(
        registry
            .by_custom_property("tree")
            .iter()
            .any(|entry| entry.id == "sdui.publishTree"),
        "publishTree should be discoverable by tree custom property"
    );
}

#[test]
fn lookup_finds_api_by_stable_id_and_export() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    let by_id = registry
        .by_id("editor.clientSetCursorStyle")
        .expect("lookup by stable id");
    assert_eq!(by_id.js_module, "clay:editor");
    assert_eq!(by_id.js_export, "clientSetCursorStyle");

    let by_export = registry
        .by_js_export("clay:editor", "clientSetCursorStyle")
        .expect("lookup by JS module/export");
    assert_eq!(by_export.id, by_id.id);

    let by_name = registry.by_user_facing_name("set cursor style");
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].id, "editor.clientSetCursorStyle");

    let server_owned = registry.by_kind_owner(Some("clay-js-api"), Some("server"));
    assert!(
        server_owned
            .iter()
            .any(|entry| entry.id == "keybindings.bindKey"),
        "kind/owner lookup should include server-owned key binding configuration APIs"
    );

    let editor_tagged = registry.by_lookup_tag("editor");
    assert!(
        editor_tagged
            .iter()
            .any(|entry| entry.id == "editor.clientSetCursorStyle"),
        "lookup tag search should find editor configuration APIs"
    );
}

#[test]
fn lookup_finds_configuration_by_custom_property() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for property in ["shape", "blink", "stopBlinkOnTyping"] {
        let matches = registry.by_custom_property(property);
        assert!(
            matches
                .iter()
                .any(|entry| entry.id == "editor.clientSetCursorStyle"),
            "custom property lookup should find cursor style by {property}"
        );
    }
}

#[test]
fn cursor_style_custom_properties_are_complete() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let cursor_style = registry
        .by_id("editor.clientSetCursorStyle")
        .expect("cursor style customization API");

    let shape = cursor_style
        .custom_properties
        .iter()
        .find(|property| property.name == "shape")
        .expect("shape custom property");
    assert_eq!(shape.property_type, "enum");
    assert_eq!(shape.default, "bar");
    for allowed in ["bar", "line", "block", "underline"] {
        assert!(
            shape.description.contains(allowed),
            "shape custom property must document allowed value {allowed}"
        );
    }

    let blink = cursor_style
        .custom_properties
        .iter()
        .find(|property| property.name == "blink")
        .expect("blink custom property");
    assert_eq!(blink.property_type, "enum");
    assert_eq!(blink.default, "solid");
    for allowed in ["solid", "blink", "phase", "smooth"] {
        assert!(
            blink.description.contains(allowed),
            "blink custom property must document allowed value {allowed}"
        );
    }

    let stop_blink = cursor_style
        .custom_properties
        .iter()
        .find(|property| property.name == "stopBlinkOnTyping")
        .expect("stopBlinkOnTyping custom property");
    assert_eq!(stop_blink.property_type, "boolean");
    assert_eq!(stop_blink.default, "true");

    let root = repository_root();
    let text = std::fs::read_to_string(
        root.join("docs/reference/clay-js-api/editor/client-set-cursor-style.md"),
    )
    .expect("read cursor style API doc");
    assert!(text.contains("default `solid`"));
    assert!(
        text.contains("allowed values are `\"bar\"`, `\"line\"`, `\"block\"`, and `\"underline\"`")
    );
}

#[test]
fn editor_customization_has_no_external_authority() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for id in ["editor.clientSetCursorStyle", "editor.clientSetViewport"] {
        let entry = registry.by_id(id).expect("editor customization entry");
        assert!(
            entry.security.contains("document mutation"),
            "{id} must deny document mutation authority"
        );
        for denied in [
            "filesystem",
            "network",
            "shell",
            "extension loading",
            "AI mutation",
            "workspace",
            "package",
            "WASM",
            "client-side JavaScript",
        ] {
            assert!(
                entry.security.contains(denied),
                "{id} must deny implicit {denied} authority"
            );
        }
    }
}

#[test]
fn configuration_lookup_finds_cursor_customization() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    assert!(
        registry
            .by_lookup_tag("cursorstylecustomization")
            .iter()
            .any(|entry| entry.id == "editor.clientSetCursorStyle"),
        "cursor style customization should be discoverable by lookup tag"
    );
    for property in ["shape", "blink", "stopBlinkOnTyping"] {
        assert!(
            registry
                .by_custom_property(property)
                .iter()
                .any(|entry| entry.id == "editor.clientSetCursorStyle"),
            "cursor style customization should be discoverable by {property} custom property"
        );
    }
}

#[test]
fn lookup_lists_empty_default_key_bindings() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for id in [
        "keybindings.bindKey",
        "keybindings.unbindKey",
        "keybindings.listKeyBindings",
        "editor.clientSetCursorStyle",
        "editor.clientSelectPrevMatch",
        "editor.clientKeepSelection",
        "editor.clientRemoveSelection",
    ] {
        let entry = registry
            .by_id(id)
            .expect("lookup entry with empty defaults");
        assert!(
            entry.key_bindings.is_empty(),
            "{id} should expose an empty key_bindings list when it has no defaults"
        );
    }

    let escape = registry.by_key_binding("Escape");
    assert_eq!(escape.len(), 2);
    let escape_ids: Vec<&str> = escape.iter().map(|entry| entry.id.as_str()).collect();
    assert!(escape_ids.contains(&"application.quit"));
    // Plan 071 task 9: the editor consumes Escape first (menu > snippet >
    // cancel multiple selections); app-level quit remains the fallback.
    assert!(escape_ids.contains(&"editor.clientCancelMultipleSelections"));
}

#[test]
fn keybinding_configuration_apis_have_empty_defaults() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for (id, export) in [
        ("keybindings.bindKey", "bindKey"),
        ("keybindings.unbindKey", "unbindKey"),
        ("keybindings.listKeyBindings", "listKeyBindings"),
    ] {
        let entry = registry.by_id(id).expect("key binding API is generated");
        assert_eq!(entry.js_module, "clay:keybindings");
        assert_eq!(entry.js_export, export);
        assert!(
            entry.key_bindings.is_empty(),
            "{id} has no default key binding"
        );
        assert!(
            entry.lookup_tags.iter().any(|tag| tag == "keybindings"),
            "{id} should be discoverable by keybindings lookup tag"
        );
        assert!(
            entry.security.contains("client-side JavaScript"),
            "{id} must deny client-side JavaScript authority"
        );
    }
}

#[test]
fn keybinding_configuration_custom_properties_are_queryable() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    let bind_key = registry
        .by_id("keybindings.bindKey")
        .expect("bindKey registry entry");
    let bind_key_properties: BTreeSet<_> = bind_key
        .custom_properties
        .iter()
        .map(|property| property.name.as_str())
        .collect();
    assert_eq!(
        bind_key_properties,
        BTreeSet::from(["command", "key", "scope", "when"])
    );

    for property in ["key", "command", "scope", "when"] {
        let matches = registry.by_custom_property(property);
        assert!(
            matches
                .iter()
                .any(|entry| entry.id == "keybindings.bindKey"),
            "custom property lookup should find bindKey by {property}"
        );
    }
    assert!(
        registry
            .by_custom_property("scope")
            .iter()
            .any(|entry| entry.id == "keybindings.listKeyBindings"),
        "scope lookup should include listKeyBindings"
    );
}

#[test]
fn keybinding_docs_reject_undocumented_authority() {
    let root = repository_root();
    let denied_authorities = [
        "filesystem",
        "network",
        "shell",
        "extension loading",
        "AI mutation",
        "workspace",
        "package",
        "WASM",
        "client-side JavaScript",
    ];

    for path in [
        "docs/reference/clay-js-api/keybindings/bind-key.md",
        "docs/reference/clay-js-api/keybindings/unbind-key.md",
        "docs/reference/clay-js-api/keybindings/list-key-bindings.md",
    ] {
        let text = std::fs::read_to_string(root.join(path)).expect("read key binding API doc");
        assert!(
            text.contains("server-owned")
                || text.contains("future inert behavior manifests")
                || text.contains("manifest-routing metadata"),
            "{path} must describe behavior-manifest routing instead of client JavaScript hooks"
        );
        for denied in denied_authorities {
            assert!(
                text.contains(denied),
                "{path} must deny implicit {denied} authority"
            );
        }
    }

    let bind_key =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/keybindings/bind-key.md"))
            .expect("read bindKey API doc");
    assert!(
        bind_key.contains("documented Clay command/API ID")
            && bind_key.contains("registered and permissioned"),
        "bindKey docs must require documented/registered command IDs before binding"
    );
}

#[test]
fn configuration_entrypoint_is_documented_and_indexed() {
    let root = repository_root();
    let config_overview =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration overview");
    assert!(
        config_overview.contains("~/.config/clay/init.js"),
        "configuration overview must document the init.js entry point"
    );

    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let load_module = registry
        .by_id("configuration.loadConfigurationModule")
        .expect("loadConfigurationModule generated entry");
    let state = registry
        .by_id("configuration.getConfigurationState")
        .expect("getConfigurationState generated entry");

    assert_eq!(load_module.js_module, "clay:configuration");
    assert_eq!(load_module.js_export, "loadConfigurationModule");
    assert!(load_module.security.contains("constrained runtime"));
    assert!(
        load_module
            .custom_properties
            .iter()
            .any(|property| property.name == "path")
    );
    assert_eq!(state.js_module, "clay:configuration");
    assert_eq!(state.js_export, "getConfigurationState");
    assert!(
        state
            .security
            .contains("Returns configuration metadata only")
    );
    assert!(
        state
            .custom_properties
            .iter()
            .any(|property| property.name == "entryPoint")
    );
}

#[test]
fn configuration_module_loading_is_runtime_backed_no_external_authority() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let load_module = registry
        .by_js_export("clay:configuration", "loadConfigurationModule")
        .expect("configuration module loading export");

    assert!(load_module.key_bindings.is_empty());
    for denied in [
        "filesystem",
        "network",
        "shell",
        "extension loading",
        "AI mutation",
        "workspace",
        "package",
        "WASM",
        "client-side JavaScript",
    ] {
        assert!(
            load_module.security.contains(denied),
            "loadConfigurationModule must deny implicit {denied} authority"
        );
    }
    assert!(
        registry
            .by_lookup_tag("configuration")
            .iter()
            .any(|entry| entry.id == "configuration.loadConfigurationModule")
    );
    assert!(
        registry
            .by_custom_property("path")
            .iter()
            .any(|entry| entry.id == "configuration.loadConfigurationModule")
    );
    assert!(
        registry
            .by_custom_property("optional")
            .iter()
            .any(|entry| entry.id == "configuration.loadConfigurationModule"),
        "loadConfigurationModule must document the optional custom property"
    );
}

#[test]
fn lookup_is_read_only() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    assert!(registry.by_id("keybindings.bindKey").is_some());
    assert!(
        registry
            .by_id("configuration.loadConfigurationModule")
            .is_some()
    );
    assert!(
        registry.by_id("~/.config/clay/init.js").is_none(),
        "documentation lookup must not treat local configuration files as executable registry entries"
    );
    assert!(
        registry
            .entries
            .iter()
            .all(|entry| !entry.security.contains("executes configuration files")),
        "lookup exposes documentation metadata only and must not execute JavaScript or configuration files"
    );
}

#[test]
fn generated_registry_configuration_security_denies_implicit_external_authority() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for entry in registry.entries.iter().filter(|entry| {
        entry.lookup_tags.iter().any(|tag| {
            matches!(
                tag.as_str(),
                "configuration" | "keybindings" | "cursorstylecustomization"
            )
        }) || !entry.custom_properties.is_empty()
    }) {
        for denied in denied_configuration_authorities() {
            assert!(
                entry.security.contains(denied),
                "{} {} generated security metadata is missing no-authority language for {denied}",
                entry.id,
                entry.documentation_path
            );
        }
    }
}

#[test]
fn generated_registry_security_matches_source_docs() {
    let root = repository_root();
    let generated = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let from_docs = ClayJsApiRegistry::from_docs(&root).expect("build registry from source docs");

    for generated_entry in &generated.entries {
        let source_entry = from_docs
            .by_id(&generated_entry.id)
            .unwrap_or_else(|| panic!("source docs are missing {}", generated_entry.id));
        assert_eq!(
            generated_entry.security, source_entry.security,
            "{} {} generated security metadata must match source docs",
            generated_entry.id, generated_entry.documentation_path
        );

        let doc_text = std::fs::read_to_string(root.join(&generated_entry.documentation_path))
            .expect("read source API doc");
        assert_eq!(
            frontmatter_security(&doc_text),
            Some(generated_entry.security.as_str()),
            "{} {} generated registry security must preserve frontmatter exactly",
            generated_entry.id,
            generated_entry.documentation_path
        );
    }
}

#[test]
fn diagnostics_api_docs_and_generated_registry_are_fresh() {
    let root = repository_root();
    let indexed_paths: BTreeSet<_> = registry_source_paths(&root)
        .expect("registry source paths")
        .into_iter()
        .collect();
    assert!(
        indexed_paths
            .contains("docs/reference/clay-js-api/diagnostics/server-publish-diagnostics.md"),
        "docs/index.md must link serverPublishDiagnostics"
    );

    check_generated_registry_current(&root).unwrap_or_else(|error| {
        panic!("{error}\nRepair command: {UPDATE_COMMAND}");
    });

    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    let diagnostics = registry
        .by_id("diagnostics.serverPublishDiagnostics")
        .expect("serverPublishDiagnostics is generated");
    assert_eq!(diagnostics.js_module, "clay:diagnostics");
    assert_eq!(diagnostics.js_export, "serverPublishDiagnostics");
    assert_eq!(diagnostics.user_facing_name, "Publish Diagnostics");
    assert_eq!(diagnostics.key_bindings, Vec::<String>::new());
    assert!(
        diagnostics
            .permissions
            .iter()
            .any(|permission| permission == "render-decorations")
    );
    for property in [
        "documentId",
        "documentVersion",
        "viewport",
        "source",
        "spans",
        "packagePrefix",
    ] {
        assert!(
            diagnostics
                .custom_properties
                .iter()
                .any(|custom_property| custom_property.name == property),
            "diagnostics registry must preserve custom property {property}"
        );
    }

    let doc = std::fs::read_to_string(
        root.join("docs/reference/clay-js-api/diagnostics/server-publish-diagnostics.md"),
    )
    .expect("read diagnostics API doc");
    for phrase in [
        "background parse/analyze",
        "must not be called from ordinary typing, paint, layout, scroll, pointer, or text-event paths",
        "DIAGNOSTIC_PAYLOAD_BUDGET_BYTES",
        "render-decorations",
        "language-server process",
        "raw `Deno.core.ops`",
        "runtime/js/diagnostics.js::serverPublishDiagnostics",
        "op_clay_diagnostics_publish_diagnostics",
        "src/server/diagnostics.rs::validate_diagnostic_publication",
    ] {
        assert!(
            doc.contains(phrase),
            "diagnostics API docs must include {phrase:?}"
        );
    }
}

#[test]
fn configuration_api_covers_phase20_4_needs_or_defers() {
    // Plan 065 task 9: Phase 20.4 is a restyling phase that consumes existing
    // tokens (density.default/spacing_scale(), state tokens, typography
    // hierarchy). Verify the existing clay.theme/clay.configuration APIs
    // (setTheme, setTypography, designTokens overrides) already expose control
    // of those tokens, and that no new undocumented configuration key was
    // introduced. The deferral of a new config API is recorded in the plan.
    let root = repository_root();
    let set_theme =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/theme/set-theme.md"))
            .expect("read setTheme API doc");
    // setTheme + designTokens cover density, state/spacing tokens, ResolvedUiTheme.
    assert!(
        set_theme.contains("designTokens"),
        "setTheme must document designTokens overrides"
    );
    assert!(
        set_theme.contains("density"),
        "setTheme must document density coverage"
    );
    assert!(
        set_theme.contains("ResolvedUiTheme"),
        "setTheme must document ResolvedUiTheme resolution"
    );
    for token in ["spacing", "opacity"] {
        assert!(
            set_theme.contains(token),
            "setTheme must document {token} token coverage"
        );
    }

    let set_typography =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/theme/set-typography.md"))
            .expect("read setTypography API doc");
    assert!(
        set_typography.contains("hierarchy.display"),
        "setTypography must document the typography hierarchy custom properties"
    );

    let configuration =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration doc");
    assert!(
        configuration.contains("theme.setTheme"),
        "configuration.md must link setTheme"
    );
    assert!(
        configuration.contains("theme.setTypography"),
        "configuration.md must link setTypography"
    );

    // No new Phase 20.4 configuration API was introduced.
    let registry = ClayJsApiRegistry::from_docs(&root).expect("build registry from docs");
    for deferred_id in [
        "theme.setDensity",
        "ui.setComponentState",
        "theme.setComponentState",
        "ui.setSpacingRhythm",
    ] {
        assert!(
            registry.by_id(deferred_id).is_none(),
            "Phase 20.4 must not introduce a new configuration API {deferred_id}; the need is covered by setTheme/setTypography/designTokens"
        );
    }
}

#[test]
fn configuration_api_no_authority_grant() {
    // Plan 065 task 9: the configuration APIs Phase 20.4 consumes (setTheme,
    // setTypography) must not implicitly grant filesystem/network/shell/AI/
    // workspace authority. Their security metadata must deny each authority.
    let root = repository_root();
    let registry = ClayJsApiRegistry::from_docs(&root).expect("build registry from docs");
    for id in ["theme.setTheme", "theme.setTypography"] {
        let entry = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("{id} must be a registered configuration API"));
        for denied in denied_configuration_authorities() {
            assert!(
                entry.security.contains(denied),
                "{id} security metadata must deny {denied} authority"
            );
        }
    }
}

#[test]
fn clay_js_api_inventory_unchanged_or_documented() {
    // Plan 065 task 10: Phase 20.4 is pub(crate) paint/interaction work with
    // no new public programmatic capability. Verify the generated Clay JS API
    // registry contains no new Phase 20.4 programmatic surface id — i.e. no
    // new public API was introduced, and none is missing docs/registry entry.
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");
    // No Phase 20.4 helper or interaction primitive may appear as a public API id.
    for absent_id in [
        "ui.componentStateColor",
        "ui.interactionState",
        "theme.spacingRhythm",
        "ui.spacingRhythm",
        "editor.scrollbarState",
        "ui.scrollbarState",
        "ui.focusedAction",
        "ui.pointerState",
        "theme.fromUiTheme",
        "ui.themeStyle",
        "ui.disabled",
    ] {
        assert!(
            registry.by_id(absent_id).is_none(),
            "Phase 20.4 must not introduce a public Clay JS API {absent_id}; all new helpers are pub(crate)"
        );
    }
    // No registered API id may reference a Phase 20.4 internal helper name.
    let forbidden_fragments = [
        "componentStateColor",
        "listRowFillColor",
        "disabledTextColor",
        "fromUiTheme",
        "themeStyle",
        "interactionState",
        "scrollbarInteractionState",
        "focusedAction",
    ];
    for entry in &registry.entries {
        for fragment in forbidden_fragments {
            assert!(
                !entry.id.contains(fragment),
                "Phase 20.4 must not register a public API id containing {fragment}: {}",
                entry.id
            );
        }
    }
}

#[test]
fn phase22_8_programmatic_surface_inventory_is_closed() {
    // Plan 079 task 10: tab state is server-internal. Existing documented
    // workspace/document/tab APIs remain the only public programmatic routes;
    // none accepts an arbitrary TabId or exposes per-tab server handles.
    let root = repository_root();
    let registry = ClayJsApiRegistry::from_generated().expect("load generated registry");

    for id in [
        "commands.serverExecuteCommand",
        "commands.serverOpenFile",
        "commands.serverOpenDirectory",
        "documents.serverOpenDocument",
        "documents.serverListDocuments",
        "workspace.serverListWorkspaceRoots",
        "workspace.serverAddWorkspaceRoot",
        "workspace.clientOpenFolderDialog",
        "shell.clientTabNew",
        "keybindings.bindKey",
    ] {
        let entry = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("existing public API {id} is missing from registry"));
        assert_eq!(entry.visibility, "public", "{id} must remain public");
        assert!(
            !entry.js_facade.is_empty(),
            "{id} needs a JS facade mapping"
        );
        assert!(!entry.deno_op_path.is_empty(), "{id} needs an op mapping");
        assert!(
            root.join(&entry.documentation_path).is_file(),
            "{id} documentation path must exist: {}",
            entry.documentation_path
        );
        assert!(!entry.lookup_tags.is_empty(), "{id} needs lookup tags");
    }

    for absent_id in [
        "tabs.serverCreateTab",
        "tabs.serverSelectTabWorkspace",
        "workspace.serverOpenDocumentInTab",
        "documents.serverOpenDocumentInTab",
        "documents.serverListDocumentsForTab",
        "workspace.serverToggleFileBrowser",
        // This is a fixed built-in command ID routed through bindKey, not a
        // callable facade API or a second configuration surface.
        "workspace.toggleFileBrowser",
    ] {
        assert!(
            registry.by_id(absent_id).is_none(),
            "Phase 22.8 must not register internal/arbitrary-tab API {absent_id}"
        );
    }

    let keybindings =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/keybindings/bind-key.md"))
            .expect("read bindKey API doc");
    assert!(
        keybindings.contains("workspace.toggleFileBrowser"),
        "bindKey docs must document the fixed workspace toggle command"
    );
    let configuration =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration API doc");
    assert!(
        configuration.contains("visibility is retained per tab"),
        "configuration docs must document per-tab workspace-pane state"
    );
}

#[test]
fn configuration_api_documents_phase22_8_workspace_surface_without_new_keys() {
    // Plan 079 task 11: Phase 22.8 reuses bindKey; per-tab roots and pane
    // visibility remain server/client implementation state.
    let root = repository_root();
    let registry = ClayJsApiRegistry::from_docs(&root).expect("build registry from docs");
    let bind_key = registry
        .by_id("keybindings.bindKey")
        .expect("bindKey must remain a public configuration API");
    assert!(bind_key.key_bindings.is_empty());
    let client_tab_new = registry
        .by_id("shell.clientTabNew")
        .expect("clientTabNew must remain a public configuration API");
    assert_eq!(client_tab_new.key_bindings, vec!["Ctrl+T".to_string()]);
    let client_tab_new_doc =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/shell/client-tab-new.md"))
            .expect("read clientTabNew API doc");
    assert!(client_tab_new_doc.contains("Default: `Ctrl+T`"));
    for property in ["key", "command", "scope", "when"] {
        assert!(
            bind_key
                .custom_properties
                .iter()
                .any(|entry| entry.name == property)
        );
    }

    let configuration =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration API doc");
    for required in [
        "Phase 22.8 per-tab workspace configuration verification",
        "adds no new `clay:configuration` export",
        "shell.clientTabNew",
        "workspace.toggleFileBrowser",
        "fileBrowser.visible",
        "layout.json",
        "Configuration evaluation remains startup/reload work",
    ] {
        assert!(
            configuration.contains(required),
            "missing Phase 22.8 config marker {required}"
        );
    }

    let example = std::fs::read_to_string(root.join("examples/init.js"))
        .expect("read canonical init.js example");
    assert_eq!(
        example
            .matches("bindKey(\"Ctrl+B\", \"workspace.toggleFileBrowser\"")
            .count(),
        1,
        "canonical init.js must contain one active Ctrl+B workspace-toggle example"
    );
    assert!(
        root.join("docs/index.md").is_file()
            && std::fs::read_to_string(root.join("docs/index.md"))
                .expect("read docs index")
                .contains("reference/clay-js-api/configuration.md")
    );
}

#[test]
fn canonical_example_covers_theme_typography_and_modular_configuration() {
    let root = repository_root();
    let example = std::fs::read_to_string(root.join("examples/init.js"))
        .expect("read canonical init.js example");

    for import in [
        r#"import { loadConfigurationModule, getConfigurationState } from "clay:configuration";"#,
        r#"import { setTheme, setTypography, setAppearance } from "clay:theme";"#,
        r#"import { clientSetCursorStyle } from "clay:editor";"#,
        r#"import { clientSetEditorLayout } from "clay:editor";"#,
        r#"import { bindKey, unbindKey } from "clay:keybindings";"#,
        r#"import { setPaneFocusPolicy } from "clay:shell";"#,
        r#"import { setSyntaxEnginePreference } from "clay:syntax";"#,
        r#"import { clientExecuteEditorCommand } from "clay:editor";"#,
    ] {
        assert_eq!(
            example.matches(import).count(),
            1,
            "canonical example must import each configuration facade exactly once: {import}"
        );
    }

    assert_eq!(
        example
            .matches("\nsetTheme(\"@clay/theme-gruvbox-material-dark\");")
            .count(),
        1,
        "canonical example must keep one active explicit theme selection"
    );
    assert_eq!(
        example.matches("\nsetTypography({").count(),
        1,
        "canonical example must keep one active atomic typography call"
    );
    for theme in [
        "@clay/theme-gruvbox-material-light",
        "@clay/theme-modus-operandi",
        "@clay/theme-modus-vivendi",
    ] {
        assert!(
            example.contains(&format!("// setTheme(\"{theme}\");")),
            "canonical example must show {theme} as a documented alternative"
        );
    }
    for hierarchy_field in [
        "    display: 1.5,",
        "    title: 14 / 12,",
        "    section: 13 / 12,",
        "    body: 1,",
        "    status: 1,",
        "    detail: 10 / 12,",
        "    caption: 0.75,",
    ] {
        assert_eq!(
            example.matches(hierarchy_field).count(),
            1,
            "canonical example must document hierarchy field exactly once: {hierarchy_field}"
        );
    }
    assert!(
        example.contains("// setAppearance(\"dark\");"),
        "canonical example must show appearance as an optional alternative to explicit setTheme"
    );
    assert!(
        example.contains("path: \"./packages/first-party.js\"")
            && example.contains("path: \"./packages/third-party.js\""),
        "canonical example must keep package configuration loads modular and optional"
    );

    let configuration =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration API doc");
    for marker in [
        "examples/` tree",
        "theme.setTheme",
        "theme.setTypography",
        "theme.setAppearance",
        "loadConfigurationModule",
        "No hidden JSON/TOML",
    ] {
        assert!(
            configuration.contains(marker),
            "configuration docs must cover canonical example marker {marker}"
        );
    }
}

#[test]
fn phase28_canonical_example_lists_all_bindable_editor_commands() {
    let root = repository_root();
    let example = std::fs::read_to_string(root.join("examples/init.js"))
        .expect("read canonical init.js example");

    for (command, binding) in [
        (
            "editor.toggleComment",
            "\"Ctrl+/\": \"editor.toggleComment\",",
        ),
        (
            "editor.toggleListMarker",
            "// bindKey(\"Ctrl+Shift+8\", \"editor.toggleListMarker\", { scope: \"editor\" });",
        ),
        (
            "editor.rotateHeading",
            "// bindKey(\"Ctrl+Alt+1\", \"editor.rotateHeading\", { scope: \"editor\" });",
        ),
        (
            "editor.clientToggleFold",
            "// bindKey(\"Ctrl+Shift+F\", \"editor.clientToggleFold\", { scope: \"editor\" });",
        ),
        (
            "editor.toggleInlayHints",
            "// bindKey(\"Ctrl+Alt+I\", \"editor.toggleInlayHints\", { scope: \"editor\" });",
        ),
    ] {
        assert_eq!(
            example.matches(command).count(),
            2,
            "canonical example must mention {command} once in its Phase 28 description and once in its binding"
        );
        assert_eq!(
            example.matches(binding).count(),
            1,
            "canonical example must bind {command} exactly once"
        );
    }

    for marker in [
        "editor.toggleComment    argless client-first edit; default Ctrl+/;",
        "editor.toggleListMarker argless client-first edit; no core default chord;",
        "editor.rotateHeading    argless client-first edit; no core default chord;",
        "editor.clientToggleFold argless client-UI command; no core default chord;",
        "editor.toggleInlayHints argless client-UI command; no core default chord;",
    ] {
        assert!(
            example.contains(marker),
            "canonical example must document Phase 28 command metadata: {marker}"
        );
    }
}

#[test]
fn canonical_example_cross_checks_editor_layout_options_against_inventory() {
    // Phase 26 configuration task: the canonical example's new editor-layout
    // option names/enums/defaults must cross-check against the validated
    // server-side parser surface (api-inventory.toml custom_properties), not
    // prose. The example documents each option exactly once, the enum values
    // match the inventory entry, and the configuration guide names the API.
    let root = repository_root();
    let example = std::fs::read_to_string(root.join("examples/init.js"))
        .expect("read canonical init.js example");

    // The example must document the option names and the bounded enum in the
    // options annotation, and keep exactly one ACTIVE (uncommented) call.
    assert!(
        example.contains("wrapPolicy  \"none\" | \"viewport\" | \"column\"   (required)"),
        "canonical example must annotate the wrapPolicy enum"
    );
    assert!(
        example.contains("columnCap   number   column cap for \"column\" (default 72, clamped to"),
        "canonical example must annotate columnCap with type/default"
    );
    assert_eq!(
        example
            .matches("\nclientSetEditorLayout({ wrapPolicy: \"column\", columnCap: 72 });")
            .count(),
        1,
        "canonical example must keep one active editor-layout call"
    );

    // The inventory entry's custom_properties must name the same options, and
    // the doc must state the same enum and clamp.
    let registry = ClayJsApiRegistry::from_docs(&root).expect("build registry from docs");
    let layout = registry
        .by_id("editor.clientSetEditorLayout")
        .expect("editor.clientSetEditorLayout must be a registered public configuration API");
    assert_eq!(layout.visibility, "public");
    assert_eq!(layout.js_module, "clay:editor");
    let property_names: BTreeSet<_> = layout
        .custom_properties
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert!(
        property_names.contains("wrapPolicy") && property_names.contains("columnCap"),
        "inventory must list wrapPolicy and columnCap in custom_properties, got {property_names:?}"
    );
    let doc = std::fs::read_to_string(
        root.join("docs/reference/clay-js-api/editor/client-set-editor-layout.md"),
    )
    .expect("read clientSetEditorLayout API doc");
    for marker in [
        "\"none\" | \"viewport\" | \"column\"",
        "clamped to 16–240",
        "package-unforgeable",
    ] {
        assert!(
            doc.contains(marker),
            "clientSetEditorLayout doc must state {marker}"
        );
    }

    // The configuration guide must name the new configuration surface.
    let configuration =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration API doc");
    assert!(
        configuration.contains("clientSetEditorLayout"),
        "configuration.md must cover the editor-layout configuration surface"
    );
}

#[test]
fn configuration_api_documents_phase20_6_appearance_and_precedence() {
    // Plan 067 task 11: Phase 20.6 treats appearance + persistence as
    // documented Clay JS configuration APIs. setAppearance is a registry-
    // public `clay:theme` API (not an undocumented config key), its
    // behavior-changing `appearance` input is in `custom_properties`, the
    // configuration guide documents the precedence model + ui-session source,
    // and every configuration API denies authority.
    let root = repository_root();
    let registry = ClayJsApiRegistry::from_docs(&root).expect("build registry from docs");

    // setAppearance is registry-public, in clay:theme, with `appearance` in
    // custom_properties (behavior-changing setting present in custom_properties).
    let appearance = registry
        .by_id("theme.setAppearance")
        .expect("theme.setAppearance must be a registered public configuration API");
    assert_eq!(appearance.visibility, "public");
    assert_eq!(appearance.js_module, "clay:theme");
    assert_eq!(appearance.phase, "Phase 20.6");
    let property_names: BTreeSet<_> = appearance
        .custom_properties
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert!(
        property_names.contains("appearance"),
        "setAppearance must list `appearance` in custom_properties, got {property_names:?}"
    );

    // The appearance enum is bounded; the security/denied sections reject
    // out-of-enum and authority grants.
    assert!(
        appearance.security.contains("light") && appearance.security.contains("dark"),
        "setAppearance security must state the bounded enum"
    );
    for denied in denied_configuration_authorities() {
        assert!(
            appearance.security.contains(denied),
            "theme.setAppearance must deny {denied} authority"
        );
    }

    // configuration.md documents the Phase 20.6 precedence model + setAppearance.
    let configuration =
        std::fs::read_to_string(root.join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration doc");
    assert!(
        configuration.contains("theme.setAppearance"),
        "configuration.md must link setAppearance"
    );
    assert!(
        configuration.contains("ui-session"),
        "configuration.md must document the ui-session persisted-preference source"
    );
    assert!(
        configuration.contains("preferences.json"),
        "configuration.md must document the preferences.json persistence store"
    );
    // Precedence order: canonical/package default < init-js < ui-session.
    let precedence_anchor = configuration.find("ui-session");
    let init_js_anchor = configuration.find("init-js");
    assert!(
        precedence_anchor.is_some() && init_js_anchor.is_some(),
        "configuration.md must name both ui-session and init-js sources"
    );

    // set-package-option.md documents the extended source taxonomy.
    let set_package_option = std::fs::read_to_string(
        root.join("docs/reference/clay-js-api/configuration/set-package-option.md"),
    )
    .expect("read setPackageOption API doc");
    assert!(
        set_package_option.contains("ui-session"),
        "setPackageOption must document the `ui-session` source"
    );

    // The closed clay:configuration module did not gain appearance: appearance
    // is a clay:theme API, and clay:configuration stays closed (setPackageOption
    // + loadConfigurationModule + getConfigurationState only).
    assert!(
        registry.by_id("configuration.setAppearance").is_none(),
        "appearance must not be a clay:configuration API; it lives in clay:theme"
    );
}
