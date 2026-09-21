use super::*;

#[tokio::test]
async fn js_parse_handler_bridge_runs_registered_markdown_handler() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let evaluation = service
        .evaluate_controlled_module(
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("@clay/markdown");
            "#,
        )
        .await
        .unwrap();
    assert_eq!(evaluation.js_parse_handlers.len(), 1);

    let coordinator = ParseCoordinator::new();
    service
        .register_parse_handlers(&coordinator, 1, &evaluation)
        .unwrap();

    let text = "# Title\n";
    let request = ParseScheduleRequest {
        document_id: 1,
        document_version: 1,
        behavior_version: 1 as BehaviorVersion,
        package_prefix: "markdown".to_string(),
        mode_id: "markdown".to_string(),
        viewport: ParseByteRange::new(0, text.len() as u64),
        invalidated_ranges: vec![ParseByteRange::new(0, text.len() as u64)],
        accepted_edit: None,
        trace_id: None,
        request_id: None,
        client_id: None,
    };
    let windows = vec![ParseWindowSnapshot {
        document_id: 1,
        document_version: 1,
        package_prefix: "markdown".to_string(),
        mode_id: "markdown".to_string(),
        window_id: 0,
        byte_start: 0,
        byte_end: text.len() as u64,
        base_line: 0,
        base_column: 0,
        incremental_edit: false,
        text: text.to_string(),
    }];
    coordinator
        .schedule_parse_with_windows(
            request,
            windows,
            Some(ParsePolicy::new(
                64 * 1024,
                4 * 1024,
                30 * 1024 * 1024,
                5_000,
            )),
        )
        .unwrap();

    let update = tokio::time::timeout(Duration::from_secs(6), coordinator.next_update())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(update.package_prefix, "markdown");
    assert!(
        update
            .decoration_updates
            .iter()
            .any(|set| !set.spans.is_empty()),
        "markdown parser produced syntax decorations"
    );
}

#[tokio::test]
async fn js_parse_handler_bridge_accepts_inert_diagnostic_records() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@clay/fixture-parser",
            "fixture",
            &["parse-document"],
            serde_json::json!({}),
        ),
        vec![crate::packages::permissions::PackagePermission::ParseDocument],
        r#"
            import { serverRegisterParseHandler } from "clay:parse";
            const parser = { default: async (notification) => ({
              diagnostics: {
                source: "fixture-parser",
                spans: [{
                  byteStart: 0,
                  byteEnd: 1,
                  severity: "error",
                  code: "syntax.error",
                  message: "syntax error",
                }],
              },
              viewport: notification.viewport,
            }) };
            serverRegisterParseHandler({
              module: parser,
              mode: "fixture",
            });
            "#,
    )
    .await
    .unwrap();
    let coordinator = ParseCoordinator::new();
    service
        .register_parse_handlers(&coordinator, 1, &evaluation)
        .unwrap();
    coordinator
        .schedule_parse(ParseScheduleRequest {
            document_id: 9,
            document_version: 2,
            behavior_version: 1,
            package_prefix: "fixture".to_string(),
            mode_id: "fixture".to_string(),
            viewport: ParseByteRange::new(0, 8),
            invalidated_ranges: vec![ParseByteRange::new(0, 1)],
            accepted_edit: None,
            trace_id: None,
            request_id: None,
            client_id: None,
        })
        .unwrap();

    let update = tokio::time::timeout(Duration::from_secs(1), coordinator.next_update())
        .await
        .unwrap()
        .unwrap();
    let diagnostics = update.diagnostic_update.unwrap();

    assert_eq!(diagnostics.source, "fixture-parser");
    assert_eq!(diagnostics.spans[0].severity, DiagnosticSeverity::Error);
}

#[tokio::test]
async fn parse_registration_rejects_executable_callbacks_and_missing_permissions() {
    let service = ClayJsRuntimeService::default();
    // Executable callback fields are rejected by the facade before any op.
    let error = service
        .evaluate_controlled_module(
            r#"
            import { serverRegisterParseHandler } from "clay:parse";
            serverRegisterParseHandler({
              mode: "evil",
              handler() {}
            });
            "#,
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("parse.invalid_handler"),
        "unexpected registration error: {error}"
    );
    // Missing approved parse-document capability fails closed.
    let error = evaluate_as_package(
        &service,
        test_package_json("@clay/no-parse", "noparse", &[], serde_json::json!({})),
        vec![],
        r#"
        import { serverRegisterParseHandler } from "clay:parse";
        serverRegisterParseHandler({
          mode: "noparse"
        });
        "#,
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().contains("packages.missing_permission"),
        "unexpected registration error: {error}"
    );
}

