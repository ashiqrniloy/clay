use super::*;

#[tokio::test]
async fn language_commands_are_package_prefixed_and_server_first_with_provenance() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { loadPackage } from "clay:packages";
            import { serverListCommands } from "clay:commands";
            await loadPackage("@clay/rust");
            await loadPackage("@clay/typescript");
            await loadPackage("@clay/javascript");
            await loadPackage("@clay/markdown");
            Deno.core.ops.op_clay_runtime_record(JSON.stringify(serverListCommands()));
            "#,
        )
        .await
        .unwrap();

    let commands: serde_json::Value = serde_json::from_str(&result.op_records[0]).unwrap();
    for (command_id, package_name, api_prefix, declaration_source, _load_source) in [
        (
            "rust.toggleLineComment",
            "@clay/rust",
            "rust",
            include_str!("../../../../packages/rust/dist/index.js"),
            include_str!("../../../../packages/rust/dist/load.js"),
        ),
        (
            "typescript.toggleLineComment",
            "@clay/typescript",
            "typescript",
            include_str!("../../../../packages/typescript/dist/index.js"),
            include_str!("../../../../packages/typescript/dist/load.js"),
        ),
        (
            "javascript.toggleLineComment",
            "@clay/javascript",
            "javascript",
            include_str!("../../../../packages/javascript/dist/index.js"),
            include_str!("../../../../packages/javascript/dist/load.js"),
        ),
        (
            "markdown.toggleComment",
            "@clay/markdown",
            "markdown",
            include_str!("../../../../packages/markdown/dist/index.js"),
            include_str!("../../../../packages/markdown/dist/load.js"),
        ),
    ] {
        let command = commands
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["commandId"] == command_id)
            .unwrap_or_else(|| panic!("missing {command_id}"));
        assert_eq!(command["packageName"], package_name);
        assert_eq!(command["packageVersion"], "0.1.0");
        assert_eq!(command["apiPrefix"], api_prefix);
        assert!(declaration_source.contains(command_id));
        assert!(
            declaration_source.contains("routingPolicy: \"server-first\"")
                || declaration_source.contains("routingPolicy: \"ServerFirst\"")
        );
        assert!(declaration_source.contains("permissions: []"));
    }

    for (component_id, package_name, api_prefix) in [
        ("rust.status.mode", "@clay/rust", "rust"),
        ("typescript.status.mode", "@clay/typescript", "typescript"),
        ("javascript.status.mode", "@clay/javascript", "javascript"),
        ("markdown.status.mode", "@clay/markdown", "markdown"),
    ] {
        let component = result
            .ui_contributions
            .components
            .iter()
            .find(|component| component.id == component_id)
            .unwrap_or_else(|| panic!("missing {component_id}"));
        assert_eq!(component.root_kind, "statusItem");
        assert_eq!(component.provenance.package_name, package_name);
        assert_eq!(component.provenance.package_version, "0.1.0");
        assert_eq!(component.provenance.api_prefix, api_prefix);
    }
}

#[test]
fn language_mode_registration_has_no_per_language_rust_branch() {
    let sources = [
        include_str!("../../ops/modes.rs"),
        include_str!("../../../packages/modes.rs"),
        include_str!("../../../packages/commands.rs"),
    ];
    for source in sources {
        for mode in ["rust", "typescript", "javascript", "markdown"] {
            assert!(!source.contains(&format!("mode_id == \"{mode}\"")));
            assert!(!source.contains(&format!("mode_id == {mode:?}")));
        }
    }
}

