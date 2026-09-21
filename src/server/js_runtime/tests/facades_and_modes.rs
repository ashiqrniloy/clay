use super::*;

#[tokio::test]
async fn runtime_imports_modes_commands_and_packages_facades() {
    let service = ClayJsRuntimeService::default();
    let result = evaluate_as_trusted_package(
        &service,
        test_package_json(
            "@clay/markdown-facade",
            "markdown",
            &[
                "mode-registration",
                "mode-activation",
                "command-registration",
                "parse-document",
            ],
            serde_json::json!({
                "commands": [{ "id": "markdown.togglePreview", "displayName": "Toggle Markdown Preview", "routingPolicy": "server-first" }]
            }),
        ),
        vec![
            crate::packages::permissions::PackagePermission::ModeRegistration,
            crate::packages::permissions::PackagePermission::ModeActivation,
            crate::packages::permissions::PackagePermission::CommandRegistration,
            crate::packages::permissions::PackagePermission::ParseDocument,
        ],
        r#"
            import { serverRegisterModePattern, serverActivateMajorMode } from "clay:modes";
            import { serverRegisterCommand, serverListCommands } from "clay:commands";
            import { serverLoadPackage, serverValidatePackagePermissions } from "clay:packages";
            import { serverPublishDecorations } from "clay:decorations";
            import { serverRegisterParseHandler } from "clay:parse";

            if (typeof serverPublishDecorations !== "function" || typeof serverRegisterParseHandler !== "function") {
              throw new Error("decoration/parse facade export missing");
            }
            // serverLoadPackage validates a manifest shape only; it grants
            // no authority and sets no provenance.
            const manifest = {
              name: "@clay/markdown-facade",
              version: "0.1.0",
              clay: {
                apiPrefix: "markdown",
                permissions: ["mode-registration", "mode-activation", "command-registration", "parse-document", "package-configuration"],
                modes: ["markdown"],
                entry: "./dist/index.js",
                loadEntry: "./dist/load.js",
                docs: "./docs/index.md",
                performance: { estimatedManifestBytes: 2048 },
                apiDependencies: ["modes.serverRegisterModePattern", "commands.serverRegisterCommand"],
                contributions: {
                  commands: [{ id: "markdown.togglePreview", displayName: "Toggle Markdown Preview", routingPolicy: "server-first" }],
                  configuration: [{ key: "markdown.preview.enabled", type: "boolean", default: false }]
                }
              }
            };
            const loaded = serverLoadPackage(manifest);
            const permissions = serverValidatePackagePermissions(manifest.clay.permissions);
            serverRegisterModePattern({
              modeId: "markdown",
              displayName: "Markdown",
              extensions: ["md"],
              mimeTypes: ["text/markdown"]
            });
            const activation = serverActivateMajorMode({ documentId: 5, path: "README.md" });
            const command = serverRegisterCommand({
              commandId: "markdown.togglePreview",
              displayName: "Toggle Markdown Preview",
              permissions: ["parse-document"]
            });
            const commands = serverListCommands();
            Deno.core.ops.op_clay_runtime_record(`${loaded.contributions.commands}:${permissions.permissions.length}:${activation.modeId}:${activation.behaviorVersion}:${command.commandId}:${commands.length}`);
            "#,
    )
    .await
    .unwrap();

    assert_eq!(
        result.op_records,
        vec!["1:5:markdown:1:markdown.togglePreview:1"]
    );
}

