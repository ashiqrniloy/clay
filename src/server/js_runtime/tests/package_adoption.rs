use super::*;

/// Plan 061 task 15: one-line `loadPackage` from a trusted configuration
/// must fail with a clear pending-adoption diagnostic when no durable
/// approval exists — init.js cannot bypass the pre-execution gate.
#[tokio::test]
async fn third_party_config_load_fails_with_pending_adoption_diagnostic() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let root = config_fixture("third-party-config-adoption")
        .join(format!("pending-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterCompletionProvider } from "clay:completion";
        serverRegisterCompletionProvider({
          module: {
            provideCompletion: async () => ({
              status: "ok",
              items: [{ label: "config-loaded", insertText: "config-loaded" }]
            })
          }
        });
        export default function load() {}
        "#,
    );
    let mut package_json = loadable_package_fixture("@vendor/config-adopt", "cfgadopt");
    package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
    package_json["clay"]["contributions"] = serde_json::json!({
        "completionProviders": [{
            "id": "cfgadopt.provider",
            "triggerCharacters": ["."],
            "budgets": { "timeoutMs": 500, "maxItems": 8 }
        }]
    });
    {
        let op_state = service.test_op_state();
        let mut locked = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        locked
            .install_from_value_at_root_with_spec(
                package_json,
                root.clone(),
                "local:config-adopt-test",
            )
            .expect("synthetic package installs");
        // Intentionally skip approve_package — leaving it in Pending state.
        locked
            .authorize_package(
                "@vendor/config-adopt",
                vec![crate::packages::permissions::PackagePermission::CompletionProvider],
                crate::packages::authorization::RuntimeProfile::Restricted,
                "test",
            )
            .expect("synthetic package authorizes");
    }
    let config_root = config_fixture("third-party-config-adoption").join("config");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("@vendor/config-adopt");
        "#,
    )
    .unwrap();
    let error = service
        .load_configuration_from_root(config_root)
        .await
        .expect_err("unadopted third-party package must not execute via config");
    let message = error.to_string();
    assert!(
        message.contains("adoption") || message.contains("missing"),
        "expected adoption diagnostic, got: {message}"
    );
    assert!(
        !service
            .test_op_state()
            .package_service()
            .lock()
            .unwrap()
            .inspect("@vendor/config-adopt")
            .unwrap()
            .is_enabled,
        "pending package must stay disabled after failed config load"
    );
    let _ = fs::remove_dir_all(
        config_fixture("third-party-config-adoption")
            .join(format!("pending-{}", std::process::id())),
    );
}

/// Plan 115 task 4: the Clay-appended init.js block is still just
/// `loadPackage`; an installed-but-unadopted third-party package fails
/// closed and executes no package JS.
#[tokio::test]
async fn clay_appended_load_line_fails_closed_without_adoption() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let root = config_fixture("third-party-config-adoption")
        .join(format!("init-line-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterCompletionProvider } from "clay:completion";
        Deno.core.ops.op_clay_runtime_record("package-js-ran");
        serverRegisterCompletionProvider({
          module: {
            provideCompletion: async () => ({
              status: "ok",
              items: [{ label: "init-line", insertText: "init-line" }]
            })
          }
        });
        export default function load() {}
        "#,
    );
    let mut package_json = loadable_package_fixture("@vendor/init-line", "initline");
    package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
    package_json["clay"]["contributions"] = serde_json::json!({
        "completionProviders": [{
            "id": "initline.provider",
            "triggerCharacters": ["."],
            "budgets": { "timeoutMs": 500, "maxItems": 8 }
        }]
    });
    {
        let op_state = service.test_op_state();
        let mut locked = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        locked
            .install_from_value_at_root_with_spec(
                package_json,
                root.clone(),
                "npm:@vendor/init-line",
            )
            .expect("synthetic package installs");
        locked
            .authorize_package(
                "@vendor/init-line",
                vec![crate::packages::permissions::PackagePermission::CompletionProvider],
                crate::packages::authorization::RuntimeProfile::Restricted,
                "test",
            )
            .expect("synthetic package authorizes");
    }
    let config_root = config_fixture("third-party-config-adoption").join("config-init-line");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("init.js"),
        "import { loadPackage } from \"clay:packages\";\n",
    )
    .unwrap();
    crate::packages::init_lines::append_load_line(
        &config_root.join("init.js"),
        "@vendor/init-line",
    )
    .expect("clay load line appends");
    let error = service
        .load_configuration_from_root(config_root)
        .await
        .expect_err("unadopted clay-appended load line must not execute");
    let message = error.to_string();
    assert!(
        message.contains("adoption") || message.contains("missing"),
        "expected adoption diagnostic, got: {message}"
    );
    assert!(
        !message.contains("package-js-ran"),
        "package JS must not run: {message}"
    );
    assert!(
        !service
            .test_op_state()
            .package_service()
            .lock()
            .unwrap()
            .inspect("@vendor/init-line")
            .unwrap()
            .is_enabled,
        "pending package must stay disabled"
    );
    let _ = fs::remove_dir_all(&root);
}

