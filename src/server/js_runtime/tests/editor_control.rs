use super::*;

/// Follow-up round (`editor-control`): package callers may use the editor
/// ops only with approved `editor-control` AND an active major mode named
/// in their `clay.editorControl.modes` declaration. Deny-by-default.
#[tokio::test]
async fn editor_control_gate_enforces_permission_and_declared_mode() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let editor_control = crate::packages::permissions::PackagePermission::EditorControl;
    let register = crate::packages::permissions::PackagePermission::ModeRegistration;
    let activate = crate::packages::permissions::PackagePermission::ModeActivation;

    // (a) Approved editor-control + declared active mode: allowed.
    let mut package_json = test_package_json(
        "@clay/fixture-editor-ok",
        "fixtureeditorok",
        &["mode-registration", "mode-activation", "editor-control"],
        serde_json::json!({}),
    );
    package_json["clay"]["editorControl"] = serde_json::json!({ "modes": ["fixtureeditorok"] });
    let evaluation = evaluate_as_trusted_package(
        &service,
        package_json,
        vec![register, activate, editor_control],
        r#"
        import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
        serverRegisterModePattern({
            modeId: "fixtureeditorok",
            displayName: "Fixture Editor OK",
            defaultFontRole: "proportional",
            extensions: ["feo"],
            editorRules: { tabSpaces: 4 }
        });
        const classification = serverClassifyDocument({ documentId: 71, path: "a.feo" });
        serverActivateClassifiedMode(classification, { path: "a.feo" });
        const moved = JSON.parse(Deno.core.ops.op_clay_editor_move_cursor(
            JSON.stringify({ direction: "nextWordStart" })));
        if (moved.commandId !== "editor.clientMoveCursor" || moved.direction !== "nextWordStart") {
            throw new Error("unexpected descriptor: " + moved.commandId + "/" + moved.direction);
        }
        "#,
    )
    .await;
    assert!(
        evaluation.is_ok(),
        "declared mode + approved editor-control must pass the gate: {evaluation:?}"
    );

    // (b) Missing `editor-control` permission: denied.
    let package_json = test_package_json(
        "@clay/fixture-editor-noperm",
        "fixtureeditornoperm",
        &["mode-registration", "mode-activation"],
        serde_json::json!({}),
    );
    let evaluation = evaluate_as_trusted_package(
        &service,
        package_json,
        vec![register, activate],
        r#"
        import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
        serverRegisterModePattern({
            modeId: "fixtureeditornoperm",
            displayName: "Fixture Editor NoPerm",
            defaultFontRole: "proportional",
            extensions: ["fen"],
            editorRules: { tabSpaces: 4 }
        });
        const classification = serverClassifyDocument({ documentId: 72, path: "a.fen" });
        serverActivateClassifiedMode(classification, { path: "a.fen" });
        let error = "";
        try {
            Deno.core.ops.op_clay_editor_move_cursor(JSON.stringify({ direction: "nextWordStart" }));
        } catch (e) {
            error = String(e && e.message ? e.message : e);
        }
        if (!error.includes("editor-control")) {
            throw new Error("expected editor-control denial, got: " + (error || "allowed"));
        }
        "#,
    )
    .await;
    assert!(
        evaluation.is_ok(),
        "missing-permission case must deny inside JS: {evaluation:?}"
    );

    // (c) Approved editor-control but the active mode is not declared:
    // denied deny-by-default.
    let mut package_json = test_package_json(
        "@clay/fixture-editor-wrongmode",
        "fixtureeditorwrongmode",
        &["mode-registration", "mode-activation", "editor-control"],
        serde_json::json!({}),
    );
    package_json["clay"]["editorControl"] = serde_json::json!({ "modes": ["some.other.mode"] });
    let evaluation = evaluate_as_trusted_package(
        &service,
        package_json,
        vec![register, activate, editor_control],
        r#"
        import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
        serverRegisterModePattern({
            modeId: "fixtureeditorwrongmode",
            displayName: "Fixture Editor WrongMode",
            defaultFontRole: "proportional",
            extensions: ["few"],
            editorRules: { tabSpaces: 4 }
        });
        const classification = serverClassifyDocument({ documentId: 73, path: "a.few" });
        serverActivateClassifiedMode(classification, { path: "a.few" });
        let error = "";
        try {
            Deno.core.ops.op_clay_editor_move_cursor(JSON.stringify({ direction: "nextWordStart" }));
        } catch (e) {
            error = String(e && e.message ? e.message : e);
        }
        if (!error.includes("mode_not_declared")) {
            throw new Error("expected mode_not_declared denial, got: " + (error || "allowed"));
        }
        "#,
    )
    .await;
    assert!(
        evaluation.is_ok(),
        "undeclared-mode case must deny inside JS: {evaluation:?}"
    );
}

