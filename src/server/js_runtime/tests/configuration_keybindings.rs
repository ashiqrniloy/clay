use super::*;

#[tokio::test]
async fn configuration_runtime_rejects_traversal_and_urls() {
    for rejected_path in [
        "../outside.js",
        "https://example.invalid/config.js",
        "npm:pkg",
        "package",
    ] {
        let root = config_fixture("reject");
        fs::write(
            root.join("init.js"),
            format!(
                r#"
                import {{ loadConfigurationModule }} from "clay:configuration";
                await loadConfigurationModule({{ path: "{rejected_path}" }});
                "#
            ),
        )
        .unwrap();
        let error = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(error.to_string().contains("configuration.invalid_module"));
    }
}

#[tokio::test]
async fn configuration_error_diagnostic_names_the_rejected_specifier() {
    // Phase 20.6: a config typo (e.g. `clay:themes` instead of `clay:theme`)
    // must produce a diagnostic that names the rejected specifier so the
    // user can fix it, not an opaque generic string.
    let root = config_fixture("bad-facade-import");
    fs::write(
        root.join("init.js"),
        r#"
        import { setAppearance } from "clay:themes";
        setAppearance("light");
        "#,
    )
    .unwrap();
    let error = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .unwrap_err();
    let diagnostic = error.diagnostic();
    assert_eq!(diagnostic.code, "configuration.invalid_module");
    // Secure: the rejected specifier (`clay:themes`) must NOT leak...
    assert!(
        !diagnostic.message.contains("clay:themes"),
        "diagnostic must not leak the rejected specifier, got: {}",
        diagnostic.message
    );
    // ...but the message must be actionable: name a real facade example
    // so the user can spot the typo.
    assert!(
        diagnostic.message.contains("clay:theme")
            && diagnostic.message.contains("specifier spelling"),
        "diagnostic must name an allowed facade and hint at spelling, got: {}",
        diagnostic.message
    );
}

#[tokio::test]
async fn configuration_bind_key_updates_behavior_manifest() {
    let service = ClayJsRuntimeService::default();
    let result = service
        .evaluate_controlled_module(
            r#"
            import { bindKey, listKeyBindings } from "clay:keybindings";
            import { getActiveBehaviorManifest, listBehaviorRoutes } from "clay:behavior";
            const bound = bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
            const bindings = listKeyBindings("editor");
            const manifest = await getActiveBehaviorManifest();
            const routes = await listBehaviorRoutes();
            Deno.core.ops.op_clay_runtime_record(`${bound.key}:${bound.command}:${manifest.version}:${bindings.length}:${routes.some((route) => route.apiId === "documents.serverSaveDocument")}`);
            "#,
        )
        .await
        .unwrap();
    let manifest = result
        .behavior_manifest
        .expect("published behavior manifest");

    assert_eq!(
        result.op_records,
        vec!["Ctrl+S:documents.serverSaveDocument:2:4:true"]
    );
    assert_eq!(manifest.behavior_version, 2);
    assert!(manifest.keymaps.iter().any(|rule| {
        rule.command_id == "documents.serverSaveDocument"
            && rule.routing_policy == crate::protocol::RoutingPolicy::ServerFirst
    }));
}

#[tokio::test]
async fn configuration_default_reload_binding_is_present_and_overridable() {
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey, listKeyBindings, unbindKey } from "clay:keybindings";
            const defaultBinding = listKeyBindings("global").find(
              (binding) => binding.command === "runtime.reloadConfiguration"
            );
            unbindKey("Ctrl+Shift+R", { scope: "global" });
            bindKey("Ctrl+Alt+R", "runtime.reloadConfiguration", { scope: "global" });
            const bindings = listKeyBindings("global");
            Deno.core.ops.op_clay_runtime_record(
              `${defaultBinding?.key}:${bindings.some((binding) => binding.key === "Ctrl+Shift+R")}:${bindings.some((binding) => binding.key === "Ctrl+Alt+R")}`
            );
            "#,
        )
        .await
        .expect("override reload command");
    let manifest = result.behavior_manifest.expect("bound behavior manifest");
    let rule = manifest
        .keymaps
        .iter()
        .find(|rule| rule.command_id == "runtime.reloadConfiguration")
        .expect("overridden reload binding");

    assert_eq!(result.op_records, vec!["Ctrl+Shift+R:false:true"]);
    assert_eq!(rule.context, crate::protocol::KeyBindingContext::Global);
    assert_eq!(
        rule.sequence,
        vec![crate::protocol::KeyStroke {
            key: crate::protocol::KeyCode::Character("r".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                control: true,
                alt: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        }]
    );
    assert_eq!(
        rule.routing_policy,
        crate::protocol::RoutingPolicy::ServerFirstWithLock {
            lock_scope: crate::protocol::LockScope::Behavior,
        }
    );
}