#[tokio::test]
async fn syntax_grammar_packages_default_load_from_init_js() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = config_fixture("syntax-grammar-init-load");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";

        const loaded = [];
        for (const specifier of ["@clay/rust", "@clay/typescript", "@clay/javascript", "@clay/markdown"]) {
          const summary = await loadPackage(specifier);
          loaded.push(`${summary.name}:${summary.apiPrefix}:${summary.modes.length}:${summary.permissions.join("+")}:${summary.contributions.syntaxGrammars}`);
        }
        Deno.core.ops.op_clay_runtime_record(loaded.join("|"));
        "#,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();

    assert_eq!(
        result.op_records,
        vec![
            "@clay/rust:rust:1:mode-registration+mode-activation+command-registration+completion-provider+parse-document+render-decorations:0|@clay/typescript:typescript:1:mode-registration+mode-activation+command-registration+completion-provider+parse-document+render-decorations:0|@clay/javascript:javascript:1:mode-registration+mode-activation+command-registration+completion-provider+parse-document+render-decorations:0|@clay/markdown:markdown:1:mode-registration+mode-activation+command-registration+completion-provider+parse-document+render-decorations:0"
        ]
    );
}

#[tokio::test]
async fn invalid_mode_font_role_fails_before_registration_and_keeps_core_fallback() {
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { serverClassifyDocument, serverRegisterModePattern } from "clay:modes";
            const manifest = {
              name: "@clay/example", version: "0.1.0", type: "module",
              exports: { ".": "./index.js" },
              clay: {
                apiPrefix: "example", entry: "./index.js",
                permissions: ["mode-registration", "mode-activation"],
                modes: ["example"], docs: "./docs/index.md"
              }
            };
            try {
              serverRegisterModePattern(manifest, {
                modeId: "example", extensions: ["rs"], defaultFontRole: "serif"
              });
            } catch {}
            const classification = serverClassifyDocument({ documentId: 9, path: "main.rs" });
            Deno.core.ops.op_clay_runtime_record(classification.modeId);
            "#,
        )
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["core.code"]);
}

#[tokio::test]
async fn rust_package_expansion_registers_mode_command_completion_and_status() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { loadPackage } from "clay:packages";
            import { serverClassifyDocument } from "clay:modes";
            import { serverListCommands } from "clay:commands";

            const summary = await loadPackage("@clay/rust");
            const classification = serverClassifyDocument({ documentId: 42, path: "src/main.rs" });
            const commands = serverListCommands();
            const rustCommand = commands.find((command) => command.commandId === "rust.toggleLineComment");

            Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                apiPrefix: summary.apiPrefix,
                modes: summary.modes,
                commands: summary.contributions.commands,
                uiComponents: summary.contributions.uiComponents,
                classification,
                rustCommandRegistered: Boolean(rustCommand)
            }));
            "#,
        )
        .await
        .unwrap();

    let record = result.op_records.into_iter().next().expect("one record");
    let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
    assert_eq!(parsed["apiPrefix"], "rust");
    assert_eq!(parsed["modes"], serde_json::json!(["rust"]));
    assert_eq!(parsed["classification"]["modeId"], "rust");
    assert_eq!(parsed["classification"]["apiPrefix"], "rust");
    assert!(parsed["rustCommandRegistered"].as_bool().unwrap());
    assert_eq!(parsed["commands"], 1);
    assert_eq!(parsed["uiComponents"], 1);
}

#[tokio::test]
async fn typescript_package_expansion_registers_mode_command_completion_and_status() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { loadPackage } from "clay:packages";
            import { serverClassifyDocument } from "clay:modes";
            import { serverListCommands } from "clay:commands";

            const summary = await loadPackage("@clay/typescript");
            const classification = serverClassifyDocument({ documentId: 42, path: "src/index.ts" });
            const commands = serverListCommands();
            const tsCommand = commands.find((command) => command.commandId === "typescript.toggleLineComment");

            Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                apiPrefix: summary.apiPrefix,
                modes: summary.modes,
                commands: summary.contributions.commands,
                uiComponents: summary.contributions.uiComponents,
                classification,
                tsCommandRegistered: Boolean(tsCommand)
            }));
            "#,
        )
        .await
        .unwrap();

    let record = result.op_records.into_iter().next().expect("one record");
    let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
    assert_eq!(parsed["apiPrefix"], "typescript");
    assert_eq!(parsed["modes"], serde_json::json!(["typescript"]));
    assert_eq!(parsed["classification"]["modeId"], "typescript");
    assert_eq!(parsed["classification"]["apiPrefix"], "typescript");
    assert!(parsed["tsCommandRegistered"].as_bool().unwrap());
    assert_eq!(parsed["commands"], 1);
    assert_eq!(parsed["uiComponents"], 1);
}

