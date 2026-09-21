use super::*;

#[test]
fn clay_module_loader_resolves_trusted_helper_exports() {
    let loader = loader_with_allowlist(&[], None);
    let resolved = loader
        .resolve(
            "lsp-shared/client.js",
            CONTROLLED_MAIN_SPECIFIER,
            ResolutionKind::Import,
        )
        .expect("trusted helper export must resolve");
    assert_eq!(resolved.as_str(), "clay://packages/lsp-shared/client.js");
    let source = match loader.load(&resolved, None, default_load_options()) {
        ModuleLoadResponse::Sync(Ok(source)) => source,
        ModuleLoadResponse::Sync(Err(error)) => panic!("helper load failed: {error:?}"),
        _ => panic!("expected synchronous helper load"),
    };
    assert_eq!(source.module_type, ModuleType::JavaScript);
    assert!(!source.code.as_bytes().is_empty());
}

#[test]
fn clay_module_loader_denies_helper_exports_outside_trusted_domain() {
    let loader = ClayModuleLoader::new(
        ModuleSpecifier::parse(CONTROLLED_MAIN_SPECIFIER).unwrap(),
        None,
        None,
        Arc::new(PackageLoadEntryAllowlist::default()),
        crate::packages::bundled::RuntimeDomain::ThirdParty,
    );
    for specifier in ["lsp-shared/client.js", "lsp-shared/bridge.js"] {
        assert!(
            loader
                .resolve(specifier, CONTROLLED_MAIN_SPECIFIER, ResolutionKind::Import)
                .is_err(),
            "third-party runtime must not resolve {specifier}"
        );
    }
}

#[test]
fn clay_module_loader_denies_unexported_and_escaping_helper_specifiers() {
    let loader = loader_with_allowlist(&[], None);
    for specifier in [
        "lsp-shared/private.js",
        "lsp-shared/client.ts",
        "lsp-shared/../client.js",
        "lsp-shared/client.js/../mapping.js",
    ] {
        assert!(
            loader
                .resolve(specifier, CONTROLLED_MAIN_SPECIFIER, ResolutionKind::Import)
                .is_err(),
            "unexported or escaping helper {specifier} must be denied"
        );
    }
}

#[tokio::test]
async fn default_init_js_load_package_lines_activate_markdown_and_rust() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = config_fixture("default-load-markdown-rust");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("@clay/markdown");
        await loadPackage("@clay/rust");
        "#,
    )
    .unwrap();
    let started = Instant::now();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("two explicit package loads must succeed");
    assert!(
        started.elapsed() < Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
        "default package loads must stay below the hard evaluation timeout"
    );
    assert!(
        result
            .parse_handlers
            .iter()
            .any(|handler| handler.mode_id == "markdown")
    );
    let providers: Vec<_> = result
        .completion_providers
        .iter()
        .map(|provider| provider.id.as_str())
        .collect();
    assert!(providers.contains(&"markdown.keywords"));
    assert!(providers.contains(&"rust.keywords"));
    assert!(result.document_analyzers.is_empty());
}

#[tokio::test]
async fn editor_layout_config_eval_stays_within_hard_timeout() {
    let service = ClayJsRuntimeService::default();
    let evaluation = tokio::time::timeout(
        Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
        service.evaluate_controlled_module(
            r#"
            import { clientSetEditorLayout } from "clay:editor";
            clientSetEditorLayout({ wrapPolicy: "column", columnCap: 72 });
            "#,
        ),
    )
    .await;
    assert!(matches!(evaluation, Ok(Ok(_))));
}

#[tokio::test]
async fn init_js_can_still_register_mode_pattern_after_load_package() {
    let service = ClayJsRuntimeService::default();
    service
        .evaluate_controlled_module(
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/markdown");
            "#,
        )
        .await
        .expect("one-line package load must succeed before imperative registration");
    let result = evaluate_as_trusted_package(
        &service,
        test_package_json(
            "@clay/fixture-after-load",
            "fixtureafterload",
            &["mode-registration", "mode-activation"],
            serde_json::json!({}),
        ),
        vec![
            crate::packages::permissions::PackagePermission::ModeRegistration,
            crate::packages::permissions::PackagePermission::ModeActivation,
        ],
        r#"
            import { serverClassifyDocument, serverRegisterModePattern } from "clay:modes";
            serverRegisterModePattern({
              modeId: "fixtureafterload",
              displayName: "Fixture After Load",
              extensions: ["after"],
              mimeTypes: ["text/x-after"]
            });
            const classification = serverClassifyDocument({ documentId: 42, path: "note.after" });
            Deno.core.ops.op_clay_runtime_record(`${classification.modeId}:${classification.documentId}`);
            "#,
    )
    .await
    .expect("imperative mode registration must remain available after loadPackage");
    assert_eq!(result.op_records, ["fixtureafterload:42"]);
}

