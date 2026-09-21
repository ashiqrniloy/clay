use super::*;

#[tokio::test]
async fn smoke_config_fixture_publishes_runtime_sdui_snapshot() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("configuration")
        .join("runtime-sdui");

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();
    let tree = result.published_sdui_tree.expect("published SDUI tree");

    assert!(tree.nodes.iter().any(|node| matches!(
        &node.kind,
        crate::protocol::SduiNodeKind::Panel { title, .. } if title == "Runtime Smoke Workspace"
    )));
    assert!(tree.nodes.iter().any(|node| matches!(
        &node.kind,
        crate::protocol::SduiNodeKind::EditorView { binding }
            if binding.document_id == 1 && binding.expected_version == Some(1)
    )));
}

#[tokio::test]
async fn markdown_config_fixture_opens_workspace_without_default_panel() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("configuration")
        .join("markdown-mode");
    let workspace_root = root.join("workspace");
    let mut workspace = WorkspaceState::new();
    workspace
        .add_root(&workspace_root)
        .expect("markdown workspace fixture root must register");

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    // Phase 20 task 4: the fixture uses the default load path and publishes
    // NO default side panel — only behavior/decorations state. The optional
    // preview is a package PanelContribution, validated separately by
    // `markdown_optional_preview_is_valid_panel_contribution`.
    assert!(
        result.published_sdui_tree.is_none(),
        "markdown-mode fixture must not publish a default side panel SDUI tree"
    );
    assert_eq!(result.parse_handlers.len(), 1);
    assert_eq!(result.parse_handlers[0].package_prefix, "markdown");
    // Decorations publish only through package callbacks now; the fixture
    // (configuration code) no longer publishes directly.
    assert!(result.published_decoration_set.is_none());
    let manifest = result
        .behavior_manifest
        .expect("markdown behavior manifest");
    assert!(
        manifest
            .commands
            .iter()
            .any(|command| command.command_id == "markdown.togglePreview")
    );
}

#[tokio::test]
async fn windows_markdown_open_config_fixture_loads_without_default_panel() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("configuration")
        .join("windows-markdown-open");
    let workspace_root = root.join("workspace");
    let mut workspace = WorkspaceState::new();
    workspace
        .add_root(&workspace_root)
        .expect("Windows Markdown open fixture root must register");

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root_with_workspace(root, Arc::new(Mutex::new(workspace)))
        .await
        .unwrap();

    // Phase 20 task 4: the fixture uses the default load path and publishes
    // NO default side panel — only behavior/decorations state.
    assert!(
        result.published_sdui_tree.is_none(),
        "windows-markdown-open fixture must not publish a default side panel SDUI tree"
    );
    assert_eq!(result.parse_handlers.len(), 1);
    assert_eq!(result.parse_handlers[0].package_prefix, "markdown");
    // Decorations publish only through package callbacks now; the fixture
    // (configuration code) no longer publishes directly.
    assert!(result.published_decoration_set.is_none());
    let manifest = result
        .behavior_manifest
        .expect("Windows Markdown open behavior manifest");
    assert!(manifest.keymaps.iter().any(|rule| {
        rule.sequence
            == vec![crate::protocol::KeyStroke {
                key: crate::protocol::KeyCode::Character("o".to_string()),
                modifiers: crate::protocol::KeyModifiers {
                    control: true,
                    ..crate::protocol::KeyModifiers::NONE
                },
            }]
            && rule.command_id == "documents.clientOpenFileDialog"
            && rule.routing_policy == crate::protocol::RoutingPolicy::ClientUiCommand
    }));
    assert!(manifest.commands.iter().any(|command| {
        command.command_id == "documents.clientOpenFileDialog"
            && command.authority == crate::protocol::CommandAuthority::ClientUi
    }));
    assert!(
        manifest
            .commands
            .iter()
            .any(|command| command.command_id == "markdown.togglePreview")
    );
}