#[tokio::test]
async fn syntax_facade_registers_grammar_metadata_without_raw_ops() {
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@clay/fixture-grammar",
            "fixture",
            &["parse-document", "render-decorations"],
            serde_json::json!({
                "syntaxGrammars": [{
                    "languageId": "fixture",
                    "filePatterns": { "extensions": ["fx"] },
                    "grammar": { "kind": "native", "source": "tree-sitter-fixture" },
                    "queries": { "highlights": "./queries/highlights.scm" },
                    "styleMap": {
                      "keyword": { "type": "Keyword" },
                      "string": { "type": "String" },
                      "comment": { "type": "Comment" },
                      "punctuation": { "type": "Operator" }
                    },
                    "budgets": { "timeoutMs": 5000, "maxWindowBytes": 4096 }
                }]
            }),
        ),
        vec![
            crate::packages::permissions::PackagePermission::ParseDocument,
            crate::packages::permissions::PackagePermission::RenderDecorations,
        ],
        r#"
            import { serverRegisterSyntaxGrammar } from "clay:syntax";
            const result = serverRegisterSyntaxGrammar({});
            Deno.core.ops.op_clay_runtime_record(`${result.packagePrefix}:${result.languages[0]}:${result.registeredGrammarCount}`);
            "#,
    )
    .await
    .unwrap();

    assert_eq!(evaluation.op_records, vec!["fixture:fixture:1"]);
    assert!(evaluation.syntax_grammars.iter().any(|grammar| {
        grammar.language_id == "fixture"
            && grammar.engine_tier == crate::server::syntax::SyntaxEngineTier::Native
    }));
}