/// Plan 061 task 15: after CLI adoption, a one-line `loadPackage` from
/// `init.js` succeeds — the package executes in the third-party runtime
/// and its registration payload is absorbed into the trusted worker.
#[tokio::test]
async fn third_party_config_load_succeeds_after_cli_adoption() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let root = config_fixture("third-party-config-adoption")
        .join(format!("adopted-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterCompletionProvider } from "clay:completion";
        serverRegisterCompletionProvider({
          module: {
            provideCompletion: async () => ({
              status: "ok",
              items: [{ label: "config-loaded", insertText: "config-loaded" }]
            })
          }
        });
        export default function load() {}
        "#,
    );
    let mut package_json = loadable_package_fixture("@vendor/config-adopt-ok", "cfgadok");
    package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
    package_json["clay"]["contributions"] = serde_json::json!({
        "completionProviders": [{
            "id": "cfgadok.provider",
            "triggerCharacters": ["."],
            "budgets": { "timeoutMs": 500, "maxItems": 8 }
        }]
    });
    {
        let op_state = service.test_op_state();
        let mut locked = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        locked
            .install_from_value_at_root_with_spec(
                package_json,
                root.clone(),
                "local:config-adopt-ok",
            )
            .expect("synthetic package installs");
        locked
            .authorize_package(
                "@vendor/config-adopt-ok",
                vec![crate::packages::permissions::PackagePermission::CompletionProvider],
                crate::packages::authorization::RuntimeProfile::Restricted,
                "test",
            )
            .expect("synthetic package authorizes");
        // Simulate the CLI adoption step that must precede config load.
        locked
            .approve_package("@vendor/config-adopt-ok", "cli")
            .expect("CLI adoption succeeds");
    }
    let config_root = config_fixture("third-party-config-adoption").join("config-ok");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("@vendor/config-adopt-ok");
        Deno.core.ops.op_clay_runtime_record("package-loaded");
        "#,
    )
    .unwrap();
    let result = service
        .load_configuration_from_root(config_root)
        .await
        .expect("adopted third-party package must load via config");
    assert!(
        result.op_records.contains(&"package-loaded".to_string()),
        "config evaluation must complete after third-party load"
    );
    assert!(
        !result.js_completion_providers.is_empty(),
        "absorbed cross-domain registration must include completion provider"
    );
    let _ = fs::remove_dir_all(
        config_fixture("third-party-config-adoption")
            .join(format!("adopted-{}", std::process::id())),
    );
}

/// Plan 061 task 15: a stale approval (version drift, scope expansion, or
/// target replacement) blocks config-load of an approved package — the
/// adoption gate fails closed and a diagnostic is produced.
#[tokio::test]
async fn stale_approval_blocks_config_load_with_clear_diagnostic() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let root =
        config_fixture("third-party-config-stale").join(format!("stale-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterCompletionProvider } from "clay:completion";
        serverRegisterCompletionProvider({
          module: {
            provideCompletion: async () => ({
              status: "ok",
              items: [{ label: "stale", insertText: "stale" }]
            })
          }
        });
        export default function load() {}
        "#,
    );
    let package_json_v1 = loadable_package_fixture("@vendor/config-stale", "cfgstale");
    {
        let op_state = service.test_op_state();
        let mut locked = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        locked
            .install_from_value_at_root_with_spec(
                package_json_v1,
                root.clone(),
                "local:config-stale",
            )
            .expect("synthetic package v1 installs");
        locked
            .authorize_package(
                "@vendor/config-stale",
                Vec::new(),
                crate::packages::authorization::RuntimeProfile::Restricted,
                "test",
            )
            .expect("synthetic package authorizes");
        locked
            .approve_package("@vendor/config-stale", "cli")
            .expect("initial adoption succeeds");
    }
    // Update the install to a different version, staling the approval.
    let mut package_json_v2 = loadable_package_fixture("@vendor/config-stale", "cfgstale");
    package_json_v2["version"] = serde_json::json!("0.2.0");
    {
        let op_state = service.test_op_state();
        let mut locked = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        locked
            .install_from_value_at_root_with_spec(
                package_json_v2,
                root.clone(),
                "local:config-stale-v2",
            )
            .expect("synthetic package v2 installs");
    }
    let config_root = config_fixture("third-party-config-stale").join("config");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("@vendor/config-stale");
        "#,
    )
    .unwrap();
    let error = service
        .load_configuration_from_root(config_root)
        .await
        .expect_err("stale approval must block config load (version drift beyond adopted)");
    let message = error.to_string();
    assert!(
        message.contains("adoption") || message.contains("stale") || message.contains("missing"),
        "expected stale-adoption diagnostic, got: {message}"
    );
    let _ = fs::remove_dir_all(
        config_fixture("third-party-config-stale").join(format!("stale-{}", std::process::id())),
    );
}

