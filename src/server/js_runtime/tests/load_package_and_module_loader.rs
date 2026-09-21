use super::*;

#[tokio::test]
async fn load_package_user_installed_default_loads_from_init_js() {
    // Plan 035 task 8: the one-line end-user default loads an installed,
    // authorized, user-installed package from a genuine `init.js` config
    // root. No inline manifest, no per-primitive registration, and no
    // manual facade plumbing in user config — `loadPackage` owns all of it.
    let config_root = config_fixture("init-js-user-package");
    let package_root = config_root
        .join("node_modules")
        .join("@vendor")
        .join("mode");
    let init_js = r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("github:vendor/mode");
        "#;
    fs::write(config_root.join("init.js"), init_js).unwrap();

    // The user config carries no manifest object and no per-primitive
    // registration calls — loadPackage does all of it.
    for forbidden in [
        "contributions",
        "modePattern",
        "serverRegisterCommand",
        "serverRegisterParseHandler",
        "serverActivateMajorMode",
        "markdownPackageManifest",
    ] {
        assert!(
            !init_js.contains(forbidden),
            "default init.js must not carry `{forbidden}` for a user-installed package"
        );
    }

    let result = evaluate_init_js_with_seeded_package(
        config_root.clone(),
        "github:vendor/mode",
        "@vendor/mode",
        "vendormode",
        package_root.clone(),
        r#"Deno.core.ops.op_clay_runtime_record("user-installed init.js load"); export default function load() {}"#,
    )
    .await
    .expect("one-line init.js load must succeed for installed user package");

    // The package loadEntry default export ran (it recorded an op), proving
    // activation went through the shared resolver + enable + authorize +
    // loadEntry import path from a real init.js file.
    assert_eq!(result.op_records, vec!["user-installed init.js load"]);
    let _ = fs::remove_dir_all(config_root);
}

#[tokio::test]
async fn op_clay_packages_load_package_by_specifier_rejects_uninstalled_specifier() {
    // Source-aware loading no longer categorically rejects npm/GitHub/local
    // shapes. They must still exist in the package service's installed and
    // authorized registry before runtime loading can proceed.
    for denied in [
        "left-pad",
        "github:user/mode",
        "./local-package",
        "../escape",
        "/absolute/package",
    ] {
        let err = resolve_by_specifier(denied).await.unwrap_err();
        assert!(
            err.contains("packages.not_installed"),
            "uninstalled specifier `{denied}` must be not_installed, got: {err}"
        );
    }
}

#[tokio::test]
async fn op_clay_packages_load_package_by_specifier_rejects_invalid_bundled_specifier() {
    for denied in [
        "@clay/",
        "@clay/../escape",
        "@clay/foo/bar",
        "@clay/markdown?tag=latest",
        "@clay/markdown#hash",
    ] {
        let err = resolve_by_specifier(denied).await.unwrap_err();
        assert!(
            err.contains("packages.invalid_specifier"),
            "invalid bundled specifier `{denied}` must be invalid_specifier, got: {err}"
        );
    }
}