#[tokio::test]
async fn syntax_facade_engine_preference_allows_explicit_wasm_override() {
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_trusted_package(
        &service,
        test_package_json(
            "@clay/fixture-grammar",
            "fixture",
            &["parse-document", "render-decorations"],
            serde_json::json!({
                "syntaxGrammars": [{
                    "languageId": "fixture",
                    "filePatterns": { "extensions": ["fx"] },
                    "grammar": { "kind": "tree-sitter-wasm", "path": "./grammars/fixture.wasm" },
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
            import { setSyntaxEnginePreference, serverRegisterSyntaxGrammar } from "clay:syntax";
            setSyntaxEnginePreference("fixture", "wasm");
            const result = serverRegisterSyntaxGrammar({});
            Deno.core.ops.op_clay_runtime_record(`${result.packagePrefix}:${result.registeredGrammarCount}`);
            "#,
    )
    .await
    .unwrap();

    assert_eq!(evaluation.op_records, vec!["fixture:1"]);
    assert!(evaluation.syntax_grammars.iter().any(|grammar| {
        grammar.language_id == "fixture"
            && grammar.engine_tier == crate::server::syntax::SyntaxEngineTier::Wasm
    }));
}

#[tokio::test]
async fn javascript_engine_preference_keeps_markdown_tier3_fallback_selected() {
    let root = config_fixture("markdown-javascript-fallback");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        import { setSyntaxEnginePreference } from "clay:syntax";
        setSyntaxEnginePreference("markdown", "javascript");
        await loadPackage("@clay/markdown");
        "#,
    )
    .unwrap();
    let service = ClayJsRuntimeService::default();
    let evaluation = service
        .load_configuration_from_root(root)
        .await
        .expect("Markdown JavaScript preference loads");
    assert_eq!(
        evaluation.syntax_engine_preferences.get("markdown"),
        Some(&crate::server::syntax::SyntaxEngineTier::JavaScriptFallback)
    );
    assert_eq!(evaluation.js_parse_handlers.len(), 1);
    assert!(
        service
            .register_native_syntax_handler(
                &ParseCoordinator::new(),
                1,
                &evaluation,
                "note.md",
                "markdown",
                "markdown",
            )
            .expect("engine selection")
            .is_none(),
        "explicit JavaScript preference must suppress native handler installation"
    );
}

#[tokio::test]
async fn typescript_and_tsx_install_their_selected_native_handlers() {
    let root = config_fixture("typescript-native-handlers");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("@clay/typescript");
        "#,
    )
    .unwrap();
    let service = ClayJsRuntimeService::default();
    let evaluation = service
        .load_configuration_from_root(root)
        .await
        .expect("TypeScript package loads");
    let coordinator = ParseCoordinator::new();

    let typescript = service
        .register_native_syntax_handler(
            &coordinator,
            1,
            &evaluation,
            "app.ts",
            "typescript",
            "typescript",
        )
        .expect("TypeScript native selection")
        .expect("TypeScript native handler");
    let tsx = service
        .register_native_syntax_handler(
            &coordinator,
            1,
            &evaluation,
            "app.tsx",
            "typescript",
            "typescript",
        )
        .expect("TSX native selection")
        .expect("TSX native handler");

    assert_eq!(typescript.0.mode_id, "typescript.typescript");
    assert_eq!(tsx.0.mode_id, "typescript.tsx");
}

#[tokio::test]
async fn syntax_facade_rejects_raw_authority_and_third_party_grammars() {
    let service = ClayJsRuntimeService::default();
    // Raw authority fields are rejected by the facade before any op.
    let error = service
        .evaluate_controlled_module(
            r#"
            import { serverRegisterSyntaxGrammar } from "clay:syntax";
            serverRegisterSyntaxGrammar({ rawOps: true });
            "#,
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("syntax.invalid_grammar"),
        "unexpected syntax registration error: {error}"
    );
    // Third-party grammar contributions are rejected at host-side manifest
    // validation, before any package code runs.
    let error = crate::packages::record::assemble_package_record(&test_package_json(
        "@vendor/rust",
        "vendor-rust",
        &["parse-document", "render-decorations"],
        serde_json::json!({
            "syntaxGrammars": [{
                "languageId": "rust",
                "filePatterns": { "extensions": ["rs"] },
                "grammar": { "kind": "tree-sitter-wasm", "path": "./grammars/rust.wasm" },
                "queries": { "highlights": "./queries/highlights.scm" },
                "styleMap": { "keyword": { "type": "Keyword" } }
            }]
        }),
    ))
    .unwrap_err();
    assert!(
        error.message.contains("first-party-only"),
        "unexpected manifest validation error: {error:?}"
    );
}

#[tokio::test]
async fn completion_facade_registers_provider_metadata_without_raw_ops() {
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_trusted_package(
        &service,
        test_package_json(
            "@vendor/words",
            "words",
            &["completion-provider"],
            serde_json::json!({
                "completionProviders": [{
                    "id": "words.buffer",
                    "priority": 2,
                    "exclusive": true,
                    "triggerCharacters": ["."],
                    "wordBoundaryChars": [".", ","],
                    "budgets": { "timeoutMs": 50, "maxItems": 20 }
                }]
            }),
        ),
        vec![crate::packages::permissions::PackagePermission::CompletionProvider],
        r#"
            import { serverListCompletionProvidersForTrigger, serverRegisterCompletionProvider } from "clay:completion";
            const result = serverRegisterCompletionProvider({});
            const listed = serverListCompletionProvidersForTrigger({ trigger: "." });
            Deno.core.ops.op_clay_runtime_record(`${result.packagePrefix}:${result.providers[0]}:${result.registeredProviderCount}:${result.runtimeBridge}:${listed.providers[0].exclusive}`);
            "#,
    )
    .await
    .unwrap();

    assert_eq!(
        evaluation.op_records,
        vec!["words:words.buffer:1:false:true"]
    );
    assert_eq!(evaluation.completion_providers.len(), 1);
    assert_eq!(evaluation.completion_providers[0].id, "words.buffer");
    assert!(evaluation.completion_providers[0].exclusive);
    assert_eq!(
        evaluation.completion_providers[0].provenance.package_prefix,
        "words"
    );
}

#[tokio::test]
async fn completion_facade_invokes_token_backed_dynamic_provider() {
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@vendor/dynamic",
            "dynamic",
            &["completion-provider"],
            serde_json::json!({
                "completionProviders": [{
                    "id": "dynamic.provider",
                    "triggerCharacters": ["."],
                    "budgets": { "timeoutMs": 500, "maxItems": 8 }
                }]
            }),
        ),
        vec![crate::packages::permissions::PackagePermission::CompletionProvider],
        r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: {
                provideCompletion: async (_request, window) => ({
                  status: "ok",
                  items: [{ label: "dynamic", insertText: "dynamic", detail: window.text }]
                })
              }
            });
            "#,
    )
    .await
    .unwrap();
    let coordinator = crate::server::completion::CompletionCoordinator::new();
    service
        .register_completion_providers(&coordinator, 4, &evaluation)
        .unwrap();
    let request = crate::protocol::CompletionRequest {
        request_id: 91,
        client_id: 2,
        document_id: 7,
        document_version: 3,
        behavior_version: 5,
        cursor_byte_offset: 2,
        replacement_range: crate::protocol::CompletionReplacementRange {
            byte_start: 2,
            byte_end: 2,
        },
        trigger: crate::protocol::CompletionTrigger::Manual,
        provider_generation: 4,
        recent_completions: Box::new([]),
    };
    let reply_rx = coordinator
        .schedule_completion(
            "dynamic.provider",
            request,
            crate::server::completion::CompletionDocumentWindow {
                document_id: 7,
                document_version: 3,
                behavior_version: 5,
                package_prefix: "dynamic".to_string(),
                byte_start: 0,
                byte_end: 2,
                text: "fn".to_string(),
            },
        )
        .unwrap();

    let result = tokio::time::timeout(Duration::from_secs(2), reply_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.items[0].detail, "fn");
}