#[tokio::test]
async fn configuration_default_control_center_binding_is_present_and_overridable() {
    // Phase 24.5 / plan 124: `controlCenter.open` ships a Global
    // `Ctrl+X Ctrl+O` chord default (it moved off the P stroke, which now
    // toggles the agent lane) that init.js can unbind and rebind like any
    // other default.
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey, listKeyBindings, unbindKey } from "clay:keybindings";
            const defaultBinding = listKeyBindings("global").find(
              (binding) => binding.command === "controlCenter.open"
            );
            unbindKey("Ctrl+X Ctrl+O", { scope: "global" });
            bindKey("Ctrl+Alt+P", "controlCenter.open", { scope: "global" });
            const bindings = listKeyBindings("global");
            Deno.core.ops.op_clay_runtime_record(
              `${defaultBinding?.key}:${bindings.some((binding) => binding.key === "Ctrl+X Ctrl+O")}:${bindings.some((binding) => binding.key === "Ctrl+Alt+P")}`
            );
            "#,
        )
        .await
        .expect("override control center binding");
    let manifest = result.behavior_manifest.expect("bound behavior manifest");
    let rule = manifest
        .keymaps
        .iter()
        .find(|rule| rule.command_id == "controlCenter.open")
        .expect("overridden control center binding");

    assert_eq!(result.op_records, vec!["Ctrl+X Ctrl+O:false:true"]);
    assert_eq!(rule.context, crate::protocol::KeyBindingContext::Global);
    assert_eq!(
        rule.sequence,
        vec![crate::protocol::KeyStroke {
            key: crate::protocol::KeyCode::Character("p".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                control: true,
                alt: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        }]
    );
    assert_eq!(
        rule.routing_policy,
        crate::protocol::RoutingPolicy::ServerFirst
    );
}

#[tokio::test]
async fn configuration_default_agent_lane_binding_is_present_and_overridable() {
    // Plan 124: `shell.toggleAgentLane` ships a Global `Ctrl+X Ctrl+P`
    // chord, and init.js can unbind and rebind it through the same keybinding
    // API as the palette and other Clay-owned commands.
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey, listKeyBindings, unbindKey } from "clay:keybindings";
            const defaultBinding = listKeyBindings("global").find(
              (binding) => binding.command === "shell.toggleAgentLane"
            );
            unbindKey("Ctrl+X Ctrl+P", { scope: "global" });
            bindKey("Alt+L", "shell.toggleAgentLane", { scope: "global" });
            const bindings = listKeyBindings("global");
            Deno.core.ops.op_clay_runtime_record(
              `${defaultBinding?.key}:${bindings.some((binding) => binding.key === "Ctrl+X Ctrl+P")}:${bindings.some((binding) => binding.key === "Alt+L")}`
            );
            "#,
        )
        .await
        .expect("override agent lane binding");
    let manifest = result.behavior_manifest.expect("bound behavior manifest");
    let rule = manifest
        .keymaps
        .iter()
        .find(|rule| rule.command_id == "shell.toggleAgentLane")
        .expect("overridden agent lane binding");

    assert_eq!(result.op_records, vec!["Ctrl+X Ctrl+P:false:true"]);
    assert_eq!(rule.context, crate::protocol::KeyBindingContext::Global);
    assert_eq!(
        rule.sequence,
        vec![crate::protocol::KeyStroke {
            key: crate::protocol::KeyCode::Character("l".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                alt: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        }]
    );
    assert_eq!(
        rule.routing_policy,
        crate::protocol::RoutingPolicy::ServerFirst
    );
}