#[tokio::test]
async fn build_code_editing_manifest_produces_valid_editor_rules() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { buildCodeEditingManifest } from "clay:behavior";
            import { loadPackage } from "clay:packages";
            import { serverClassifyDocument } from "clay:modes";

            // @clay/javascript now uses buildCodeEditingManifest for its editor rules.
            // Loading the package exercises the manifest validator; classifying a
            // matching document proves the mode pattern (built from helper output)
            // was registered successfully.
            const summary = await loadPackage("@clay/javascript");
            const classification = serverClassifyDocument({ documentId: 7, path: "src/index.js" });

            const rules = buildCodeEditingManifest({
              indentSize: 4,
              lineComment: "//",
              enter: { kind: "continueLineMarkers", markers: ["-"], exitOnEmptyItem: true },
              pairs: [{ open: "(", close: ")" }],
              electricOutdentCharacters: ["}", ")", "]", "xx", "}"],
              autocompleteTriggers: [".", "::", ":", ":"]
            });

            Deno.core.ops.op_clay_runtime_record(JSON.stringify({
              modeId: classification.modeId,
              apiPrefix: classification.apiPrefix,
              packageName: classification.packageName,
              packageVersion: classification.packageVersion,
              summaryModes: summary.modes,
              rulesEnterKind: rules.enter.kind,
              rulesTabSpaces: rules.tabSpaces,
              rulesPairCount: rules.pairs.length,
              rulesCommentCount: rules.comments.length,
              rulesElectricCount: rules.electricCharacters.length,
              rulesAutocompleteCount: rules.autocompleteTriggers.length
            }));
            "#,
        )
        .await
        .unwrap();

    let record = result.op_records.into_iter().next().expect("one record");
    let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
    assert_eq!(parsed["modeId"], "javascript");
    assert_eq!(parsed["apiPrefix"], "javascript");
    assert_eq!(parsed["packageName"], "@clay/javascript");
    assert_eq!(parsed["packageVersion"], "0.1.0");
    assert_eq!(parsed["rulesEnterKind"], "continueLineMarkers");
    assert_eq!(parsed["rulesTabSpaces"], 4);
    assert_eq!(parsed["rulesPairCount"], 1);
    assert_eq!(parsed["rulesCommentCount"], 1);
    assert_eq!(parsed["rulesElectricCount"], 3);
    assert_eq!(parsed["rulesAutocompleteCount"], 2);
}

#[tokio::test]
async fn language_packages_classify_with_core_fallbacks_and_no_conflicts() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { loadPackage } from "clay:packages";
            import { serverClassifyDocument } from "clay:modes";
            import { serverListCommands } from "clay:commands";
            import { serverListCompletionProvidersForTrigger } from "clay:completion";

            await loadPackage("@clay/rust");
            await loadPackage("@clay/typescript");
            await loadPackage("@clay/javascript");

            const classifications = {
              rust: serverClassifyDocument({ documentId: 1, path: "src/main.rs" }),
              typescript: serverClassifyDocument({ documentId: 2, path: "src/index.ts" }),
              javascript: serverClassifyDocument({ documentId: 3, path: "src/index.js" }),
              plainText: serverClassifyDocument({ documentId: 4, path: "README.txt" }),
              unknownCode: serverClassifyDocument({ documentId: 5, path: "prog.py" }),
            };

            const commands = serverListCommands();
            const commandIds = commands.map((command) => command.commandId).sort();
            const dotProviders = serverListCompletionProvidersForTrigger({ trigger: "." });
            const providerIds = dotProviders.providers.map((provider) => provider.id).sort();

            Deno.core.ops.op_clay_runtime_record(JSON.stringify({
              classifications,
              commandIds,
              providerIds,
              commandCount: commands.length,
              providerCount: dotProviders.providers.length
            }));
            "#,
        )
        .await
        .unwrap();

    let record = result.op_records.into_iter().next().expect("one record");
    let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");

    // Package-declared modes win over core.code for known extensions.
    assert_eq!(parsed["classifications"]["rust"]["modeId"], "rust");
    assert_eq!(
        parsed["classifications"]["typescript"]["modeId"],
        "typescript"
    );
    assert_eq!(
        parsed["classifications"]["javascript"]["modeId"],
        "javascript"
    );

    // Plain text falls back to core.text; unmatched code-like extension falls back to core.code.
    assert_eq!(
        parsed["classifications"]["plainText"]["modeId"],
        "core.text"
    );
    assert_eq!(
        parsed["classifications"]["unknownCode"]["modeId"],
        "core.code"
    );

    // No duplicate command or provider IDs across packages.
    assert_eq!(parsed["commandCount"], 3);
    assert_eq!(
        parsed["commandIds"],
        serde_json::json!([
            "javascript.toggleLineComment",
            "rust.toggleLineComment",
            "typescript.toggleLineComment"
        ])
    );
    assert_eq!(parsed["providerCount"], 5);
    assert_eq!(
        parsed["providerIds"],
        serde_json::json!([
            "javascript.keywords",
            "rust.keywords",
            "rust.snippets",
            "typescript.keywords",
            "typescript.snippets"
        ])
    );
}