#[tokio::test]
async fn javascript_package_expansion_registers_mode_command_completion_and_status() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { loadPackage } from "clay:packages";
            import { serverClassifyDocument } from "clay:modes";
            import { serverListCommands } from "clay:commands";

            const summary = await loadPackage("@clay/javascript");
            const classification = serverClassifyDocument({ documentId: 42, path: "src/index.js" });
            const commands = serverListCommands();
            const jsCommand = commands.find((command) => command.commandId === "javascript.toggleLineComment");

            Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                apiPrefix: summary.apiPrefix,
                modes: summary.modes,
                commands: summary.contributions.commands,
                uiComponents: summary.contributions.uiComponents,
                classification,
                jsCommandRegistered: Boolean(jsCommand)
            }));
            "#,
        )
        .await
        .unwrap();

    let record = result.op_records.into_iter().next().expect("one record");
    let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
    assert_eq!(parsed["apiPrefix"], "javascript");
    assert_eq!(parsed["modes"], serde_json::json!(["javascript"]));
    assert_eq!(parsed["classification"]["modeId"], "javascript");
    assert_eq!(parsed["classification"]["apiPrefix"], "javascript");
    assert!(parsed["jsCommandRegistered"].as_bool().unwrap());
    assert_eq!(parsed["commands"], 1);
    assert_eq!(parsed["uiComponents"], 1);
}