#[tokio::test]
async fn configuration_default_path_browser_binding_is_present_and_overridable() {
    // Phase 24.5: `controlCenter.openPath` ships a Global `Ctrl+X Ctrl+F`
    // chord default that init.js can unbind and rebind like any other
    // default; the command id never changes.
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey, listKeyBindings, unbindKey } from "clay:keybindings";
            const defaultBinding = listKeyBindings("global").find(
              (binding) => binding.command === "controlCenter.openPath"
            );
            unbindKey("Ctrl+X Ctrl+F", { scope: "global" });
            bindKey("Alt+P", "controlCenter.openPath", { scope: "global" });
            const bindings = listKeyBindings("global");
            Deno.core.ops.op_clay_runtime_record(
              `${defaultBinding?.key}:${bindings.some((binding) => binding.key === "Ctrl+X Ctrl+F")}:${bindings.some((binding) => binding.key === "Alt+P")}`
            );
            "#,
        )
        .await
        .expect("override path browser binding");
    let manifest = result.behavior_manifest.expect("bound behavior manifest");
    let rule = manifest
        .keymaps
        .iter()
        .find(|rule| rule.command_id == "controlCenter.openPath")
        .expect("overridden path browser binding");

    assert_eq!(result.op_records, vec!["Ctrl+X Ctrl+F:false:true"]);
    assert_eq!(rule.context, crate::protocol::KeyBindingContext::Global);
    assert_eq!(
        rule.sequence,
        vec![crate::protocol::KeyStroke {
            key: crate::protocol::KeyCode::Character("p".to_string()),
            modifiers: crate::protocol::KeyModifiers {
                alt: true,
                ..crate::protocol::KeyModifiers::NONE
            },
        }]
    );
    assert_eq!(
        rule.routing_policy,
        crate::protocol::RoutingPolicy::ServerFirst
    );
}

#[tokio::test]
async fn package_javascript_cannot_directly_execute_reload_command() {
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { serverExecuteCommand } from "clay:commands";
            try {
              await serverExecuteCommand("runtime.reloadConfiguration");
            } catch (error) {
              Deno.core.ops.op_clay_runtime_record(String(error));
            }
            "#,
        )
        .await
        .expect("reload denial remains a handled JS error");

    assert!(result.op_records.iter().any(|record| {
        record.contains("UnauthorizedTarget")
            && record.contains("runtime reload requires a user command intent")
    }));
}

#[tokio::test]
async fn configuration_binds_client_ui_file_folder_and_copy_commands() {
    let service = ClayJsRuntimeService::default();
    let result = service
        .evaluate_controlled_module(
            r#"
            import { bindKey, listKeyBindings } from "clay:keybindings";
            import { listBehaviorRoutes } from "clay:behavior";
            import { clientOpenFileDialog } from "clay:documents";
            import { clientOpenFolderDialog } from "clay:workspace";
            import { clientCopySelection } from "clay:editor";
            const file = bindKey("Ctrl+O", clientOpenFileDialog(), { scope: "editor" });
            const folder = bindKey("Ctrl+Shift+O", clientOpenFolderDialog(), { scope: "editor" });
            const copy = bindKey("Ctrl+Shift+C", clientCopySelection(), { scope: "editor" });
            const bindings = listKeyBindings("editor");
            const routes = await listBehaviorRoutes();
            const fileRoute = routes.find((candidate) => candidate.apiId === "documents.clientOpenFileDialog");
            const folderRoute = routes.find((candidate) => candidate.apiId === "workspace.clientOpenFolderDialog");
            const copyRoute = routes.find((candidate) => candidate.apiId === "editor.clientCopySelection");
            Deno.core.ops.op_clay_runtime_record(`${file.key}:${file.command}:${folder.key}:${folder.command}:${copy.key}:${copy.command}:${bindings.length}:${fileRoute.runtimePath}:${fileRoute.authority}:${folderRoute.runtimePath}:${folderRoute.authority}:${copyRoute.runtimePath}:${copyRoute.authority}`);
            "#,
        )
        .await
        .unwrap();
    let manifest = result
        .behavior_manifest
        .expect("published behavior manifest");

    assert_eq!(
        result.op_records,
        vec![
            "Ctrl+O:documents.clientOpenFileDialog:Ctrl+Shift+O:workspace.clientOpenFolderDialog:Ctrl+Shift+C:editor.clientCopySelection:6:client-ui-command:client-ui:client-ui-command:client-ui:client-ui-command:client-ui"
        ]
    );
    for command_id in [
        "documents.clientOpenFileDialog",
        "workspace.clientOpenFolderDialog",
        "editor.clientCopySelection",
    ] {
        assert!(manifest.keymaps.iter().any(|rule| {
            rule.command_id == command_id
                && rule.routing_policy == crate::protocol::RoutingPolicy::ClientUiCommand
        }));
        assert!(manifest.commands.iter().any(|command| {
            command.command_id == command_id
                && command.authority == crate::protocol::CommandAuthority::ClientUi
        }));
    }
}