#[tokio::test]
async fn language_intelligence_facade_registers_token_backed_provider_without_process_authority() {
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_package(
        &service,
        test_package_json("@org/intel", "intel", &["parse-document"], serde_json::json!({})),
        vec![crate::packages::permissions::PackagePermission::ParseDocument],
        r#"
            import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
            const result = serverRegisterLanguageIntelligenceProvider({
              provider: {
                id: "intel.intelligence",
                modes: ["intel"],
                features: ["hover", "definition", "codeAction", "signatureHelp"],
                priority: 10,
                timeoutMs: 500
              },
              module: {
                provideLanguageIntelligence: async () => ({ status: "ok" })
              }
            });
            Deno.core.ops.op_clay_runtime_record(`${result.packagePrefix}:${result.providerId}:${result.runtimeBridge}:${result.languageServerRequired}:${typeof result.token}`);
            "#,
    )
    .await
    .unwrap();

    assert_eq!(
        evaluation.op_records,
        vec!["intel:intel.intelligence:true:false:string"]
    );
    assert_eq!(evaluation.language_intelligence_providers.len(), 1);
    assert_eq!(
        evaluation.language_intelligence_providers[0].id,
        "intel.intelligence"
    );
    assert_eq!(evaluation.js_language_intelligence_providers.len(), 1);
    assert_eq!(
        evaluation.js_language_intelligence_providers[0].export_name,
        "provideLanguageIntelligence"
    );
    assert!(
        !evaluation.language_intelligence_providers[0]
            .provenance
            .package_prefix
            .is_empty()
    );
}

#[tokio::test]
async fn document_analyzer_registration_rejects_unowned_module_and_runtime_process_fields() {
    let service = ClayJsRuntimeService::default();
    for analyzer in [
        r#"{ id: "analysis.worker", contribution: "analysis.server", moduleSpecifier: "file:///tmp/escape.js" }"#,
        r#"{ id: "analysis.worker", contribution: "analysis.server", moduleSpecifier: "clay://packages/other/worker.js", executable: "/bin/true" }"#,
    ] {
        let source = format!(
            r#"
            import {{ serverRegisterDocumentAnalyzer }} from "clay:language";
            serverRegisterDocumentAnalyzer({{
              analyzer: {analyzer}
            }});
            "#
        );
        let error = evaluate_as_package_with_ls_grant(
            &service,
            serde_json::json!({
                "name": "@vendor/analysis",
                "version": "0.1.0",
                "type": "module",
                "exports": { ".": "./dist/index.js" },
                "clay": {
                    "apiPrefix": "analysis",
                    "entry": "./dist/index.js",
                    "permissions": ["parse-document"],
                    "capabilities": ["language-server"],
                    "modes": [],
                    "docs": "./docs/index.md",
                    "contributions": {
                        "languageServers": [{
                            "id": "analysis.server",
                            "executable": "/bin/true",
                            "args": []
                        }]
                    }
                }
            }),
            vec![crate::packages::permissions::PackagePermission::ParseDocument],
            Some((
                "analysis.server",
                std::fs::canonicalize("/bin/true").expect("canonical /bin/true"),
            )),
            &source,
        )
        .await
        .unwrap_err();
        assert!(
            error.to_string().contains("language.invalid_analyzer"),
            "unexpected analyzer registration error: {error}"
        );
    }
}