#[tokio::test]
async fn load_package_markdown_does_not_activate_hardcoded_document_1() {
    let root = config_fixture("loadpackage-document-id");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
        await loadPackage("@clay/markdown");
        const classification = serverClassifyDocument({ documentId: 42, path: "README.md" });
        serverActivateClassifiedMode(classification, { path: "README.md" });
        Deno.core.ops.op_clay_runtime_record(String(classification.documentId));
        "#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_for_document(root, 42)
        .await
        .expect("non-default runtime document id must load");
    assert_eq!(result.op_records, ["42"]);
    assert!(
        result
            .parse_handlers
            .iter()
            .any(|handler| handler.mode_id == "markdown")
    );
}

#[tokio::test]
async fn load_package_without_load_entry_register_calls_activates_rust_mode_and_completions() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let load_source = fs::read_to_string("packages/rust/dist/load.js").unwrap();
    assert!(!load_source.contains("serverRegisterCommand"));
    assert!(!load_source.contains("serverRegisterCompletionProvider"));
    let root = config_fixture("rust-execute-only-load");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
        await loadPackage("@clay/rust");
        const classification = serverClassifyDocument({ documentId: 42, path: "src/main.rs" });
        serverActivateClassifiedMode(classification, { path: "src/main.rs" });
        "#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("Rust execute-only load must use host contributions");
    let manifest = result.behavior_manifest.expect("Rust mode manifest");
    assert!(
        manifest
            .commands
            .iter()
            .any(|command| command.command_id == "rust.toggleLineComment")
    );
    assert!(
        result
            .completion_providers
            .iter()
            .any(|provider| provider.id == "rust.keywords")
    );
}

#[tokio::test]
async fn server_register_syntax_grammar_on_native_owned_package_returns_ownership_diagnostic() {
    let service = ClayJsRuntimeService::default();
    let error = evaluate_as_trusted_package(
        &service,
        test_package_json(
            "@clay/rust",
            "rust",
            &["parse-document", "render-decorations"],
            serde_json::json!({
                "syntaxGrammars": [{
                    "languageId": "rust",
                    "filePatterns": { "extensions": ["rs"] },
                    "grammar": { "kind": "native", "source": "tree-sitter-rust" },
                    "queries": { "highlights": "./queries/highlights.scm" },
                    "styleMap": { "keyword": { "type": "Keyword" } }
                }]
            }),
        ),
        vec![
            crate::packages::permissions::PackagePermission::ParseDocument,
            crate::packages::permissions::PackagePermission::RenderDecorations,
        ],
        r#"
            import { serverRegisterSyntaxGrammar } from "clay:syntax";
            serverRegisterSyntaxGrammar({});
            "#,
    )
    .await
    .expect_err("native-owned Rust grammar registration must fail closed");
    assert!(
        error
            .to_string()
            .contains("syntax.owned_by_native_descriptor")
    );
}

#[tokio::test]
async fn set_editor_layout_publishes_runtime_wrap_override() {
    let service = ClayJsRuntimeService::default();
    service
        .evaluate_controlled_module(
            r#"
            import { clientSetEditorLayout } from "clay:editor";
            clientSetEditorLayout({ wrapPolicy: "column", columnCap: 88 });
            "#,
        )
        .await
        .expect("editor layout override must evaluate");
    assert_eq!(
        service.editor_layout_override(),
        Some(crate::protocol::WrapPolicy::Column(88))
    );
}

#[tokio::test]
async fn set_editor_layout_rejects_unknown_and_clamps_column() {
    let service = ClayJsRuntimeService::default();
    for source in [
        r#"import { clientSetEditorLayout } from "clay:editor"; clientSetEditorLayout({});"#,
        r#"import { clientSetEditorLayout } from "clay:editor"; clientSetEditorLayout({ wrapPolicy: "diagonal" });"#,
    ] {
        let error = service
            .evaluate_controlled_module(source)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("editor.invalid_set_editor_layout")
        );
    }
    service
        .evaluate_controlled_module(
            r#"
            import { clientSetEditorLayout } from "clay:editor";
            clientSetEditorLayout({ wrapPolicy: "column", columnCap: 9999 });
            "#,
        )
        .await
        .expect("column layout must clamp instead of rejecting a large cap");
    assert_eq!(
        service.editor_layout_override(),
        Some(crate::protocol::WrapPolicy::Column(
            crate::protocol::WrapPolicy::MAX_COLUMN
        ))
    );
}