#[tokio::test]
async fn load_package_loads_authorized_npm_style_fixture() {
    let root = config_fixture("npm-package-load")
        .join("node_modules")
        .join("left-pad");
    let result = evaluate_with_seeded_package(
        "left-pad",
        "left-pad",
        "leftpad",
        root.clone(),
        r#"Deno.core.ops.op_clay_runtime_record("npm fixture loaded"); export default function load() {}"#,
    )
    .await
    .expect("authorized npm-style package must load through shared package path");

    assert_eq!(result.op_records, vec!["npm fixture loaded"]);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn load_package_loads_authorized_github_requested_spec_fixture() {
    let root = config_fixture("github-package-load")
        .join("node_modules")
        .join("@vendor")
        .join("mode");
    let result = evaluate_with_seeded_package(
        "github:vendor/mode",
        "@vendor/mode",
        "vendormode",
        root.clone(),
        r#"import "./helper.js"; export default function load() {}"#,
    )
    .await
    .expect("authorized scoped package must load through shared package path");

    assert_eq!(result.op_records, vec!["helper loaded"]);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn load_package_completion_provider_fixture_registers_metadata() {
    let root = config_fixture("completion-provider-package-load").join("completion-provider");
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterCompletionProvider } from "clay:completion";
        export default function load() {
          serverRegisterCompletionProvider({});
        }
        "#,
    );
    let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
        Arc::new(Mutex::new(WorkspaceState::new())),
        1,
    ));
    let _third_party_worker = wire_test_third_party_bridge(&op_state);
    let mut package_json = loadable_package_fixture("completion-provider", "completionprovider");
    package_json["clay"]["permissions"] = serde_json::json!(["completion-provider"]);
    package_json["clay"]["contributions"]["completionProviders"] = serde_json::json!([{
        "id": "completionprovider.words",
        "triggerCharacters": ["."],
        "budgets": { "timeoutMs": 50, "maxItems": 20 }
    }]);
    {
        let mut service = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        service
            .install_from_value_at_root_with_spec(package_json, root.clone(), "completion-provider")
            .expect("seed completion package install succeeds");
        service
            .authorize_package(
                "completion-provider",
                vec![crate::packages::permissions::PackagePermission::CompletionProvider],
                crate::packages::authorization::RuntimeProfile::NativeTrust,
                "test-user",
            )
            .expect("seed completion package authorization succeeds");
        service
            .approve_package("completion-provider", "test")
            .expect("seed completion package adoption approval succeeds");
    }
    let main_specifier = ModuleSpecifier::parse(CONTROLLED_MAIN_SPECIFIER).unwrap();
    let loader = Rc::new(ClayModuleLoader::new(
        main_specifier,
        None,
        None,
        op_state.load_entry_allowlist(),
        crate::packages::bundled::RuntimeDomain::Trusted,
    ));
    let (mut runtime, heap_limit_hit) = create_js_runtime(
        Arc::clone(&op_state),
        Rc::clone(&loader),
        JS_RUNTIME_HEAP_LIMIT_BYTES,
        crate::packages::bundled::RuntimeDomain::Trusted,
    );
    let loaded = prepare_runtime_entry(
        RuntimeEntry::ControlledSource(
            r#"
            import { loadPackage } from "clay:packages";
            await loadPackage("completion-provider");
            "#
            .to_string(),
        ),
        1,
    )
    .unwrap();
    loader.set_entry(
        loaded.main_specifier.clone(),
        loaded.main_source.clone(),
        loaded.configuration.clone(),
    );
    let result = evaluate_loaded_module(
        &mut runtime,
        &op_state,
        loaded,
        Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
        true,
        &heap_limit_hit,
    )
    .await
    .expect("completion provider loadPackage path succeeds");

    assert_eq!(result.completion_providers.len(), 1);
    assert_eq!(
        result.completion_providers[0].id,
        "completionprovider.words"
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn load_package_loads_authorized_local_requested_spec_fixture() {
    let root = config_fixture("local-package-load").join("local-package");
    let result = evaluate_with_seeded_package(
        "./local-package",
        "local-package",
        "localpackage",
        root.clone(),
        r#"Deno.core.ops.op_clay_runtime_record("local fixture loaded"); export default function load() {}"#,
    )
    .await
    .expect("authorized local package spec must load through shared package path");

    assert_eq!(result.op_records, vec!["local fixture loaded"]);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn load_package_rejects_escaping_relative_import_from_package_root() {
    let root = config_fixture("escaping-package-load").join("evil-mode");
    fs::create_dir_all(root.parent().unwrap()).expect("create parent fixture root");
    fs::write(root.parent().unwrap().join("escape.js"), "export {};\n")
        .expect("write outside escape module");
    let err = evaluate_with_seeded_package(
        "evil-mode",
        "evil-mode",
        "evilmode",
        root.clone(),
        r#"import "../escape.js"; export default function load() {}"#,
    )
    .await
    .unwrap_err();

    let message = err.to_string();
    assert!(
        message.contains("runtime.invalid_import"),
        "escaping relative import must fail at module loader boundary, got: {message}"
    );
    let _ = fs::remove_dir_all(root.parent().unwrap());
}

/// Plan 061 task 10 sentinel: an unapproved third-party package's
/// JavaScript never executes — `loadPackage` fails at the adoption gate
/// before the load entry module is imported/evaluated.
#[tokio::test]
async fn unapproved_third_party_package_never_executes_before_adoption() {
    let root = config_fixture("unapproved-package-load").join("stealth-package");
    let err = evaluate_with_seeded_package_adoption(
        "stealth-package",
        "stealth-package",
        "stealthpackage",
        root.clone(),
        r#"Deno.core.ops.op_clay_runtime_record("SENTINEL-EXECUTED"); export default function load() {}"#,
        false,
    )
    .await
    .unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("package_approval.missing"),
        "unapproved loadPackage must fail at the adoption gate, got: {message}"
    );
    assert!(
        !message.contains("SENTINEL-EXECUTED"),
        "package load entry must never execute before approval, got: {message}"
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn op_clay_packages_load_package_by_specifier_rejects_unknown_package() {
    // `@clay/*` shape but no installed package on disk.
    let err = resolve_by_specifier("@clay/does-not-exist")
        .await
        .unwrap_err();
    assert!(
        err.contains("packages.not_installed"),
        "unknown first-party package must be not_installed, got: {err}"
    );
}

#[tokio::test]
async fn op_clay_packages_load_package_by_specifier_resolves_and_enables_first_party_markdown() {
    // The real shipped `@clay/markdown` package validates/enables through
    // PackageService and returns an opaque loadEntrySpecifier. The module
    // import itself is task 4/5; here we prove resolve + enable works and
    // the opaque specifier is recorded in the allowlist via the returned
    // summary shape.
    let source = r#"
        const raw = Deno.core.ops.op_clay_packages_load_package_by_specifier(
          JSON.stringify({ specifier: "@clay/markdown" })
        );
        const summary = JSON.parse(raw);
        globalThis.__clay_summary = summary;
    "#;
    let evaluation = ClayJsRuntimeService::default()
        .evaluate_controlled_module(source)
        .await
        .expect("@clay/markdown must resolve and enable");

    // The op returns the typed summary as a JSON string; we cannot read
    // `globalThis` after the runtime tears down, so we assert the op ran
    // without error and that subsequent resolver calls for the same
    // package succeed (idempotent enable via AlreadyEnabled fallback).
    assert!(evaluation.behavior_manifest.is_none());
    let second = resolve_by_specifier("@clay/markdown").await;
    assert!(
        second.is_ok(),
        "resolving an already-enabled package must be idempotent, got: {second:?}"
    );
}

#[test]
fn clay_module_loader_loads_allowlisted_first_party_load_entry() {
    // A real on-disk loadEntry OUTSIDE any config root, recorded in the
    // allowlist (what the resolver op does), must resolve and load.
    let outside_root = config_fixture("loader-loadentry");
    let loadentry_path = outside_root.join("load.js");
    fs::write(&loadentry_path, "export const clayLoadedEntry = true;\n").unwrap();

    let opaque = "clay://packages/@clay/example/dist/load.js";
    let loader = loader_with_allowlist(&[(opaque, loadentry_path, outside_root)], None);

    let resolved = loader
        .resolve(opaque, "clay://runtime/main.js", ResolutionKind::Import)
        .expect("allowlisted loadEntry must resolve");
    assert_eq!(resolved.as_str(), opaque);

    let source = match loader.load(&resolved, None, default_load_options()) {
        ModuleLoadResponse::Sync(Ok(source)) => source,
        ModuleLoadResponse::Sync(Err(error)) => panic!("load failed: {error:?}"),
        _ => panic!("expected sync response, got async"),
    };
    assert_eq!(source.module_type, ModuleType::JavaScript);
    assert!(
        std::str::from_utf8(source.code.as_bytes())
            .unwrap()
            .contains("clayLoadedEntry"),
        "load must return the recorded on-disk loadEntry source"
    );
}

#[test]
fn package_load_entry_allowlist_revokes_owned_entries() {
    let root = config_fixture("loader-revoke-package");
    let loadentry_path = root.join("load.js");
    let helper_path = root.join("helper.js");
    fs::write(&loadentry_path, "import './helper.js';\n").unwrap();
    fs::write(&helper_path, "export const helper = true;\n").unwrap();
    let allowlist = PackageLoadEntryAllowlist::default();
    let opaque = "clay://packages/@vendor/example/dist/load.js";
    let canonical_root = std::fs::canonicalize(&root).unwrap();
    let canonical_loadentry = std::fs::canonicalize(&loadentry_path).unwrap();
    allowlist.record_for_package(
        opaque,
        canonical_loadentry,
        canonical_root,
        Some("@vendor/example"),
    );
    let helper = allowlist
        .resolve_relative(opaque, "./helper.js")
        .expect("relative helper import is recorded with same owner");

    assert_eq!(allowlist.revoke_package("@vendor/example"), 2);
    assert!(allowlist.absolute_path(opaque).is_none());
    assert!(allowlist.absolute_path(&helper).is_none());
}

#[test]
fn clay_module_loader_denies_unallowlisted_first_party_url() {
    // Empty allowlist: every `clay://packages/...` URL is denied exactly
    // like any other untrusted specifier, even loadEntry-shaped ones.
    let loader = loader_with_allowlist(&[], None);
    for url in [
        "clay://packages/@clay/markdown/dist/load.js",
        "clay://packages/@clay/coding-agent/dist/load.js",
        "clay://packages/@clay/evil/x.js",
        "clay://packages/anything",
    ] {
        let error = loader
            .resolve(url, "clay://runtime/main.js", ResolutionKind::Import)
            .expect_err("unallowlisted package URL must be denied");
        assert!(
            error.to_string().contains("runtime.invalid_import"),
            "unallowlisted `{url}` must be denied, got: {error:?}"
        );
    }
}

#[test]
fn clay_module_loader_preserves_config_root_confinement_for_non_package_imports() {
    // A real config root exercises the configuration branch. The allowlist
    // addition must NOT relax config-root confinement: escaping imports are
    // still rejected, while an allowlisted package loadEntry still loads.
    let parent = config_fixture("loader-configroot-parent");
    let root = parent.join("config");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("init.js"), "export const ready = true;\n").unwrap();
    // `escape.js` lives in the parent (a real file OUTSIDE config root) so
    // `canonicalize` succeeds and the `starts_with(config_root)` check is
    // the thing that rejects it.
    fs::write(parent.join("escape.js"), "export const escape = true;\n").unwrap();
    let configuration = Arc::new(ConfigurationRuntime::from_config_root(&root).unwrap());

    // Allowlisted loadEntry lives OUTSIDE the config root but still loads.
    let outside = config_fixture("loader-configroot-loadentry");
    let loadentry_path = outside.join("load.js");
    fs::write(&loadentry_path, "export const ok = true;\n").unwrap();
    let opaque = "clay://packages/@clay/example/dist/load.js";
    let loader = loader_with_allowlist(
        &[(opaque, loadentry_path, outside.clone())],
        Some(configuration),
    );

    let resolved = loader
        .resolve(opaque, "clay:configuration", ResolutionKind::Import)
        .expect("allowlisted loadEntry loads even with a config root present");

    // Escaping relative imports (not validated loadEntries) stay confined.
    let escape_err = loader
        .resolve("../escape.js", "clay:configuration", ResolutionKind::Import)
        .expect_err("escaping import must be denied by config-root confinement");
    assert!(
        escape_err.to_string().contains("configuration directory"),
        "config-root confinement must reject escaping imports, got: {escape_err:?}"
    );

    // And the allowlisted entry still returns its on-disk source alongside.
    let source = match loader.load(&resolved, None, default_load_options()) {
        ModuleLoadResponse::Sync(Ok(source)) => source,
        ModuleLoadResponse::Sync(Err(error)) => panic!("load failed: {error:?}"),
        _ => panic!("expected sync response, got async"),
    };
    assert!(
        std::str::from_utf8(source.code.as_bytes())
            .unwrap()
            .contains("ok = true"),
        "allowlisted loadEntry must load alongside config-root confinement"
    );
}

#[test]
fn clay_module_loader_denies_arbitrary_file_url_or_https_specifier() {
    // `file://`, `https://`, `http://`, bare, and scheme-bearing specifiers
    // that are not curated facades or allowlisted loadEntries stay denied.
    let loader = loader_with_allowlist(&[], None);
    for specifier in [
        "file:///etc/passwd",
        "https://example.com/evil.js",
        "http://example.com/x.js",
        "react",
        "node:fs",
        "npm:lodash",
    ] {
        let error = loader
            .resolve(specifier, "clay://runtime/main.js", ResolutionKind::Import)
            .expect_err("non-allowlisted specifier must be denied");
        assert!(
            error.to_string().contains("runtime.invalid_import"),
            "specifier `{specifier}` must be denied, got: {error:?}"
        );
    }
}

#[test]
fn clay_module_loader_denies_load_entry_imports_outside_package_root() {
    // Phase 18.6 task 7 security boundary: a validated package loadEntry
    // may import its own sibling modules (e.g. `./index.js`) — those are
    // confined to the validated package root by `resolve_relative`. But an
    // import that ESCAPES the package root (e.g. `../escape.js` landing
    // outside it) must be denied so a package cannot read arbitrary files
    // outside its validated root. This is the transitive-load confinement
    // gate added in task 5.
    let outside = config_fixture("pkg-escape-root");
    let package_root = outside.join("pkg");
    let dist = package_root.join("dist");
    fs::create_dir_all(&dist).unwrap();
    let load_entry = dist.join("load.js");
    fs::write(&load_entry, "// loadEntry").unwrap();
    // A legitimate sibling inside the package root.
    let sibling = dist.join("index.js");
    fs::write(&sibling, "// sibling").unwrap();
    // An escape file OUTSIDE the package root (in the fixture parent).
    let escape = outside.join("escape.js");
    fs::write(&escape, "// secret").unwrap();

    let opaque = "clay://packages/@clay/example/dist/load.js";
    let allowlist = Arc::new(PackageLoadEntryAllowlist::default());
    allowlist.record(
        opaque,
        load_entry.canonicalize().unwrap(),
        package_root.canonicalize().unwrap(),
    );

    // Legitimate sibling import inside the package root resolves.
    let ok = allowlist.resolve_relative(opaque, "./index.js");
    assert!(
        ok.is_some(),
        "a sibling import inside the validated package root must resolve"
    );
    // An import that escapes the package root is denied (returns None).
    assert_eq!(
        allowlist.resolve_relative(opaque, "../escape.js"),
        None,
        "an import escaping the validated package root must be denied"
    );
    // A deep escape attempt is also denied.
    assert_eq!(
        allowlist.resolve_relative(opaque, "../../escape.js"),
        None,
        "a deep-escape import must be denied"
    );
    // A relative import from an unknown referrer (not in the allowlist) is
    // denied — the confinement gate only fires for validated package modules.
    assert_eq!(
        allowlist.resolve_relative("clay://packages/@clay/unknown/dist/x.js", "./y.js"),
        None,
        "a relative import from a non-validated referrer must be denied"
    );
}