#[tokio::test]
async fn language_intelligence_facade_rejects_executable_and_process_fields() {
    let service = ClayJsRuntimeService::default();
    for source in [
        r#"
        import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
        serverRegisterLanguageIntelligenceProvider({
          packageName: "@org/intel",
          packagePrefix: "intel",
          permissions: ["parse-document"],
          id: "intel.intelligence",
          features: ["hover"],
          handler: () => {}
        });
        "#,
        r#"
        import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
        serverRegisterLanguageIntelligenceProvider({
          packageName: "@org/intel",
          packagePrefix: "intel",
          permissions: ["parse-document"],
          id: "intel.intelligence",
          features: ["hover"],
          languageServer: true
        });
        "#,
    ] {
        let error = service
            .evaluate_controlled_module_for_document(source, 88)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("language.invalid_provider"),
            "unexpected language registration error: {error}"
        );
    }
}

#[tokio::test]
async fn language_intelligence_js_bridge_publishes_validated_hover_result() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@org/intel",
            "intel",
            &["parse-document"],
            serde_json::json!({}),
        ),
        vec![crate::packages::permissions::PackagePermission::ParseDocument],
        r#"
            import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
            serverRegisterLanguageIntelligenceProvider({
              provider: {
                id: "intel.intelligence",
                modes: ["intel"],
                features: ["hover"],
                priority: 10,
                timeoutMs: 500
              },
              module: {
                provideLanguageIntelligence: async (request, window) => ({
                  status: "ok",
                  markdown: `hover:${request.feature}:${window.text}`,
                  range: { byteStart: 0, byteEnd: window.text.length }
                })
              }
            });
            "#,
    )
    .await
    .unwrap();
    assert_eq!(evaluation.js_language_intelligence_providers.len(), 1);

    let coordinator = crate::server::language_intelligence::LanguageIntelligenceCoordinator::new();
    service
        .register_language_intelligence_providers(&coordinator, 1, &evaluation)
        .unwrap();

    let request = crate::protocol::LanguageIntelligenceRequest {
        request_id: 7,
        client_id: 1,
        document_id: 1,
        document_version: 1,
        behavior_version: 1,
        cursor_byte_offset: 0,
        feature: crate::protocol::LanguageIntelligenceFeature::Hover,
        provider_generation: 1,
    };
    let window = crate::server::language_intelligence::LanguageIntelligenceDocumentWindow {
        document_id: 1,
        document_version: 1,
        behavior_version: 1,
        byte_start: 0,
        byte_end: 4,
        text: "fn()".to_string(),
        active_mode: "intel".to_string(),
    };
    let reply_rx = coordinator
        .schedule(Some("intel.intelligence"), request.clone(), window)
        .unwrap();
    let result = reply_rx.await.expect("js hover result");
    crate::server::language_intelligence::validate_result(&result).unwrap();
    assert_eq!(
        result.status,
        crate::protocol::LanguageIntelligenceStatus::Ok
    );
    assert_eq!(result.request_id, 7);
    assert_eq!(result.provenance.package_prefix, "intel");
    match result.payload {
        crate::protocol::LanguageIntelligencePayload::Hover(hover) => {
            assert_eq!(hover.markdown, "hover:hover:fn()");
            assert_eq!(hover.range, Some(crate::protocol::TextByteRange::new(0, 4)));
        }
        other => panic!("expected hover payload, got {other:?}"),
    }
}