#[tokio::test]
async fn each_language_mode_registers_indent_electric_pairs_comment_triggers() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let cases = [
        (
            "@clay/rust",
            "src/main.rs",
            4,
            5,
            1,
            vec!["}"],
            vec![".", ":"],
            false,
            6540,
        ),
        (
            "@clay/typescript",
            "src/main.ts",
            2,
            6,
            1,
            vec!["}", ")", "]"],
            vec!["."],
            false,
            6540,
        ),
        (
            "@clay/javascript",
            "src/main.js",
            2,
            6,
            1,
            vec!["}", ")", "]"],
            vec!["."],
            false,
            6540,
        ),
        (
            "@clay/markdown",
            "README.md",
            2,
            5,
            0,
            vec![],
            vec!["#", "[", "`"],
            true,
            // Plan 124 raised every mode layer's manifest by one command
            // declaration and one keymap rule (`shell.toggleAgentLane` on
            // `Ctrl+X Ctrl+P`): 6740 → 6900.
            6900,
        ),
    ];

    for (
        specifier,
        path,
        indent,
        pair_count,
        comment_count,
        electric,
        triggers,
        markdown,
        estimated_bytes,
    ) in cases
    {
        let source = format!(
            r#"
            import {{ loadPackage }} from "clay:packages";
            import {{ serverActivateClassifiedMode, serverClassifyDocument }} from "clay:modes";
            await loadPackage("{specifier}");
            const classification = serverClassifyDocument({{ documentId: 88, path: "{path}" }});
            serverActivateClassifiedMode(classification, {{ path: "{path}" }});
            "#,
        );
        let result = ClayJsRuntimeService::default()
            .evaluate_controlled_module_for_document(source, 88)
            .await
            .unwrap_or_else(|error| panic!("{specifier} should activate: {error}"));
        let manifest = result
            .behavior_manifest
            .unwrap_or_else(|| panic!("{specifier} should publish a behavior manifest"));
        let rules = &manifest.editor_rules;

        assert_eq!(rules.tab.spaces_per_tab, indent, "{specifier} indent");
        assert_eq!(rules.pairs.len(), pair_count, "{specifier} pairs");
        assert_eq!(
            rules.comments.len(),
            if markdown { 1 } else { comment_count },
            "{specifier} comments"
        );
        assert_eq!(
            rules
                .electric_characters
                .iter()
                .map(|rule| rule.trigger.as_str())
                .collect::<Vec<_>>(),
            electric,
            "{specifier} electric characters"
        );
        assert_eq!(
            rules
                .autocomplete_triggers
                .iter()
                .map(|rule| rule.trigger.as_str())
                .collect::<Vec<_>>(),
            triggers,
            "{specifier} autocomplete triggers"
        );
        if markdown {
            assert!(matches!(
                &rules.enter,
                EnterRule::ContinueLineMarkers {
                    markers,
                    exit_on_empty_item: true,
                } if markers == &["-", "*", "+", "ordered-dot"]
            ));
            // Plan 071 task 11: markdown ships prose movement via its
            // manifest — underscore and camelCase carry no meaning in
            // prose. Caret defers to the editor default bar.
            assert_eq!(
                rules.movement.word_separators,
                crate::protocol::WordSeparatorPolicy::Prose,
                "{specifier} prose word separators"
            );
            assert!(
                !rules.movement.treat_underscore_as_word,
                "{specifier} prose underscore policy"
            );
            assert!(
                !rules.movement.camel_case_sub_word,
                "{specifier} prose camelCase policy"
            );
        } else {
            assert!(matches!(rules.enter, EnterRule::PreserveLeadingWhitespace));
            assert_eq!(rules.comments[0].line_prefix, "//");
            // Plan 071 task 11: code packages declare the code movement
            // policy explicitly (identical to the built-in default).
            assert_eq!(
                rules.movement.word_separators,
                crate::protocol::WordSeparatorPolicy::Code,
                "{specifier} code word separators"
            );
            assert!(
                rules.movement.treat_underscore_as_word,
                "{specifier} code underscore policy"
            );
            assert!(
                rules.movement.camel_case_sub_word,
                "{specifier} code camelCase policy"
            );
        }
        // No package ships a caret override today: the reduced-motion-safe
        // editor default bar applies to every mode (customization is opt-in).
        assert_eq!(
            rules.caret_style, None,
            "{specifier} caret defers to default"
        );
        let payload = rkyv::to_bytes::<rkyv::rancor::Error>(&manifest)
            .expect("behavior manifest serializes")
            .len();
        assert!(
            payload <= estimated_bytes,
            "{specifier} payload {payload} exceeds package estimate {estimated_bytes}"
        );
        assert!(payload <= BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES);
    }
}