#[tokio::test]
async fn language_package_classification_is_deterministic_across_load_orders() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;

    for (first, second, third) in [
        ("@clay/rust", "@clay/typescript", "@clay/javascript"),
        ("@clay/javascript", "@clay/rust", "@clay/typescript"),
        ("@clay/typescript", "@clay/javascript", "@clay/rust"),
    ] {
        let source = format!(
            r#"
            import {{ loadPackage }} from "clay:packages";
            import {{ serverClassifyDocument }} from "clay:modes";

            await loadPackage("{}");
            await loadPackage("{}");
            await loadPackage("{}");

            const rust = serverClassifyDocument({{ documentId: 10, path: "lib.rs" }});
            const ts = serverClassifyDocument({{ documentId: 11, path: "app.ts" }});
            const js = serverClassifyDocument({{ documentId: 12, path: "app.js" }});

            Deno.core.ops.op_clay_runtime_record(JSON.stringify({{
              rust: rust.modeId,
              typescript: ts.modeId,
              javascript: js.modeId
            }}));
            "#,
            first, second, third
        );
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module_for_document(source, 88)
            .await
            .unwrap();

        let record = result.op_records.into_iter().next().expect("one record");
        let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
        assert_eq!(parsed["rust"], "rust");
        assert_eq!(parsed["typescript"], "typescript");
        assert_eq!(parsed["javascript"], "javascript");
    }
}

#[tokio::test]
async fn language_package_rejects_unauthorized_completion_provider() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let error = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";

            serverRegisterCompletionProvider({
              providerId: "evil.keywords",
              triggerCharacters: ["."]
            });
            "#,
        )
        .await
        .unwrap_err();

    let message = error.to_string();
    assert!(
        message.contains("packages.no_active_package"),
        "expected no-active-package provenance error, got: {message}"
    );
}

#[tokio::test]
async fn primitive_facades_return_actionable_validation_errors() {
    let error = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { serverValidatePackagePermissions } from "clay:packages";
            serverValidatePackagePermissions(["network"]);
            "#,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    assert!(error.to_string().contains("packages.prohibited_authority"));
    assert!(error.to_string().contains("network"));
}

#[tokio::test]
async fn primitive_configuration_facades_promote_package_options_only() {
    let error = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { setPackageOption, setModePreference, setDecorationTheme, setParsePolicy } from "clay:configuration";
            if ([setPackageOption, setModePreference, setDecorationTheme, setParsePolicy].some((api) => typeof api !== "function")) {
              throw new Error("configuration primitive facade export missing");
            }
            setModePreference({ modeId: "markdown", source: "init-js" });
            "#,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    assert!(
        error
            .to_string()
            .contains("configuration.setModePreference is planned")
    );
}

#[tokio::test]
async fn markdown_large_file_parse_policy_rejects_unsafe_values() {
    for (name, policy_fields, expected) in [
        (
            "zero timeout",
            "timeoutMs: 0, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 30 * 1024 * 1024",
            "timeoutMs must be between 1 and 5000",
        ),
        (
            "oversized timeout",
            "timeoutMs: 5001, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 30 * 1024 * 1024",
            "timeoutMs must be between 1 and 5000",
        ),
        (
            "zero cache budget",
            "timeoutMs: 50, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 0",
            "window and memory budgets must be non-zero",
        ),
        (
            "window larger than cache budget",
            "timeoutMs: 50, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 1024",
            "window and memory budgets must be non-zero",
        ),
        (
            "unbounded cache budget",
            "timeoutMs: 50, maxWindowBytes: 64 * 1024, memoryBudgetBytes: 64 * 1024 * 1024",
            "window and memory budgets must be non-zero",
        ),
    ] {
        let source = format!(
            r#"
            import {{ serverRegisterParseHandler }} from "clay:parse";
            serverRegisterParseHandler({{
              mode: "markdown",
              parseUnit: "line-group",
              viewportPriority: true,
              {policy_fields}
            }});
            "#
        );
        let service = ClayJsRuntimeService::default();
        let error = evaluate_as_package(
            &service,
            test_package_json(
                "@clay/markdown-policy",
                "markdown",
                &["parse-document"],
                serde_json::json!({}),
            ),
            vec![crate::packages::permissions::PackagePermission::ParseDocument],
            &source,
        )
        .await
        .unwrap_err();

        assert!(
            matches!(error, ClayRuntimeError::Runtime(_)),
            "{name} should fail in the runtime"
        );
        assert!(
            error.to_string().contains(expected),
            "{name} should reject unsafe parse policy with `{expected}`, got {error}"
        );
    }
}
