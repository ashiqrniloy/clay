use super::*;

#[tokio::test]
async fn configuration_runtime_loads_init_js_fixture() {
    let root = config_fixture("init");
    fs::write(
        root.join("init.js"),
        r#"Deno.core.ops.op_clay_runtime_record("init-loaded");"#,
    )
    .unwrap();

    let service = ClayJsRuntimeService::default();
    let result = service.load_configuration_from_root(root).await.unwrap();

    assert_eq!(result.op_records, vec!["init-loaded"]);
    assert_eq!(service.evaluation_count(), 1);
}

#[tokio::test]
async fn configuration_runtime_loads_relative_module() {
    let root = config_fixture("relative");
    fs::write(
        root.join("init.js"),
        r#"
        import { getConfigurationState, loadConfigurationModule } from "clay:configuration";
        await loadConfigurationModule({ path: "./ui.js" });
        const state = getConfigurationState();
        Deno.core.ops.op_clay_runtime_record(state.entryPoint);
        Deno.core.ops.op_clay_runtime_record(state.loadedModules.join(","));
        "#,
    )
    .unwrap();
    fs::write(
        root.join("ui.js"),
        r#"Deno.core.ops.op_clay_runtime_record("ui-loaded");"#,
    )
    .unwrap();

    let service = ClayJsRuntimeService::default();
    let result = service.load_configuration_from_root(root).await.unwrap();

    assert_eq!(result.op_records, vec!["ui-loaded", "./init.js", "./ui.js"]);
}

#[tokio::test]
async fn configuration_optional_module_failure_isolated_and_reported() {
    let root = config_fixture("optional-syntax");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadConfigurationModule } from "clay:configuration";
        const result = await loadConfigurationModule({ path: "./broken.js", optional: true });
        Deno.core.ops.op_clay_runtime_record(`${result.loaded}:${typeof result.error}`);
        Deno.core.ops.op_clay_runtime_record("after");
        "#,
    )
    .unwrap();
    fs::write(root.join("broken.js"), "export const = ;").unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("optional syntax failure must not fail configuration evaluation");

    assert_eq!(result.op_records, vec!["false:string", "after"]);
    let diagnostic = result
        .configuration_diagnostics
        .first()
        .expect("optional module failure diagnostic");
    assert_eq!(diagnostic.code, "configuration.module_failed");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert!(diagnostic.message.contains("./broken.js"));
}

#[tokio::test]
async fn configuration_optional_missing_module_failure_isolated_and_reported() {
    let root = config_fixture("optional-missing");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadConfigurationModule } from "clay:configuration";
        const result = await loadConfigurationModule({ path: "./missing.js", optional: true });
        Deno.core.ops.op_clay_runtime_record(`${result.loaded}:${typeof result.error}`);
        Deno.core.ops.op_clay_runtime_record("after");
        "#,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("optional missing module must not fail configuration evaluation");

    assert_eq!(result.op_records, vec!["false:string", "after"]);
    let diagnostic = result
        .configuration_diagnostics
        .first()
        .expect("optional missing module diagnostic");
    assert_eq!(diagnostic.code, "configuration.module_failed");
    assert!(diagnostic.message.contains("./missing.js"));
}

#[tokio::test]
async fn configuration_required_module_failure_still_fails_evaluation() {
    let root = config_fixture("required-module");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadConfigurationModule } from "clay:configuration";
        await loadConfigurationModule({ path: "./broken.js" });
        "#,
    )
    .unwrap();
    fs::write(root.join("broken.js"), "export const = ;").unwrap();

    let error = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect_err("required module failure must preserve fail-evaluation behavior");

    assert!(error.to_string().contains("SyntaxError"));
}

#[tokio::test]
async fn configuration_optional_module_path_escape_still_fails_before_catch() {
    let parent = config_fixture("optional-escape");
    let root = parent.join("config");
    fs::create_dir(&root).unwrap();
    fs::write(parent.join("outside.js"), "export const outside = true;").unwrap();
    fs::write(
        root.join("init.js"),
        r#"
        import { loadConfigurationModule } from "clay:configuration";
        await loadConfigurationModule({ path: "../outside.js", optional: true });
        "#,
    )
    .unwrap();

    let error = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect_err("optional path escape must remain a hard failure");

    assert!(error.to_string().contains("configuration.invalid_module"));
}

#[tokio::test]
async fn runtime_imports_clay_sdui_facade() {
    let service = ClayJsRuntimeService::default();
    let result = service
        .evaluate_controlled_module(
            r#"
            import { definePanel } from "clay:sdui";
            const panel = definePanel({ id: "root", title: "Runtime", children: [] });
            Deno.core.ops.op_clay_runtime_record(`${panel.kind}:${panel.title}:${panel.id}`);
            "#,
        )
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["panel:Runtime:root"]);
}