#[tokio::test]
async fn markdown_package_runtime_loads_markdown_it_workflow() {
    let root = config_fixture("markdown-package-runtime");
    for file_name in ["index.js", "load.js", "parser.js", "sdui.js"] {
        fs::write(
            root.join(file_name),
            fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                .expect("first-party Markdown runtime module must exist"),
        )
        .unwrap();
    }
    fs::write(
        root.join("init.js"),
        r##"
        import * as commands from "clay:commands";
        import * as decorations from "clay:decorations";
        import * as modes from "clay:modes";
        import * as packages from "clay:packages";
        import * as parse from "clay:parse";
        import * as sdui from "clay:sdui";
        import { loadPackage } from "clay:packages";
        import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
        import { publishMarkdownDecorations } from "./parser.js";
        import { publishMarkdownPreviewStatus } from "./sdui.js";

        const clay = { commands, decorations, modes, packages, parse, sdui };
        // Load through the real loadPackage path (host-stamped provenance
        // for this evaluation), then drive the parser/sdui workflow.
        await loadPackage("@clay/markdown");
        const classification = serverClassifyDocument({ documentId: 1, path: "sample.md" });
        serverActivateClassifiedMode(classification, { path: "sample.md" });
        const text = "# Runtime package\n\n- item\n";
        const tokens = [
          { type: "heading_open", tag: "h1", map: [0, 1] },
          { type: "inline", map: [0, 1], content: "Runtime package", children: [{ type: "text", content: "Runtime package" }] },
          { type: "heading_close" },
          { type: "bullet_list_open", map: [2, 3] },
          { type: "list_item_open", map: [2, 3] },
          { type: "inline", map: [2, 3], content: "item", children: [{ type: "text", content: "item" }] },
          { type: "list_item_close" },
          { type: "bullet_list_close" },
        ];
        const update = await publishMarkdownDecorations(clay, {
          text,
          tokens,
          documentId: 1,
          documentVersion: 1,
          behaviorVersion: 2,
          viewport: { byteStart: 0, byteEnd: 64 },
        });
        await publishMarkdownPreviewStatus(clay, {
          documentId: 1,
          documentVersion: 1,
          documentPath: "sample.md",
        });
        Deno.core.ops.op_clay_runtime_record(`markdown:${update.publishedSpanCount}`);
        "##,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["markdown:2"]);
    assert_eq!(result.parse_handlers.len(), 1);
    assert_eq!(result.parse_handlers[0].package_prefix, "markdown");
    assert!(result.published_decoration_set.is_some());
    let tree = result.published_sdui_tree.expect("Markdown SDUI tree");
    assert!(tree.nodes.iter().any(|node| matches!(
        &node.kind,
        crate::protocol::SduiNodeKind::Label { text, icon: _ } if text == "Parse: markdown-it registered"
    )));
    let manifest = result
        .behavior_manifest
        .expect("Markdown behavior manifest");
    assert!(
        manifest
            .commands
            .iter()
            .any(|command| command.command_id == "markdown.togglePreview")
    );
}

#[tokio::test]
async fn language_packages_config_fixture_loads_and_registers_all_contributions() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("configuration")
        .join("language-packages");

    let service = ClayJsRuntimeService::default();
    let result = service.load_configuration_from_root(root).await.unwrap();
    assert_eq!(
        service.completion_providers(),
        result.completion_providers,
        "runtime service must retain an inert Rust snapshot for completion requests"
    );

    for (provider_id, expected_item) in [
        ("rust.keywords", "fn"),
        ("typescript.keywords", "interface"),
        ("javascript.keywords", "function"),
        ("markdown.keywords", "# "),
    ] {
        let provider = result
            .completion_providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .unwrap_or_else(|| panic!("fixture must register {provider_id}"));
        assert_eq!(provider.priority, 0);
        assert!(
            provider
                .items
                .iter()
                .any(|item| item.label == expected_item),
            "{provider_id} must carry `{expected_item}` as inert text replacement data"
        );
        assert!(provider.items.len() <= provider.max_items);
    }

    let component_ids: Vec<_> = result
        .ui_contributions
        .components
        .iter()
        .map(|component| component.id.clone())
        .collect();
    assert!(
        component_ids.iter().any(|id| id == "rust.status.mode"),
        "fixture must register rust.status.mode status item"
    );
    assert!(
        component_ids
            .iter()
            .any(|id| id == "typescript.status.mode"),
        "fixture must register typescript.status.mode status item"
    );
    assert!(
        component_ids
            .iter()
            .any(|id| id == "javascript.status.mode"),
        "fixture must register javascript.status.mode status item"
    );
    assert!(
        component_ids.iter().any(|id| id == "markdown.status.mode"),
        "fixture must register markdown.status.mode status item"
    );

    let grammar_ids: Vec<_> = result
        .syntax_grammars
        .iter()
        .map(|grammar| grammar.language_id.clone())
        .collect();
    assert!(
        grammar_ids.iter().any(|id| id == "rust"),
        "fixture must register rust syntax grammar"
    );
    assert!(
        grammar_ids.iter().any(|id| id == "typescript"),
        "fixture must register typescript syntax grammar"
    );
    assert!(
        grammar_ids.iter().any(|id| id == "javascript"),
        "fixture must register javascript syntax grammar"
    );

    // Phase 18.18: the Markdown package also loads through a one-line
    // `loadPackage("@clay/markdown")` and registers its JS parse handler
    // (decoration/preview path) alongside the three code-language packages.
    assert!(
        grammar_ids.iter().any(|id| id == "markdown"),
        "fixture must register markdown syntax grammar"
    );
    assert_eq!(
        result.js_parse_handlers.len(),
        1,
        "fixture must register the Markdown parse handler"
    );
    assert_eq!(
        result.js_parse_handlers[0].package.manifest.name, "@clay/markdown",
        "Markdown parse handler must come from @clay/markdown"
    );
}