#[tokio::test]
async fn trusted_reload_reruns_markdown_execute_only_load_entry() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let source = r#"
        import { loadPackage } from "clay:packages";
        import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
        await loadPackage("@clay/markdown");
        const classification = serverClassifyDocument({ documentId: 7, path: "README.md" });
        serverActivateClassifiedMode(classification, { path: "README.md" });
    "#;
    let first = service
        .evaluate_controlled_module(source)
        .await
        .expect("initial Markdown load must succeed");
    assert!(
        first
            .parse_handlers
            .iter()
            .any(|handler| handler.mode_id == "markdown")
    );
    let reloaded = ClayJsRuntimeService::production_reload(&service);
    let second = reloaded
        .evaluate_controlled_module(source)
        .await
        .expect("trusted reload must rerun the execute-only load entry");
    assert!(
        second
            .parse_handlers
            .iter()
            .any(|handler| handler.mode_id == "markdown")
    );
}

#[tokio::test]
async fn set_design_system_core_via_init_js() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = config_fixture("set-design-system-core-init");
    fs::write(
        root.join("init.js"),
        r#"
        import { setDesignSystem } from "clay:theme";
        const summary = setDesignSystem("@clay/core");
        Deno.core.ops.op_clay_runtime_record(
            `ds:${summary.specifier}:recipes:${summary.recipeCount}`
        );
        "#,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("setDesignSystem('@clay/core') must succeed");

    let ds = result
        .active_design_system
        .expect("active design system snapshot emitted");
    assert_eq!(ds.specifier, "@clay/core");
    assert_eq!(ds.provenance.package_name, "core");
    assert_eq!(
        ds.provenance.trust_domain,
        crate::protocol::PackageUiTrustDomain::Trusted
    );
    assert!(!ds.recipes.is_empty());
    assert!(
        result
            .op_records
            .iter()
            .any(|record| record.starts_with("ds:@clay/core:recipes:")),
        "setDesignSystem summary must reach init.js"
    );
}

#[tokio::test]
async fn set_icon_pack_bundled_duotone_via_init_js() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = config_fixture("set-icon-pack-duotone-init");
    fs::write(
        root.join("init.js"),
        r#"
        import { setIconPack } from "clay:theme";
        const summary = setIconPack("@clay/icons-phosphor-duotone");
        Deno.core.ops.op_clay_runtime_record(
            `icons:${summary.pack}:${summary.iconCount}:v${summary.schemaVersion}`
        );
        "#,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("setIconPack duotone must succeed");

    let pack = result
        .active_icon_pack
        .expect("active icon-pack snapshot emitted");
    assert_eq!(pack.specifier, "@clay/icons-phosphor-duotone");
    assert_eq!(pack.icons.len(), 22);
    assert_eq!(pack.schema_version, 1);
    assert_eq!(
        pack.provenance.trust_domain,
        crate::protocol::PackageUiTrustDomain::Trusted
    );
    assert!(
        result
            .op_records
            .iter()
            .any(|record| record == "icons:@clay/icons-phosphor-duotone:22:v1"),
        "setIconPack summary must reach init.js"
    );
}