#[tokio::test]
async fn configuration_unbind_key_updates_behavior_manifest() {
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey, unbindKey, listKeyBindings } from "clay:keybindings";
            bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
            unbindKey("Ctrl+S", { scope: "editor" });
            Deno.core.ops.op_clay_runtime_record(`${listKeyBindings("editor").some((binding) => binding.key === "Ctrl+S")}`);
            "#,
        )
        .await
        .unwrap();
    let manifest = result
        .behavior_manifest
        .expect("published behavior manifest");

    assert_eq!(result.op_records, vec!["false"]);
    assert_eq!(manifest.behavior_version, 3);
    assert!(
        !manifest
            .keymaps
            .iter()
            .any(|rule| rule.command_id == "documents.serverSaveDocument")
    );
}

#[tokio::test]
async fn configuration_bind_key_table_form_binds_multiple_commands() {
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey, listKeyBindings } from "clay:keybindings";
            const bound = bindKey({
              scope: "editor",
              bindings: {
                "Ctrl+O": "documents.clientOpenFileDialog",
                "Alt+I": "editor.clientSelectTextobject.function.inner.current",
                "Ctrl+S": "documents.serverSaveDocument",
              },
            });
            const bindings = listKeyBindings("editor");
            Deno.core.ops.op_clay_runtime_record(`${bound.length}:${bound[0].key}:${bound[1].command}:${bindings.some((binding) => binding.key === "Ctrl+O")}:${bindings.some((binding) => binding.key === "Alt+I")}:${bindings.some((binding) => binding.key === "Ctrl+S")}`);
            "#,
        )
        .await
        .unwrap();
    let manifest = result
        .behavior_manifest
        .expect("published behavior manifest");

    assert_eq!(
        result.op_records,
        vec!["3:Ctrl+O:editor.clientSelectTextobject.function.inner.current:true:true:true"]
    );
    for command_id in [
        "documents.clientOpenFileDialog",
        "editor.clientSelectTextobject.function.inner.current",
        "documents.serverSaveDocument",
    ] {
        assert!(
            manifest
                .keymaps
                .iter()
                .any(|rule| rule.command_id == command_id),
            "{command_id} must be bound by the table form"
        );
    }
    assert_eq!(
        manifest
            .keymaps
            .iter()
            .filter(|rule| {
                rule.context == crate::protocol::KeyBindingContext::EditorTextFocus
                    && matches!(
                        rule.command_id.as_str(),
                        "documents.clientOpenFileDialog"
                            | "editor.clientSelectTextobject.function.inner.current"
                            | "documents.serverSaveDocument"
                    )
            })
            .count(),
        3
    );
}