#[tokio::test]
async fn first_party_language_packages_are_not_silent_defaults() {
    // No `loadPackage` call in init.js: no first-party package may register
    // its mode, commands, completion providers, parse handlers, or UI. The
    // compiled-in Tier 1 native grammars remain (engine capability, not
    // package activation); only an explicit `loadPackage("@clay/*")` opts a
    // package's contributions in.
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = config_fixture("no-silent-defaults");
    fs::write(
        root.join("init.js"),
        "// empty init.js: no language packages loaded\n",
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();

    assert!(
        result.completion_providers.is_empty(),
        "no completion provider may register without an explicit loadPackage"
    );
    assert!(
        result.js_parse_handlers.is_empty(),
        "no parse handler may register without an explicit loadPackage"
    );
    assert!(
        result.ui_contributions.components.is_empty(),
        "no package UI contribution may register without an explicit loadPackage"
    );
    assert!(
        result.ui_contributions.panels.is_empty(),
        "no package panel may register without an explicit loadPackage"
    );
    // The five compiled-in first-party native grammars are engine
    // capability (registered by `with_first_party_native`), not silent
    // package defaults: they only highlight when an explicit `loadPackage`
    // has registered a matching major mode that selects them.
    assert_eq!(
        result.syntax_grammars.len(),
        5,
        "only the compiled-in native grammars exist with no package loaded"
    );
    for grammar in &result.syntax_grammars {
        assert_eq!(
            grammar.engine_tier,
            crate::server::syntax::SyntaxEngineTier::Native,
            "unloaded-package grammars must be native engine capability only"
        );
    }
}

#[tokio::test]
async fn file_browser_workflow_config_fixture_loads_packages_and_bindings() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("configuration")
        .join("file-browser-workflow");

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();
    let manifest = result
        .behavior_manifest
        .expect("fixture must publish configured keybindings");

    for provider_id in [
        "rust.keywords",
        "typescript.keywords",
        "javascript.keywords",
    ] {
        assert!(
            result
                .completion_providers
                .iter()
                .any(|provider| provider.id == provider_id),
            "fixture must load completion provider {provider_id}"
        );
    }
    for command_id in [
        "workspace.clientOpenFolderDialog",
        "workspace.openFuzzyFile",
        "workspace.toggleFileBrowser",
        "documents.serverSaveDocument",
        "editor.clientCopySelection",
        "editor.clientCutSelection",
        "editor.clientPasteClipboard",
        "editor.clientShowOpenDocuments",
    ] {
        assert!(
            manifest
                .keymaps
                .iter()
                .any(|rule| rule.command_id == command_id),
            "fixture must bind {command_id}"
        );
    }
    for command_id in [
        "workspace.clientOpenFolderDialog",
        "editor.clientCopySelection",
        "editor.clientCutSelection",
        "editor.clientPasteClipboard",
        "editor.clientShowOpenDocuments",
    ] {
        assert!(manifest.commands.iter().any(|command| {
            command.command_id == command_id
                && command.authority == crate::protocol::CommandAuthority::ClientUi
        }));
    }
}