/// Plan 071 task 11: `loadPackage("@clay/markdown")` yields prose movement
/// for Markdown documents (asserted in
/// `each_language_mode_registers_indent_electric_pairs_comment_triggers`)
/// and leaves unrelated document types on the built-in fallback defaults —
/// no silent behaviour change. The built-in `core.code`/`core.text` modes
/// ship movement/caret/ligature defaults with no owning package.
#[tokio::test]
async fn markdown_load_yields_prose_movement_without_touching_code_defaults() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { loadPackage } from "clay:packages";
            import { serverClassifyDocument } from "clay:modes";
            await loadPackage("@clay/markdown");
            const markdown = serverClassifyDocument({ documentId: 1, path: "README.md" });
            const code = serverClassifyDocument({ documentId: 2, path: "src/main.rs" });
            const text = serverClassifyDocument({ documentId: 3, path: "notes" });
            Deno.core.ops.op_clay_runtime_record(JSON.stringify({
                markdown: markdown.modeId, code: code.modeId, text: text.modeId
            }));
            "#,
        )
        .await
        .unwrap();

    let record = result.op_records.into_iter().next().expect("one record");
    let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON record");
    assert_eq!(parsed["markdown"], "markdown");
    // Markdown must not claim unrelated document types: with no language
    // package loaded, code-like and plain files still resolve to the
    // built-in core.code/core.text fallbacks.
    assert_eq!(parsed["code"], "core.code");
    assert_eq!(parsed["text"], "core.text");

    // Built-in fallback manifests ship the defaults without any package:
    // code movement for core.code, caret deferred to the editor default
    // bar, and role-selected typography ligatures from the baseline.
    let code = crate::protocol::BehaviorManifest::core_code_editing(1);
    assert_eq!(code.manifest_id, "default.code");
    assert_eq!(
        code.editor_rules.movement.word_separators,
        crate::protocol::WordSeparatorPolicy::Code
    );
    assert!(code.editor_rules.movement.treat_underscore_as_word);
    assert!(code.editor_rules.movement.camel_case_sub_word);
    assert_eq!(code.editor_rules.caret_style, None);
    assert_eq!(
        code.document_font_role,
        crate::protocol::DocumentFontRole::Monospace
    );

    let text = crate::protocol::BehaviorManifest::minimal_text_editing(1);
    assert_eq!(text.manifest_id, "default.text");
    assert_eq!(text.editor_rules.caret_style, None);
    assert_eq!(
        text.document_font_role,
        crate::protocol::DocumentFontRole::Proportional
    );

    // Ligature baseline ships with every font role (standard + contextual
    // on), so both fallback roles resolve ligatures at install time.
    let typography = crate::protocol::ActiveTypography::default();
    for profile in [&typography.monospace, &typography.proportional] {
        assert!(profile.ligatures.enable_standard);
        assert!(profile.ligatures.enable_contextual);
    }
}

/// Plan 071 task 11: a package may customize its mode's movement and caret
/// through documented `editorRules` manifest data (validated server-side);
/// absent fields keep the defaults.
#[tokio::test]
async fn package_manifest_can_customize_movement_and_caret_style() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let result = evaluate_as_trusted_package(
        &service,
        test_package_json(
            "@clay/fixture-prose",
            "fixtureprose",
            &["mode-registration", "mode-activation"],
            serde_json::json!({}),
        ),
        vec![
            crate::packages::permissions::PackagePermission::ModeRegistration,
            crate::packages::permissions::PackagePermission::ModeActivation,
        ],
        r#"
        import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
        serverRegisterModePattern({
            modeId: "fixtureprose",
            displayName: "Fixture Prose",
            defaultFontRole: "proportional",
            extensions: ["fxp"],
            editorRules: {
                movement: {
                    wordSeparators: "prose",
                    treatUnderscoreAsWord: false,
                    camelCaseSubWord: false,
                    stickyColumn: false
                },
                caretStyle: { shape: "block", blink: "blink", stopBlinkOnTyping: false }
            }
        });
        const classification = serverClassifyDocument({ documentId: 7, path: "notes.fxp" });
        serverActivateClassifiedMode(classification, { path: "notes.fxp" });
        "#,
    )
    .await
    .unwrap();

    let manifest = result
        .behavior_manifest
        .expect("custom mode activation publishes a manifest");
    let rules = &manifest.editor_rules;
    assert_eq!(
        rules.movement.word_separators,
        crate::protocol::WordSeparatorPolicy::Prose
    );
    assert!(!rules.movement.treat_underscore_as_word);
    assert!(!rules.movement.camel_case_sub_word);
    assert!(!rules.movement.sticky_column);
    // Absent movement fields keep the defaults.
    assert_eq!(
        rules.movement.paragraph_style,
        crate::protocol::ParagraphStyle::BlankLineOrWhitespace
    );
    let caret = rules.caret_style.expect("caret override applies");
    assert_eq!(caret.shape, crate::protocol::CaretShape::Block);
    assert!(matches!(
        caret.blink,
        crate::protocol::BlinkStyle::Blink { .. }
    ));
    assert!(!caret.stop_blink_on_typing);
    // Absent caret fields keep the defaults.
    assert_eq!(
        caret.width_px,
        crate::protocol::CaretStyle::default().width_px
    );
}