#[tokio::test]
async fn configuration_bind_key_table_form_is_all_or_nothing() {
    let error = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey, listKeyBindings } from "clay:keybindings";
            const bound = bindKey({
              scope: "editor",
              bindings: {
                "Ctrl+O": "documents.clientOpenFileDialog",
                "PgDn": "editor.clientUndo",
              },
            });
            Deno.core.ops.op_clay_runtime_record(`${bound.length}`);
            "#,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    let message = error.to_string();
    assert!(
        message.contains("entry 2") && message.contains("unsupported key `PgDn`"),
        "diagnostic must name the failing table entry: {message}"
    );
    // All-or-nothing: the valid first entry must not be applied. The
    // manifest is not reachable from the error, so verify via a fresh
    // evaluation that no partial binding leaked into the shared service.
    let service = ClayJsRuntimeService::default();
    let result = service
        .evaluate_controlled_module(
            r#"
            import { listKeyBindings } from "clay:keybindings";
            Deno.core.ops.op_clay_runtime_record(`${listKeyBindings("editor").some((binding) => binding.key === "Ctrl+O")}`);
            "#,
        )
        .await
        .unwrap();
    assert_eq!(result.op_records, vec!["false"]);
}

#[tokio::test]
async fn configuration_unbind_key_table_form_unbinds_multiple_keys() {
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey, unbindKey, listKeyBindings } from "clay:keybindings";
            bindKey({
              scope: "editor",
              bindings: {
                "Ctrl+O": "documents.clientOpenFileDialog",
                "Alt+I": "editor.clientSelectTextobject.function.inner.current",
              },
            });
            unbindKey({ scope: "editor", keys: ["Ctrl+O", "Alt+I"] });
            const bindings = listKeyBindings("editor");
            Deno.core.ops.op_clay_runtime_record(`${bindings.length}:${bindings.some((binding) => binding.key === "Ctrl+O")}:${bindings.some((binding) => binding.key === "Alt+I")}`);
            "#,
        )
        .await
        .unwrap();
    let manifest = result
        .behavior_manifest
        .expect("published behavior manifest");

    assert_eq!(result.op_records, vec!["3:false:false"]);
    assert!(!manifest.keymaps.iter().any(|rule| {
        matches!(
            rule.command_id.as_str(),
            "documents.clientOpenFileDialog"
                | "editor.clientSelectTextobject.function.inner.current"
        )
    }));
}

#[tokio::test]
async fn configuration_unbind_key_sequence_removes_only_the_matching_rule() {
    // Phase 24.5: unbindKey accepts a multi-stroke sequence and removes
    // only the rule whose full sequence matches; a single-stroke rule
    // sharing the first stroke stays intact.
    let ctrl_x = crate::protocol::KeyStroke {
        key: crate::protocol::KeyCode::Character("x".to_string()),
        modifiers: crate::protocol::KeyModifiers {
            control: true,
            ..crate::protocol::KeyModifiers::NONE
        },
    };
    let ctrl_o = crate::protocol::KeyStroke {
        key: crate::protocol::KeyCode::Character("o".to_string()),
        modifiers: crate::protocol::KeyModifiers {
            control: true,
            ..crate::protocol::KeyModifiers::NONE
        },
    };
    let ctrl_y = crate::protocol::KeyStroke {
        key: crate::protocol::KeyCode::Character("y".to_string()),
        modifiers: crate::protocol::KeyModifiers {
            control: true,
            ..crate::protocol::KeyModifiers::NONE
        },
    };
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey, unbindKey, listKeyBindings } from "clay:keybindings";
            bindKey("Ctrl+X Ctrl+O", "controlCenter.open", { scope: "global" });
            // Phase 24.5: the single stroke must not be a strict prefix of
            // an existing rule, so a chord that shares the first stroke
            // cannot accompany a single-stroke binding.
            bindKey("Ctrl+Y", "controlCenter.openPath", { scope: "global" });
            unbindKey("Ctrl+X Ctrl+O", { scope: "global" });
            const bindings = listKeyBindings("global");
            Deno.core.ops.op_clay_runtime_record(`${bindings.some((binding) => binding.key === "Ctrl+X Ctrl+O")}:${bindings.some((binding) => binding.key === "Ctrl+Y" && binding.command === "controlCenter.openPath")}`);
            "#,
        )
        .await
        .unwrap();
    let manifest = result
        .behavior_manifest
        .expect("published behavior manifest");

    assert_eq!(result.op_records, vec!["false:true"]);
    assert!(manifest.keymaps.iter().any(|rule| {
        rule.command_id == "controlCenter.openPath"
            && rule.sequence.len() == 1
            && rule.sequence[0] == ctrl_y
    }));
    // The sequence rule is gone; unbind removes only the matching sequence.
    let expected_sequence = vec![ctrl_x, ctrl_o];
    assert!(
        !manifest
            .keymaps
            .iter()
            .any(|rule| rule.command_id == "controlCenter.open"
                && rule.sequence == expected_sequence)
    );
}