#[tokio::test]
async fn configuration_can_publish_sdui_snapshot() {
    let root = config_fixture("sdui-publish");
    fs::write(
        root.join("init.js"),
        r#"
        import {
          defineButton,
          defineEditorView,
          defineFlex,
          defineLabel,
          defineList,
          definePanel,
          defineStack,
          publishTree,
        } from "clay:sdui";

        const tree = defineFlex({
          id: "root",
          direction: "row",
          children: [
            definePanel({
              id: "panel",
              title: "Runtime Workspace",
              children: [defineStack({
                id: "stack",
                children: [
                  defineLabel({ id: "label", text: "Ready" }),
                  defineButton({
                    id: "refresh",
                    label: "Refresh",
                    action: { commandId: "workspace.refresh", arguments: { force: true } },
                  }),
                  defineList({
                    id: "documents",
                    items: [{
                      id: "active",
                      label: "Document 1",
                      detail: "Runtime generated",
                      action: { commandId: "document.open_recent" },
                    }],
                  }),
                ],
              })],
            }),
            defineEditorView({ id: "editor", documentId: 1, expectedVersion: 1 }),
          ],
        });
        await publishTree(tree);
        "#,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();
    let tree = result.published_sdui_tree.expect("published SDUI tree");

    assert_eq!(tree.ui_version, 1);
    assert_eq!(tree.nodes.len(), 7);
    assert!(tree.nodes.iter().any(|node| matches!(
        &node.kind,
        crate::protocol::SduiNodeKind::Panel { title, .. } if title == "Runtime Workspace"
    )));
    assert!(tree.nodes.iter().any(|node| matches!(
        &node.kind,
        crate::protocol::SduiNodeKind::EditorView { binding }
            if binding.document_id == 1 && binding.expected_version == Some(1)
    )));
}

#[tokio::test]
async fn js_generated_sdui_rejects_unknown_document_binding() {
    let service = ClayJsRuntimeService::default();
    let error = service
        .evaluate_controlled_module(
            r#"
            import { defineEditorView, publishTree } from "clay:sdui";
            await publishTree(defineEditorView({ documentId: 999 }));
            "#,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    assert!(error.to_string().contains("sdui.invalid_tree"));
}

#[tokio::test]
async fn js_generated_sdui_rejects_executable_action_payloads() {
    let service = ClayJsRuntimeService::default();
    let error = service
        .evaluate_controlled_module(
            r#"
            import { defineButton, publishTree } from "clay:sdui";
            await publishTree(defineButton({
              label: "Run",
              action: { commandId: "shell.run", arguments: { code: "rm -rf /" } },
            }));
            "#,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    assert!(error.to_string().contains("sdui.invalid_action"));
}