/// Follow-up round (`editor-control`): the programmatic execution op
/// publishes gated known editor command IDs to the connection lane
/// with host-stamped provenance, and denies unknown IDs deny-by-default.
#[tokio::test]
async fn editor_control_execute_publishes_gated_known_commands_only() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let mut receiver = service.subscribe_editor_commands();
    let editor_control = crate::packages::permissions::PackagePermission::EditorControl;
    let register = crate::packages::permissions::PackagePermission::ModeRegistration;
    let activate = crate::packages::permissions::PackagePermission::ModeActivation;

    let mut package_json = test_package_json(
        "@clay/fixture-editor-exec",
        "fixtureeditorexec",
        &["mode-registration", "mode-activation", "editor-control"],
        serde_json::json!({}),
    );
    package_json["clay"]["editorControl"] = serde_json::json!({ "modes": ["fixtureeditorexec"] });
    let evaluation = evaluate_as_trusted_package(
        &service,
        package_json,
        vec![register, activate, editor_control],
        r#"
        import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
        serverRegisterModePattern({
            modeId: "fixtureeditorexec",
            displayName: "Fixture Editor Exec",
            defaultFontRole: "proportional",
            extensions: ["fex"],
            editorRules: { tabSpaces: 4 }
        });
        const classification = serverClassifyDocument({ documentId: 75, path: "a.fex" });
        serverActivateClassifiedMode(classification, { path: "a.fex" });
        const executed = JSON.parse(Deno.core.ops.op_clay_editor_execute_command(
            JSON.stringify({ commandId: "editor.clientMoveCursor.nextWordStart" })));
        if (!executed.requested) {
            throw new Error("expected requested=true");
        }
        let error = "";
        try {
            Deno.core.ops.op_clay_editor_execute_command(
                JSON.stringify({ commandId: "application.quit" }));
        } catch (e) {
            error = String(e && e.message ? e.message : e);
        }
        if (!error.includes("not a known editor command")) {
            throw new Error("expected unknown-ID denial, got: " + (error || "allowed"));
        }
        "#,
    )
    .await;
    assert!(
        evaluation.is_ok(),
        "execute op must publish known IDs and deny unknown ones: {evaluation:?}"
    );

    let request = tokio::time::timeout(std::time::Duration::from_millis(500), receiver.recv())
        .await
        .expect("execution request reaches the connection lane")
        .expect("editor command channel stays open");
    assert_eq!(request.command_id, "editor.clientMoveCursor.nextWordStart");
    assert_eq!(request.package_prefix, "fixtureeditorexec");
    assert_eq!(request.mode_id, "fixtureeditorexec");
}

/// Plan 071 caret-transport fix: `clientSetCursorStyle` from user
/// configuration publishes the merged runtime caret override on the
/// connection lane. Before the fix the op only validated and returned a
/// descriptor, so blink/phase settings never reached the client.
#[tokio::test]
async fn set_cursor_style_publishes_runtime_caret_override() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let mut receiver = service.subscribe_caret_styles();
    let root = config_fixture("set-cursor-style");
    fs::write(
        root.join("init.js"),
        r#"
        import { clientSetCursorStyle } from "clay:editor";
        clientSetCursorStyle({ shape: "underline", blink: "phase" });
        "#,
    )
    .unwrap();
    service
        .load_configuration_from_root(root)
        .await
        .expect("caret style configuration loads");
    let style = receiver
        .recv()
        .await
        .expect("caret override lane delivers")
        .expect("override is set, not cleared");
    assert_eq!(style.shape, crate::protocol::CaretShape::Underline);
    assert!(matches!(
        style.blink,
        crate::protocol::BlinkStyle::Phase { .. }
    ));
    // Current-value store feeds connection initial sync / lag replay.
    assert_eq!(service.caret_style_override(), Some(style));
}

/// Phase 22.1 task 10: `setPaneFocusPolicy` from user configuration
/// publishes the validated preference on the shell-preferences lane.
#[tokio::test]
async fn set_pane_focus_policy_publishes_shell_preferences() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let mut receiver = service.subscribe_shell_preferences();
    let root = config_fixture("set-pane-focus-policy");
    fs::write(
        root.join("init.js"),
        r#"
        import { setPaneFocusPolicy } from "clay:shell";
        const summary = setPaneFocusPolicy({ paneFocusPolicy: "cursor" });
        if (summary.paneFocusPolicy !== "cursor") {
            throw new Error("expected cursor summary");
        }
        "#,
    )
    .unwrap();
    service
        .load_configuration_from_root(root)
        .await
        .expect("pane focus policy configuration loads");
    let preferences = receiver
        .recv()
        .await
        .expect("shell preferences lane delivers");
    assert_eq!(preferences.pane_focus_policy, "cursor");
    // Current-value store feeds connection initial sync / lag replay.
    assert_eq!(service.shell_preferences().pane_focus_policy, "cursor");
}

/// Phase 22.1 task 10: unknown pane-focus values are rejected at the
/// configuration boundary with an actionable diagnostic.
#[tokio::test]
async fn set_pane_focus_policy_rejects_unknown_values() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let root = config_fixture("set-pane-focus-policy-invalid");
    fs::write(
        root.join("init.js"),
        r#"
        import { setPaneFocusPolicy } from "clay:shell";
        setPaneFocusPolicy({ paneFocusPolicy: "hover" });
        "#,
    )
    .unwrap();
    let result = service.load_configuration_from_root(root).await;
    assert!(
        result.is_err(),
        "unknown pane focus value must not silently evaluate, got {result:?}"
    );
}