/// Plan 136 task 3: `authorize` is the documented capability-grant surface.
/// A trusted configuration grants the manifest-declared capability and the
/// package then loads; before the grant the same package fails closed, and
/// repeating the call is idempotent.
#[tokio::test]
async fn config_authorize_grants_declared_capability_and_enables_package() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let root = config_fixture("package-authorize").join(format!("grant-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterCompletionProvider } from "clay:completion";
        serverRegisterCompletionProvider({
          module: {
            provideCompletion: async () => ({
              status: "ok",
              items: [{ label: "granted", insertText: "granted" }]
            })
          }
        });
        export default function load() {}
        "#,
    );
    let mut package_json = loadable_package_fixture("@vendor/authorize-grant", "authgrant");
    package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
    package_json["clay"]["contributions"] = serde_json::json!({
        "completionProviders": [{
            "id": "authgrant.provider",
            "triggerCharacters": ["."],
            "budgets": { "timeoutMs": 500, "maxItems": 8 }
        }]
    });
    {
        let op_state = service.test_op_state();
        let mut locked = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        locked
            .install_from_value_at_root_with_spec(
                package_json,
                root.clone(),
                "local:authorize-grant",
            )
            .expect("synthetic package installs");
        locked
            .approve_package("@vendor/authorize-grant", "cli")
            .expect("CLI adoption succeeds");
        // Adoption alone must not grant the capability: only `authorize` may.
        let error = locked
            .enable("@vendor/authorize-grant")
            .expect_err("adopted package without a capability grant must not enable");
        assert!(
            matches!(
                error,
                crate::packages::service::PackageServiceError::MissingCapabilityGrant { .. }
            ),
            "expected MissingCapabilityGrant before the grant, got {error}"
        );
    }
    let config_root = config_fixture("package-authorize").join("config-grant");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { authorize, loadPackage } from "clay:packages";
        const options = {
          package: "@vendor/authorize-grant",
          capabilities: ["completion-provider"],
          runtimeProfile: "native-trust",
          approvedBy: "config",
        };
        const grant = authorize(options);
        const repeat = authorize(options);
        Deno.core.ops.op_clay_runtime_record(
          `granted:${grant.granted}:${grant.capabilities.join(",")}:${repeat.granted}`
        );
        await loadPackage("@vendor/authorize-grant");
        Deno.core.ops.op_clay_runtime_record("package-loaded");
        "#,
    )
    .unwrap();
    let result = service
        .load_configuration_from_root(config_root)
        .await
        .expect("granted + adopted package must load via config");
    assert!(
        result
            .op_records
            .contains(&"granted:true:completion-provider:true".to_string()),
        "authorize must report the grant and stay idempotent: {:?}",
        result.op_records
    );
    assert!(
        result.op_records.contains(&"package-loaded".to_string()),
        "config evaluation must complete after the granted load: {:?}",
        result.op_records
    );
    assert!(
        !result.js_completion_providers.is_empty(),
        "granted package must register its completion provider"
    );
    let _ = fs::remove_dir_all(
        config_fixture("package-authorize").join(format!("grant-{}", std::process::id())),
    );
}