#[tokio::test]
async fn completion_facade_disables_provider_and_filters_trigger_listing() {
    let service = ClayJsRuntimeService::default();
    for (name, prefix) in [("@vendor/words", "words"), ("@vendor/other", "other")] {
        evaluate_as_package(
            &service,
            test_package_json(
                name,
                prefix,
                &["completion-provider"],
                serde_json::json!({
                    "completionProviders": [{
                        "id": format!("{prefix}.buffer"),
                        "triggerCharacters": ["."]
                    }]
                }),
            ),
            vec![crate::packages::permissions::PackagePermission::CompletionProvider],
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({});
            "#,
        )
        .await
        .unwrap();
    }
    let evaluation = service
        .evaluate_controlled_module(
            r#"
            import { serverDisableCompletion, serverListCompletionProvidersForTrigger } from "clay:completion";
            const disabled = serverDisableCompletion({ provider: "words.buffer" });
            const repeated = serverDisableCompletion({ provider: "words.buffer" });
            const packageDisabled = serverDisableCompletion({ packagePrefix: "@vendor/other" });
            const listed = serverListCompletionProvidersForTrigger({ trigger: "." });
            Deno.core.ops.op_clay_runtime_record(`${disabled.target}:${disabled.disabled}:${disabled.providerGeneration}:${repeated.disabled}:${packageDisabled.providerGeneration}:${listed.providers.length}`);
            "#,
        )
        .await
        .unwrap();

    assert_eq!(evaluation.op_records, vec!["words.buffer:true:1:false:2:0"]);
    assert!(evaluation.completion_providers.is_empty());
}

#[tokio::test]
async fn completion_disable_facade_rejects_empty_ambiguous_and_authority_fields() {
    let service = ClayJsRuntimeService::default();
    for source in [
        r#"
        import { serverDisableCompletion } from "clay:completion";
        serverDisableCompletion({});
        "#,
        r#"
        import { serverDisableCompletion } from "clay:completion";
        serverDisableCompletion({ provider: "words.buffer", packagePrefix: "words" });
        "#,
        r#"
        import { serverDisableCompletion } from "clay:completion";
        serverDisableCompletion({ provider: "words.buffer", handler() {} });
        "#,
        r#"
        import { serverDisableCompletion } from "clay:completion";
        serverDisableCompletion({ provider: "x".repeat(129) });
        "#,
    ] {
        let error = service
            .evaluate_controlled_module_for_document(source, 88)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("completion.invalid_disable"));
    }
}

#[tokio::test]
async fn completion_facade_rejects_callbacks_missing_permission_and_bad_prefix() {
    let service = ClayJsRuntimeService::default();
    // Executable callback fields are rejected by the facade before any op.
    let error = service
        .evaluate_controlled_module(
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({ handler() {} });
            "#,
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("completion.invalid_provider"),
        "unexpected completion registration error: {error}"
    );
    // Approved-capability check: enable with the capability granted, then
    // shrink the authorization record; the enabled package's registration
    // now fails closed against the current approved set.
    let package_json = test_package_json(
        "@vendor/nope",
        "nope",
        &["completion-provider"],
        serde_json::json!({
            "completionProviders": [{ "id": "nope.words" }]
        }),
    );
    let root = config_fixture("package-provenance");
    let record = {
        let op_state = service.test_op_state();
        let mut locked = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        locked
            .install_from_value_at_root_with_spec(package_json, root, "local:provenance-test")
            .expect("synthetic package installs");
        locked
            .authorize_package(
                "@vendor/nope",
                vec![crate::packages::permissions::PackagePermission::CompletionProvider],
                crate::packages::authorization::RuntimeProfile::Restricted,
                "test",
            )
            .expect("synthetic package authorizes");
        locked
            .approve_package("@vendor/nope", "test")
            .expect("approves");
        locked.enable("@vendor/nope").expect("enables");
        locked
            .authorize_package(
                "@vendor/nope",
                vec![],
                crate::packages::authorization::RuntimeProfile::Restricted,
                "test",
            )
            .expect("capability shrink authorizes");
        crate::packages::record::assemble_package_record(&serde_json::json!({
            "name": "@vendor/nope",
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": "nope",
                "entry": "./dist/index.js",
                "permissions": ["completion-provider"],
                "modes": ["nope"],
                "docs": "./docs/index.md",
                "contributions": { "completionProviders": [{ "id": "nope.words" }] },
            }
        }))
        .expect("record assembles")
    };
    let error = service
        .evaluate_entry_as_package(
            crate::packages::bundled::RuntimeDomain::Trusted,
            &record,
            RuntimeEntry::ControlledSource(
                r#"
                import { serverRegisterCompletionProvider } from "clay:completion";
                serverRegisterCompletionProvider({});
                "#
                .to_string(),
            ),
            "runtime.evaluate_as_package",
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("packages.missing_permission")
            && error.to_string().contains("completion-provider"),
        "unexpected completion registration error: {error}"
    );
    // Provider ids outside the host package prefix are rejected at
    // host-side manifest validation, before any package code runs.
    let error = crate::packages::record::assemble_package_record(&test_package_json(
        "@vendor/bad",
        "bad",
        &["completion-provider"],
        serde_json::json!({
            "completionProviders": [{ "id": "other.words" }]
        }),
    ))
    .unwrap_err();
    assert!(
        error.message.contains("apiPrefix"),
        "unexpected manifest validation error: {error:?}"
    );
}