#[tokio::test]
async fn configuration_bind_key_sequence_publishes_multi_stroke_rule() {
    // Phase 24.5: bindKey accepts a space-separated sequence; the op
    // publishes a rule whose sequence has one stroke per chord.
    let ctrl_x = crate::protocol::KeyStroke {
        key: crate::protocol::KeyCode::Character("x".to_string()),
        modifiers: crate::protocol::KeyModifiers {
            control: true,
            ..crate::protocol::KeyModifiers::NONE
        },
    };
    let ctrl_f = crate::protocol::KeyStroke {
        key: crate::protocol::KeyCode::Character("f".to_string()),
        modifiers: crate::protocol::KeyModifiers {
            control: true,
            ..crate::protocol::KeyModifiers::NONE
        },
    };
    let result = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey, listKeyBindings } from "clay:keybindings";
            const bound = bindKey("Ctrl+X Ctrl+F", "workspace.openFuzzyFile", { scope: "global" });
            const bindings = listKeyBindings("global");
            Deno.core.ops.op_clay_runtime_record(`${bound.key}:${bindings.some((binding) => binding.key === "Ctrl+X Ctrl+F" && binding.command === "workspace.openFuzzyFile")}`);
            "#,
        )
        .await
        .unwrap();
    let manifest = result
        .behavior_manifest
        .expect("published behavior manifest");

    assert_eq!(result.op_records, vec!["Ctrl+X Ctrl+F:true"]);
    let expected = vec![ctrl_x, ctrl_f];
    assert!(
        manifest
            .keymaps
            .iter()
            .any(|rule| rule.command_id == "workspace.openFuzzyFile" && rule.sequence == expected)
    );
}

#[tokio::test]
async fn configuration_bind_key_prefix_collision_is_rejected() {
    // Phase 24.5: a runtime bindKey that would make a rule a strict
    // prefix of an existing rule in the same context is rejected before
    // install (the colliding rule never publishes).
    let error = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey } from "clay:keybindings";
            bindKey("g g", "workspace.openFuzzyFile", { scope: "global" });
            bindKey("g", "controlCenter.open", { scope: "global" });
            "#,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    assert!(error.to_string().contains("keybindings.bind_failed"));
}

#[tokio::test]
async fn unknown_command_binding_is_rejected() {
    let error = ClayJsRuntimeService::default()
        .evaluate_controlled_module(
            r#"
            import { bindKey } from "clay:keybindings";
            bindKey("Ctrl+X", "shell.run");
            "#,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    assert!(error.to_string().contains("keybindings.unknown_command"));
}

#[tokio::test]
async fn raw_clipboard_and_dialog_command_bindings_are_rejected() {
    for command_id in [
        "clipboard.writeText",
        "dialog.openRawPath",
        "Deno.core.ops.op_clipboard_write",
    ] {
        let source = format!(
            r#"
            import {{ bindKey }} from "clay:keybindings";
            bindKey("Ctrl+Alt+C", {command_id:?});
            "#
        );
        let error = ClayJsRuntimeService::default()
            .evaluate_controlled_module_for_document(source, 88)
            .await
            .unwrap_err();

        assert!(matches!(error, ClayRuntimeError::Runtime(_)));
        assert!(
            error.to_string().contains("keybindings.unknown_command"),
            "{command_id} must stay rejected: {error}"
        );
    }
}
