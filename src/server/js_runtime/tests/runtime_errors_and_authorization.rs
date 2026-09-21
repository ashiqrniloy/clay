use super::*;

#[test]
fn ordinary_typing_does_not_enter_js_runtime() {
    let service = ClayJsRuntimeService::default();

    assert_eq!(service.evaluation_count(), 0);
}

#[tokio::test]
async fn js_runtime_errors_are_typed_not_panics() {
    let service = ClayJsRuntimeService::default();
    let error = service
        .evaluate_controlled_module(r#"Deno.core.ops.op_clay_runtime_record("");"#)
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    assert!(error.to_string().contains("runtime.invalid_record"));
    assert_eq!(service.evaluation_count(), 0);
}

#[tokio::test]
async fn runtime_syntax_error_reports_diagnostic() {
    let error = ClayJsRuntimeService::default()
        .evaluate_controlled_module(r#"const broken = ;"#)
        .await
        .unwrap_err();
    let diagnostic = error.diagnostic();

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.code, "runtime.syntax_error");
    assert_eq!(
        diagnostic.message,
        "JavaScript syntax error while evaluating server-side configuration."
    );
}

#[tokio::test]
async fn runtime_permission_error_reports_sanitized_diagnostic() {
    let error = ClayJsRuntimeService::default()
        .evaluate_controlled_module(r#"import "file:///home/example/.clay/secret.js";"#)
        .await
        .unwrap_err();
    let diagnostic = error.diagnostic();

    assert_eq!(diagnostic.code, "runtime.invalid_import");
    assert!(!diagnostic.message.contains("/home/example"));
    assert!(!diagnostic.message.contains("secret.js"));
}

#[tokio::test]
async fn runtime_op_validation_error_reports_diagnostic() {
    let error = ClayJsRuntimeService::default()
        .evaluate_controlled_module(r#"Deno.core.ops.op_clay_runtime_record("");"#)
        .await
        .unwrap_err();
    let diagnostic = error.diagnostic();

    assert_eq!(diagnostic.code, "runtime.invalid_record");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
}

#[tokio::test]
async fn init_js_authorizes_exact_language_server_before_load_and_package_cannot_self_grant() {
    let config_root = config_fixture("language-server-authority");
    let package_root = config_root.join("package");
    let workspace_root = config_root.join("workspace");
    fs::create_dir_all(&workspace_root).unwrap();
    let executable = std::env::current_exe().unwrap();
    let specifier = "local:language-server-authority";
    let package_name = "@vendor/lsp-authority";
    let contribution = "lsp-authority.server";
    let load_source = format!(
        r#"
        import {{ authorizeLanguageServer }} from "clay:language-server";
        export default async function load() {{
          try {{
            await authorizeLanguageServer({{
              package: {package_name:?},
              contribution: {contribution:?},
              workspaceRootIds: [1],
            }});
            throw new Error("loaded package unexpectedly self-authorized");
          }} catch (error) {{
            // Trusted-domain runtimes reject with authorization_sealed;
            // third-party runtimes fail closed by op absence (Plan 061).
            if (!String(error).includes("authorization_sealed") && !String(error).includes("is not a function")) throw error;
            Deno.core.ops.op_clay_runtime_record("package grant sealed");
          }}
        }}
        "#
    );
    write_loadable_package(&package_root, &load_source);

    let mut package_json = loadable_package_fixture(package_name, "lsp-authority");
    package_json["clay"]["capabilities"] = serde_json::json!(["language-server"]);
    package_json["clay"]["contributions"]["languageServers"] = serde_json::json!([{
        "id": contribution,
        "executable": executable,
        "args": ["--stdio"],
        "inheritEnvironment": ["HOME"]
    }]);

    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
    let workspace_root_id = workspace.lock().await.add_root(&workspace_root).unwrap();
    let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
        Arc::clone(&workspace),
        1,
    ));
    let _third_party_worker = wire_test_third_party_bridge(&op_state);
    op_state.set_runtime_context(Arc::clone(&workspace), 1, true);
    {
        let mut locked = op_state.package_service().lock().unwrap();
        locked
            .install_from_value_at_root_with_spec(package_json, package_root, specifier)
            .unwrap();
        locked.approve_package(package_name, "test").unwrap();
    }

    fs::write(
        config_root.join("init.js"),
        format!(
            r#"
            import {{ authorizeLanguageServer }} from "clay:language-server";
            import {{ loadPackage }} from "clay:packages";
            try {{
              await authorizeLanguageServer({{
                package: {package_name:?},
                contribution: {contribution:?},
                workspaceRootIds: [999999],
              }});
              throw new Error("unknown workspace root unexpectedly authorized");
            }} catch (error) {{
              if (!String(error).includes("unknown_workspace_root")) throw error;
              Deno.core.ops.op_clay_runtime_record("unknown root rejected");
            }}
            const grant = await authorizeLanguageServer({{
              package: {package_name:?},
              contribution: {contribution:?},
              workspaceRootIds: [{workspace_root_id}],
            }});
            Deno.core.ops.op_clay_runtime_record(`granted:${{grant.contribution}}`);
            await loadPackage({specifier:?});
            "#
        ),
    )
    .unwrap();

    let loaded = prepare_runtime_entry(RuntimeEntry::ConfigurationRoot(config_root), 1).unwrap();
    let loader = Rc::new(ClayModuleLoader::new(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
        op_state.load_entry_allowlist(),
        crate::packages::bundled::RuntimeDomain::Trusted,
    ));
    let (mut runtime, heap_limit_hit) = create_js_runtime(
        Arc::clone(&op_state),
        Rc::clone(&loader),
        JS_RUNTIME_HEAP_LIMIT_BYTES,
        crate::packages::bundled::RuntimeDomain::Trusted,
    );
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    let evaluation = evaluate_loaded_module(
        &mut runtime,
        &op_state,
        loaded,
        Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
        true,
        &heap_limit_hit,
    )
    .await
    .unwrap();

    assert_eq!(
        evaluation.op_records,
        [
            "unknown root rejected",
            "granted:lsp-authority.server",
            "package grant sealed"
        ]
    );
    let service = op_state.package_service().lock().unwrap();
    let grant = service
        .language_server_grant(package_name, contribution)
        .unwrap();
    assert_eq!(grant.workspace_root_ids, [workspace_root_id]);
    assert_eq!(
        grant.canonical_executable,
        std::fs::canonicalize(executable).unwrap()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn language_server_facade_round_trips_exact_uint8array_bytes() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;

    let config_root = config_fixture("language-server-bytes");
    let package_root = config_root.join("package");
    let workspace_root = config_root.join("workspace");
    fs::create_dir_all(&workspace_root).unwrap();
    write_loadable_package(&package_root, "export default function load() {}\n");

    let executable = config_root.join("fake-byte-echo");
    let mut file = fs::File::create(&executable).unwrap();
    file.write_all(b"#!/bin/sh\nexec /bin/cat\n").unwrap();
    file.sync_all().unwrap();
    drop(file);
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();

    let package_name = "@vendor/lsp-bytes";
    let contribution = "lspbytes.server";
    let mut package_json = loadable_package_fixture(package_name, "lspbytes");
    package_json["clay"]["capabilities"] = serde_json::json!(["language-server"]);
    package_json["clay"]["contributions"]["languageServers"] = serde_json::json!([{
        "id": contribution,
        "executable": executable,
        "args": [],
        "inheritEnvironment": []
    }]);

    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
    let workspace_root_id = workspace.lock().await.add_root(&workspace_root).unwrap();
    let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
        Arc::clone(&workspace),
        1,
    ));
    let _third_party_worker = wire_test_third_party_bridge(&op_state);
    op_state.set_runtime_context(Arc::clone(&workspace), 1, true);
    {
        let mut service = op_state.package_service().lock().unwrap();
        service
            .install_from_value_at_root_with_spec(
                package_json,
                package_root,
                "local:language-server-bytes",
            )
            .unwrap();
        // Base capabilities first: the init.js language-server grant below
        // augments this record with the `language-server` capability.
        service
            .authorize_package(
                package_name,
                vec![
                    crate::packages::permissions::PackagePermission::ParseDocument,
                    crate::packages::permissions::PackagePermission::RenderDecorations,
                    crate::packages::permissions::PackagePermission::CompletionProvider,
                ],
                crate::packages::authorization::RuntimeProfile::Restricted,
                "test",
            )
            .unwrap();
    }

    // Phase 1 (configuration): approve the contribution/root grant.
    fs::write(
        config_root.join("init.js"),
        format!(
            r#"
            import {{ authorizeLanguageServer }} from "clay:language-server";
            await authorizeLanguageServer({{
              package: {package_name:?},
              contribution: {contribution:?},
              workspaceRootIds: [{workspace_root_id}],
            }});
            "#
        ),
    )
    .unwrap();

    let loaded = prepare_runtime_entry(RuntimeEntry::ConfigurationRoot(config_root), 1).unwrap();
    let loader = Rc::new(ClayModuleLoader::new(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
        op_state.load_entry_allowlist(),
        crate::packages::bundled::RuntimeDomain::Trusted,
    ));
    let (mut runtime, heap_limit_hit) = create_js_runtime(
        Arc::clone(&op_state),
        Rc::clone(&loader),
        JS_RUNTIME_HEAP_LIMIT_BYTES,
        crate::packages::bundled::RuntimeDomain::Trusted,
    );
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    evaluate_loaded_module(
        &mut runtime,
        &op_state,
        loaded,
        Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
        true,
        &heap_limit_hit,
    )
    .await
    .unwrap();

    // Enable with the grant in place, then run the session phase under the
    // package's host-stamped context: sessions are owned by the executing
    // package, never by caller-supplied names.
    let enabled = {
        let mut locked = op_state.package_service().lock().unwrap();
        locked.approve_package(package_name, "test").unwrap();
        locked.enable(package_name).unwrap().clone()
    };
    op_state.set_current_package(Some(crate::server::ops::PackageContext::from_record(
        &enabled,
    )));
    let session_source = format!(
        r#"
        import {{ startLanguageServerSession }} from "clay:language-server";
        const session = await startLanguageServerSession({{
          contribution: {contribution:?},
          workspaceRootId: {workspace_root_id},
        }});
        const sent = new Uint8Array([0, 240, 159, 166, 128, 255]);
        await session.sendBytes(sent);
        const received = [];
        while (received.length < sent.length) {{
          received.push(...await session.readBytes(sent.length - received.length, 2000));
        }}
        Deno.core.ops.op_clay_runtime_record(`bytes:${{received.join(",")}}`);
        await session.stop();
        "#
    );
    let session_entry =
        prepare_runtime_entry(RuntimeEntry::ControlledSource(session_source), 2).unwrap();
    loader.set_entry(
        session_entry.main_specifier.clone(),
        session_entry.main_source.clone(),
        session_entry.configuration.clone(),
    );
    let evaluation = evaluate_loaded_module(
        &mut runtime,
        &op_state,
        session_entry,
        Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
        false,
        &heap_limit_hit,
    )
    .await
    .unwrap();

    assert_eq!(evaluation.op_records, ["bytes:0,240,159,166,128,255"]);
}