#[tokio::test]
async fn language_package_completion_trigger_metadata_is_queryable() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let evaluation = service
        .evaluate_controlled_module(
            r##"
            import { loadPackage } from "clay:packages";
            import { serverDisableCompletion, serverListCompletionProvidersForTrigger } from "clay:completion";

            await loadPackage("@clay/rust");
            await loadPackage("@clay/typescript");
            await loadPackage("@clay/javascript");
            await loadPackage("@clay/markdown");

            const dotProviders = serverListCompletionProvidersForTrigger({ trigger: "." });
            const rustScopeProviders = serverListCompletionProvidersForTrigger({ trigger: ":" });
            const markdownProviders = serverListCompletionProvidersForTrigger({ trigger: "#" });
            const noProviders = serverListCompletionProvidersForTrigger({ trigger: "?" });
            serverDisableCompletion({ packagePrefix: "@clay/rust" });
            const dotProvidersAfterRustDisable = serverListCompletionProvidersForTrigger({ trigger: "." });

            Deno.core.ops.op_clay_runtime_record(JSON.stringify({
              dotIds: dotProviders.providers.map((p) => p.id).sort(),
              rustScopeIds: rustScopeProviders.providers.map((p) => p.id).sort(),
              noCount: noProviders.providers.length,
              rustIdsAfterDisable: dotProvidersAfterRustDisable.providers.filter((p) => p.packagePrefix === "rust").map((p) => p.id),
              rustTriggerCharacters: dotProviders.providers.find((p) => p.id === "rust.keywords")?.triggerCharacters ?? [],
              typescriptTriggerCharacters: dotProviders.providers.find((p) => p.id === "typescript.keywords")?.triggerCharacters ?? [],
              rustPriority: dotProviders.providers.find((p) => p.id === "rust.keywords")?.priority,
              rustItems: dotProviders.providers.find((p) => p.id === "rust.keywords")?.items ?? [],
              rustSnippetItems: dotProviders.providers.find((p) => p.id === "rust.snippets")?.items ?? [],
              typescriptSnippetItems: dotProviders.providers.find((p) => p.id === "typescript.snippets")?.items ?? [],
              markdownIds: markdownProviders.providers.map((p) => p.id),
              markdownItems: markdownProviders.providers.find((p) => p.id === "markdown.keywords")?.items ?? [],
            }));
            "##,
        )
        .await
        .unwrap();

    assert!(
        evaluation
            .completion_providers
            .iter()
            .all(|provider| provider.provenance.package_prefix != "rust")
    );
    let record = evaluation
        .op_records
        .into_iter()
        .next()
        .expect("one record");
    let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
    assert_eq!(
        parsed["dotIds"],
        serde_json::json!([
            "javascript.keywords",
            "rust.keywords",
            "rust.snippets",
            "typescript.keywords",
            "typescript.snippets"
        ])
    );
    assert_eq!(
        parsed["rustScopeIds"],
        serde_json::json!(["rust.keywords", "rust.snippets"])
    );
    assert_eq!(parsed["noCount"], 0);
    assert_eq!(parsed["rustIdsAfterDisable"], serde_json::json!([]));
    assert_eq!(
        parsed["rustTriggerCharacters"],
        serde_json::json!([".", ":"])
    );
    assert_eq!(
        parsed["typescriptTriggerCharacters"],
        serde_json::json!(["."])
    );
    assert_eq!(parsed["rustPriority"], 0);
    assert!(parsed["rustItems"].as_array().unwrap().iter().any(|item| {
        item["label"] == "fn" && item["insertText"] == "fn" && item["textFormat"] == "plainText"
    }));
    assert!(
        parsed["rustSnippetItems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["label"] == "fn" && item["textFormat"] == "snippet")
    );
    assert!(
        parsed["typescriptSnippetItems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["label"] == "interface" && item["textFormat"] == "snippet")
    );
    assert_eq!(
        parsed["markdownIds"],
        serde_json::json!(["markdown.keywords"])
    );
    assert!(
        parsed["markdownItems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["label"] == "# " && item["textFormat"] == "plainText")
    );
}