#[tokio::test]
async fn runtime_imports_clay_ui_facade_and_registers_contributions() {
    let service = ClayJsRuntimeService::default();
    let result = evaluate_as_package(
        &service,
        test_package_json(
            "@clay/markdown-ui",
            "markdown",
            &["command-registration"],
            serde_json::json!({}),
        ),
        vec![crate::packages::permissions::PackagePermission::CommandRegistration],
        r#"
            import { serverRegisterCommand } from "clay:commands";
            import {
              serverRegisterComponentContribution,
              serverRegisterPanelContribution,
              serverRegisterThemeToken,
              serverRegisterTransientOverlayContribution,
            } from "clay:ui";

            serverRegisterCommand({
              commandId: "markdown.togglePreview",
              displayName: "Toggle Markdown Preview",
              routingPolicy: "server-first",
            });
            const token = serverRegisterThemeToken({
              token: "markdown.preview.background",
              type: "color-role",
              fallback: "surface.panel",
              description: "Markdown preview background",
            });
            const component = serverRegisterComponentContribution({
              kind: "label",
              id: "markdown.preview.empty",
              text: "Preview unavailable",
            });
            const panel = serverRegisterPanelContribution({
              id: "markdown.preview",
              slot: "right",
              kind: "fixed",
              defaultVisibility: "hidden",
              actionTargets: ["markdown.togglePreview"],
              component: {
                kind: "panel",
                id: "markdown.preview.root",
                title: "Preview",
                children: [{
                  kind: "button",
                  id: "markdown.preview.toggle",
                  label: "Toggle",
                  action: { commandId: "markdown.togglePreview" },
                }],
              },
            });
            const overlay = serverRegisterTransientOverlayContribution({
              id: "markdown.preview.overlay",
              anchor: "working-area",
              focusPolicy: "restore",
              dismissalPolicy: "escape",
              component: { kind: "panel", id: "markdown.preview.overlay.root", title: "Overlay", children: [] },
            });
            Deno.core.ops.op_clay_runtime_record(`${panel.slot}:${component.rootKind}:${overlay.focusPolicy}:${token.type}:${panel.provenance.apiPrefix}`);
            "#,
    )
    .await
    .unwrap();

    assert_eq!(
        result.op_records,
        vec!["right:label:restore:color-role:markdown"]
    );
    assert_eq!(result.ui_contributions.panels.len(), 1);
    assert_eq!(result.ui_contributions.components.len(), 1);
    assert_eq!(result.ui_contributions.overlays.len(), 1);
    assert_eq!(result.ui_contributions.theme_tokens.len(), 1);
    assert_eq!(
        result.ui_contributions.panels[0].provenance.package_name,
        "@clay/markdown-ui"
    );
}

#[tokio::test]
async fn runtime_clay_ui_rejects_invalid_prefix_unregistered_action_and_raw_css() {
    let service = ClayJsRuntimeService::default();
    let error = evaluate_as_package(
        &service,
        test_package_json(
            "@clay/markdown-ui",
            "markdown",
            &["command-registration"],
            serde_json::json!({}),
        ),
        vec![crate::packages::permissions::PackagePermission::CommandRegistration],
        r#"
            import { serverRegisterPanelContribution } from "clay:ui";
            serverRegisterPanelContribution({
              id: "other.preview",
              slot: "right",
              rawCss: "color: red",
              component: { kind: "button", id: "other.preview.button", label: "Run", action: { commandId: "markdown.missing" } },
            });
            "#,
    )
    .await
    .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    assert!(error.to_string().contains("ui.registration_failed"));
}

#[tokio::test]
async fn runtime_facades_do_not_require_raw_ops() {
    let root = config_fixture("facade-no-raw-ops");
    fs::write(
        root.join("init.js"),
        r#"
        import { defineLabel } from "clay:sdui";
        import { getConfigurationState } from "clay:configuration";
        const label = defineLabel({ text: "Ready" });
        const state = getConfigurationState();
        if (label.kind !== "label" || state.entryPoint !== "./init.js") {
          throw new Error("facade import failed");
        }
        "#,
    )
    .unwrap();

    ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap();
}

#[tokio::test]
async fn unsupported_facade_returns_planned_error() {
    let service = ClayJsRuntimeService::default();
    let error = service
        .evaluate_controlled_module(
            r#"
            import { serverGetDocumentSnapshot } from "clay:documents";
            await serverGetDocumentSnapshot("1");
            "#,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    assert!(
        error
            .to_string()
            .contains("documents.serverGetDocumentSnapshot is planned")
    );
}

#[tokio::test]
async fn facade_op_mapping_matches_inventory() {
    let service = ClayJsRuntimeService::default();
    let result = service
        .evaluate_controlled_module(
            r#"
            import { loadConfigurationModule } from "clay:configuration";
            import { defineStack } from "clay:sdui";
            const stack = defineStack({ children: [] });
            Deno.core.ops.op_clay_runtime_record(`${typeof loadConfigurationModule}:${stack.kind}`);
            "#,
        )
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["function:stack"]);
}