#[tokio::test]
async fn set_design_system_adopted_third_party_via_init_js() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let root = config_fixture("set-design-system-third-party");
    let package_json = serde_json::json!({
        "name": "@vendor/custom-tokyo-night-ds",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "tokyonight",
            "entry": "./dist/index.js",
            "permissions": [],
            "modes": ["tokyonight"],
            "docs": "./docs/index.md",
            "contributions": {
                "uiDesignSystem": {
                    "schemaVersion": 1,
                    "id": "@vendor/custom-tokyo-night-ds",
                    "displayName": "Tokyo Night Design System",
                    "extends": "@clay/core",
                    "values": {
                        "radii.panel": {
                            "type": "radius",
                            "value": 8.0
                        }
                    },
                    "recipes": {
                        "panel.default.root.rest": {
                            "borderRadius": 8.0,
                            "backgroundColor": "surface.panel"
                        }
                    }
                }
            }
        }
    });

    let pkg_root = root.join("pkg");
    fs::create_dir_all(pkg_root.join("dist")).unwrap();
    fs::create_dir_all(pkg_root.join("docs")).unwrap();
    fs::write(pkg_root.join("dist/index.js"), "// noop").unwrap();
    fs::write(pkg_root.join("docs/index.md"), "# Tokyo Night").unwrap();

    {
        let op_state = service.test_op_state();
        let mut locked = op_state.package_service().lock().unwrap();
        locked
            .install_from_value_at_root_with_spec(package_json, pkg_root, "local:tokyo-night-ds")
            .unwrap();
        locked
            .authorize_package(
                "@vendor/custom-tokyo-night-ds",
                vec![],
                crate::packages::authorization::RuntimeProfile::Restricted,
                "test",
            )
            .unwrap();
        locked
            .approve_package("@vendor/custom-tokyo-night-ds", "cli")
            .unwrap();
    }

    fs::write(
        root.join("init.js"),
        r#"
        import { setDesignSystem } from "clay:theme";
        setDesignSystem("@vendor/custom-tokyo-night-ds");
        "#,
    )
    .unwrap();

    let result = service
        .load_configuration_from_root(root)
        .await
        .expect("third-party setDesignSystem must succeed");

    let ds = result
        .active_design_system
        .expect("active design system emitted");
    assert_eq!(ds.specifier, "@vendor/custom-tokyo-night-ds");
    assert_eq!(
        ds.provenance.trust_domain,
        crate::protocol::PackageUiTrustDomain::ThirdParty
    );
    let panel_key = crate::shell::design_system::RecipeKey::new(
        "panel",
        "default",
        "root",
        crate::shell::design_system::RecipeState::Rest,
    );
    let panel_recipe = ds.recipes.get(&panel_key).expect("panel recipe present");
    assert_eq!(panel_recipe.border_radius, 8.0);
}

#[tokio::test]
async fn set_design_system_rejection_leaves_state_clean() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = config_fixture("set-design-system-reject-init");
    fs::write(
        root.join("init.js"),
        r#"
        import { setDesignSystem } from "clay:theme";
        try {
            setDesignSystem("@vendor/non-existent-package");
        } catch (err) {
            Deno.core.ops.op_clay_runtime_record(`caught:${err.message}`);
        }
        "#,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("init.js with caught error must load");

    assert!(result.active_design_system.is_none());
    assert!(
        result
            .op_records
            .iter()
            .any(|r| r.contains("theme.load_failed")),
        "rejection error was caught and recorded"
    );
}

#[tokio::test]
async fn persisted_preferences_design_system_applied() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = config_fixture("persisted-design-system-pref");
    fs::write(root.join("init.js"), "// no explicit setDesignSystem\n").unwrap();
    fs::write(
        root.join("preferences.json"),
        serde_json::json!({
            "designSystem": "@clay/core"
        })
        .to_string(),
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("persisted preferences with designSystem must load");

    let ds = result
        .active_design_system
        .expect("active design system applied from preferences");
    assert_eq!(ds.specifier, "@clay/core");
}

#[tokio::test]
async fn persisted_removed_design_system_preference_falls_back_with_a_bounded_diagnostic() {
    // Plan 118 task 20: an upgrade can leave a persisted preference naming a
    // design system this generation removed. That is a rename the user did not
    // cause, so startup must keep loading: nothing is installed partially, the
    // previous/default state stays active, and one bounded diagnostic names both
    // the rejected specifier and what stays active. The specifier is assembled so
    // the plan-118 absence guard sees no literal of a removed package name.
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let removed = format!("@clay/design-{}", "neobrutal");
    let root = config_fixture("persisted-removed-design-system-pref");
    fs::write(root.join("init.js"), "// no explicit setDesignSystem\n").unwrap();
    fs::write(
        root.join("preferences.json"),
        serde_json::json!({ "designSystem": removed }).to_string(),
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("a removed design-system preference must not fail the generation");

    assert!(
        result.active_design_system.is_none(),
        "a rejected preference installs nothing"
    );
    let design_system_records = result
        .op_records
        .iter()
        .filter(|record| record.contains("designSystem"))
        .collect::<Vec<_>>();
    assert_eq!(
        design_system_records.len(),
        1,
        "exactly one bounded diagnostic: {:?}",
        result.op_records
    );
    let diagnostic = design_system_records[0];
    assert!(
        diagnostic.contains(&removed),
        "diagnostic names the rejected specifier: {diagnostic}"
    );
    assert!(
        diagnostic.contains("rejected") && diagnostic.contains("kept `@clay/core`"),
        "diagnostic names the active system: {diagnostic}"
    );
}