/// Phase 22.1 task 10: with no `setPaneFocusPolicy` call the store keeps
/// the `click` default for connection initial sync.
#[tokio::test]
async fn shell_preferences_default_to_click_when_unset() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let root = config_fixture("shell-preferences-default");
    fs::write(root.join("init.js"), "// no shell configuration\n").unwrap();
    service
        .load_configuration_from_root(root)
        .await
        .expect("empty configuration loads");
    assert_eq!(service.shell_preferences().pane_focus_policy, "click");
}

/// Follow-up round (`editor-control`): third-party packages pass the same
/// gate (shared op state, mode-scoped); callers without any package
/// context are denied in the third-party domain.
#[tokio::test]
async fn third_party_editor_control_gate_requires_declared_mode() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let mut receiver = service.subscribe_editor_commands();
    let editor_control = crate::packages::permissions::PackagePermission::EditorControl;
    let register = crate::packages::permissions::PackagePermission::ModeRegistration;
    let activate = crate::packages::permissions::PackagePermission::ModeActivation;

    // A trusted package owns and activates the mode first.
    let owner = test_package_json(
        "@clay/fixture-tp-owner",
        "fixturetpowner",
        &["mode-registration", "mode-activation"],
        serde_json::json!({}),
    );
    evaluate_as_trusted_package(
        &service,
        owner,
        vec![register, activate],
        r#"
        import { serverRegisterModePattern, serverClassifyDocument, serverActivateClassifiedMode } from "clay:modes";
        serverRegisterModePattern({
            modeId: "fixturetpowner",
            displayName: "Fixture TP",
            defaultFontRole: "proportional",
            extensions: ["ftp"],
            editorRules: { tabSpaces: 4 }
        });
        const classification = serverClassifyDocument({ documentId: 74, path: "a.ftp" });
        serverActivateClassifiedMode(classification, { path: "a.ftp" });
        "#,
    )
    .await
    .expect("trusted owner activates the fixture mode");

    // Third-party package declaring the active mode: allowed.
    let mut user = test_package_json(
        "@tp/editor-user",
        "editoruser",
        &["editor-control"],
        serde_json::json!({}),
    );
    user["clay"]["editorControl"] = serde_json::json!({ "modes": ["fixturetpowner"] });
    let evaluation = evaluate_as_package(
        &service,
        user,
        vec![editor_control],
        r#"
        const moved = JSON.parse(Deno.core.ops.op_clay_editor_move_cursor(
            JSON.stringify({ direction: "prevWordStart" })));
        if (moved.commandId !== "editor.clientMoveCursor" || moved.direction !== "prevWordStart") {
            throw new Error("unexpected descriptor: " + moved.commandId + "/" + moved.direction);
        }
        const executed = JSON.parse(Deno.core.ops.op_clay_editor_execute_command(
            JSON.stringify({ commandId: "editor.clientSetSelection.selectLine" })));
        if (!executed.requested) {
            throw new Error("expected requested=true");
        }
        "#,
    )
    .await;
    assert!(
        evaluation.is_ok(),
        "third-party caller in declared mode must pass the gate: {evaluation:?}"
    );
    let request = tokio::time::timeout(std::time::Duration::from_millis(500), receiver.recv())
        .await
        .expect("third-party execution request reaches the connection lane")
        .expect("editor command channel stays open");
    assert_eq!(request.command_id, "editor.clientSetSelection.selectLine");
    assert_eq!(request.package_prefix, "editoruser");
    assert_eq!(request.mode_id, "fixturetpowner");

    // Third-party package declaring a different mode: denied.
    let mut other = test_package_json(
        "@tp/editor-other",
        "editorother",
        &["editor-control"],
        serde_json::json!({}),
    );
    other["clay"]["editorControl"] = serde_json::json!({ "modes": ["other.mode"] });
    let evaluation = evaluate_as_package(
        &service,
        other,
        vec![editor_control],
        r#"
        let error = "";
        try {
            Deno.core.ops.op_clay_editor_move_cursor(JSON.stringify({ direction: "nextWordStart" }));
        } catch (e) {
            error = String(e && e.message ? e.message : e);
        }
        if (!error.includes("mode_not_declared")) {
            throw new Error("expected mode_not_declared denial, got: " + (error || "allowed"));
        }
        "#,
    )
    .await;
    assert!(
        evaluation.is_ok(),
        "undeclared-mode third-party case must deny inside JS: {evaluation:?}"
    );

    // Third-party evaluation without any package context: denied.
    let evaluation = service
        .evaluate_third_party_module(
            r#"
            let error = "";
            try {
                Deno.core.ops.op_clay_editor_move_cursor(JSON.stringify({ direction: "nextWordStart" }));
            } catch (e) {
                error = String(e && e.message ? e.message : e);
            }
            if (!error.includes("package context")) {
                throw new Error("expected package-context denial, got: " + (error || "allowed"));
            }
            "#,
        )
        .await;
    assert!(
        evaluation.is_ok(),
        "package-less third-party call must deny inside JS: {evaluation:?}"
    );
}