/// Plan 136 task 3: a grant for a capability the manifest never declares is
/// rejected instead of silently ignored.
#[tokio::test]
async fn config_authorize_rejects_undeclared_capability() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let root = config_fixture("package-authorize").join(format!("declared-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_loadable_package(&root, "export default function load() {}\n");
    let mut package_json = loadable_package_fixture("@vendor/authorize-declared", "authdeclared");
    package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
    {
        let op_state = service.test_op_state();
        let mut locked = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        locked
            .install_from_value_at_root_with_spec(
                package_json,
                root.clone(),
                "local:authorize-declared",
            )
            .expect("synthetic package installs");
    }
    let config_root = config_fixture("package-authorize").join("config-declared");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { authorize } from "clay:packages";
        authorize({
          package: "@vendor/authorize-declared",
          capabilities: ["filesystem"],
          approvedBy: "config",
        });
        "#,
    )
    .unwrap();
    let error = service
        .load_configuration_from_root(config_root)
        .await
        .expect_err("undeclared capability must be rejected");
    let message = error.to_string();
    assert!(
        message.contains("packages.undeclared_capability"),
        "expected undeclared-capability diagnostic, got: {message}"
    );
    let _ = fs::remove_dir_all(
        config_fixture("package-authorize").join(format!("declared-{}", std::process::id())),
    );
}

/// Plan 136 task 3: `source` must match the installed provenance, and an
/// unknown package is rejected before any grant is recorded.
#[tokio::test]
async fn config_authorize_rejects_provenance_mismatch_and_unknown_package() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let root = config_fixture("package-authorize").join(format!("source-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write_loadable_package(&root, "export default function load() {}\n");
    let mut package_json = loadable_package_fixture("@vendor/authorize-source", "authsource");
    package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
    {
        let op_state = service.test_op_state();
        let mut locked = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        locked
            .install_from_value_at_root_with_spec(
                package_json,
                root.clone(),
                "local:authorize-source",
            )
            .expect("synthetic package installs");
    }
    let config_root = config_fixture("package-authorize").join("config-source");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"
        import { authorize } from "clay:packages";
        authorize({
          package: "@vendor/authorize-source",
          capabilities: ["completion-provider"],
          source: "github:someone/else",
          approvedBy: "config",
        });
        "#,
    )
    .unwrap();
    let error = service
        .load_configuration_from_root(config_root)
        .await
        .expect_err("mismatched source must be rejected");
    let message = error.to_string();
    assert!(
        message.contains("packages.provenance_mismatch"),
        "expected provenance-mismatch diagnostic, got: {message}"
    );

    let unknown_root = config_fixture("package-authorize").join("config-unknown");
    fs::create_dir_all(&unknown_root).unwrap();
    fs::write(
        unknown_root.join("init.js"),
        r#"
        import { authorize } from "clay:packages";
        authorize({
          package: "@vendor/authorize-unknown",
          capabilities: ["completion-provider"],
          approvedBy: "config",
        });
        "#,
    )
    .unwrap();
    let error = service
        .load_configuration_from_root(unknown_root)
        .await
        .expect_err("unknown package must be rejected");
    let message = error.to_string();
    assert!(
        message.contains("packages.not_installed"),
        "expected not-installed diagnostic, got: {message}"
    );
    let _ = fs::remove_dir_all(
        config_fixture("package-authorize").join(format!("source-{}", std::process::id())),
    );
}

/// Plan 136 task 10: `authorize` is user configuration, CLI, or explicit-user
/// work — never package work. A package's own code cannot grant itself a
/// capability even in the trusted domain, where `clay:packages` is importable:
/// the op refuses while a package activation is open, so a load entry cannot
/// widen its own record. (In the shared third-party runtime the facade is
/// absent entirely, so package code cannot reach the op at all.)
#[tokio::test]
async fn package_code_cannot_self_grant_capabilities_during_activation() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let mut package_json = loadable_package_fixture("@vendor/selfgrant", "selfgrant");
    package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
    let evaluation = evaluate_as_trusted_package(
        &service,
        package_json,
        vec![crate::packages::permissions::PackagePermission::CompletionProvider],
        r#"
        import { authorize } from "clay:packages";
        try {
          const summary = authorize({
            package: "@vendor/selfgrant",
            capabilities: ["completion-provider"],
            approvedBy: "user",
          });
          Deno.core.ops.op_clay_runtime_record(`self-granted:${summary.granted}`);
        } catch (error) {
          Deno.core.ops.op_clay_runtime_record(`self-grant-refused:${error.message}`);
        }
        "#,
    )
    .await
    .expect("trusted package evaluation must run");
    let records = evaluation.op_records.join("\n");
    assert!(
        records.contains("packages.grant_during_activation"),
        "package activation must not grant capabilities: {records}"
    );
    assert!(
        !records.contains("self-granted"),
        "package code must never record a successful self-grant: {records}"
    );
}
