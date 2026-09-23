//! Plan 115 task 5: `clay install`/`remove`/`list` verb behavior.
//!
//! Parse coverage lives in `src/cli.rs`. Binary e2e against a live registry
//! waits for the task 13 fixture; these tests drive the shared verb helpers
//! through `FakeBackend`.

use clay::packages::manager::FakeBackend;
use clay::packages::service::{PackageService, PackageServiceError};
use clay::packages::verbs;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn fixture(name: &str, version: &str) -> serde_json::Value {
    json!({
        "name": name,
        "version": version,
        "type": "module",
        "exports": { ".": "./index.js" },
        "clay": {
            "apiPrefix": "cli",
            "entry": "./index.js",
            "loadEntry": "./index.js",
            "permissions": [],
            "modes": ["cli"],
            "docs": "./docs.md"
        }
    })
}

fn scratch(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "clay-package-cli-{}-{}-{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write_init_js(config_root: &std::path::Path) {
    fs::create_dir_all(config_root).unwrap();
    fs::write(
        config_root.join("init.js"),
        "import { loadPackage } from \"clay:packages\";\n",
    )
    .unwrap();
}

#[test]
fn install_records_ledger_appends_load_line_and_does_not_adopt() {
    let root = scratch("install");
    let config_root = root.join("config");
    write_init_js(&config_root);
    let spec = "npm:@vendor/mode";
    let backend = FakeBackend::new().will_install(spec, fixture("@vendor/mode", "2.3.4"));
    let mut service = PackageService::new(root.join("store"), Box::new(backend));
    let mut out = Vec::new();
    verbs::install(
        &mut service,
        spec,
        Default::default(),
        &config_root,
        "npm",
        &mut out,
    )
    .expect("install");
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("Installed @vendor/mode@2.3.4 from npm:@vendor/mode"));
    assert!(text.contains("Not enabled, not adopted"));
    assert!(text.contains("clay package adopt @vendor/mode"));
    assert!(text.contains("Appended loadPackage(\"@vendor/mode\")"));
    let inspection = service.inspect("@vendor/mode").unwrap();
    assert!(!inspection.is_enabled);
    let record = service.install_record("@vendor/mode").unwrap();
    assert!(!record.pinned);
    assert_eq!(record.spec, spec);
    let init_js = fs::read_to_string(config_root.join("init.js")).unwrap();
    assert_eq!(
        init_js
            .matches("await loadPackage(\"@vendor/mode\");")
            .count(),
        1
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_cleans_store_ledger_and_clay_load_line() {
    let root = scratch("remove");
    let config_root = root.join("config");
    write_init_js(&config_root);
    let spec = "npm:plain-mode@1.0.0";
    let backend = FakeBackend::new().will_install(spec, fixture("plain-mode", "1.0.0"));
    let mut service = PackageService::new(root.join("store"), Box::new(backend));
    verbs::install(
        &mut service,
        spec,
        Default::default(),
        &config_root,
        "npm",
        &mut Vec::new(),
    )
    .unwrap();
    let mut out = Vec::new();
    verbs::remove(&mut service, "npm:plain-mode", &config_root, &mut out).expect("remove");
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("Removed plain-mode"));
    assert!(text.contains("Removed loadPackage(\"plain-mode\")"));
    assert!(service.inspect("plain-mode").is_none());
    assert!(service.install_record("plain-mode").is_none());
    let init_js = fs::read_to_string(config_root.join("init.js")).unwrap();
    assert!(!init_js.contains("loadPackage(\"plain-mode\")"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn list_shows_pinned_floating_and_unmanaged() {
    let root = scratch("list");
    let pinned = FakeBackend::new().will_install(
        "npm:@vendor/pinned@1.2.3",
        fixture("@vendor/pinned", "1.2.3"),
    );
    let mut service = PackageService::new(root.join("store"), Box::new(pinned));
    service
        .install("npm:@vendor/pinned@1.2.3", Default::default())
        .unwrap();

    let floating =
        FakeBackend::new().will_install("npm:@vendor/float", fixture("@vendor/float", "9.9.9"));
    let mut floating_service = PackageService::new(root.join("store-float"), Box::new(floating));
    floating_service
        .install("npm:@vendor/float", Default::default())
        .unwrap();
    // Merge floating into the listed service via a second install backend is
    // awkward; inspect the floating service's list line separately.
    let unmanaged_json = fixture("@vendor/unmanaged", "0.1.0");
    service.install_from_value(unmanaged_json).unwrap();

    let mut out = Vec::new();
    verbs::list(&service, false, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("@vendor/pinned"));
    assert!(text.contains("pinned"));
    assert!(text.contains("npm:@vendor/pinned@1.2.3"));
    assert!(text.contains("@vendor/unmanaged"));
    assert!(text.contains("unmanaged"));

    let mut floating_out = Vec::new();
    verbs::list(&floating_service, false, &mut floating_out).unwrap();
    let floating_text = String::from_utf8(floating_out).unwrap();
    assert!(floating_text.contains("@vendor/float"));
    assert!(floating_text.contains("floating"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_bundled_inventory_name_fails_closed() {
    let root = scratch("bundled");
    let mut service = PackageService::new(root.join("store"), Box::new(FakeBackend::new()));
    let error = verbs::remove(&mut service, "@clay/markdown", &root, &mut Vec::new())
        .expect_err("bundled remove must fail");
    assert!(error.contains("bundled"));
    assert!(error.contains("@clay/markdown"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn install_without_init_js_still_installs() {
    let root = scratch("no-init");
    let spec = "npm:@vendor/mode";
    let backend = FakeBackend::new().will_install(spec, fixture("@vendor/mode", "1.0.0"));
    let mut service = PackageService::new(root.join("store"), Box::new(backend));
    let mut out = Vec::new();
    verbs::install(
        &mut service,
        spec,
        Default::default(),
        &root.join("missing-config"),
        "pnpm",
        &mut out,
    )
    .expect("install succeeds without init.js");
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("Skipped load line"));
    assert!(service.inspect("@vendor/mode").is_some());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn update_extensions_updates_floating_skips_pinned_and_unmanaged() {
    let root = scratch("update-ext");
    let config_root = root.join("config");
    write_init_js(&config_root);
    let backend = FakeBackend::new()
        .will_install("npm:@vendor/float", fixture("@vendor/float", "1.0.0"))
        .will_install(
            "npm:@vendor/pinned@1.2.3",
            fixture("@vendor/pinned", "1.2.3"),
        );
    let mut service = PackageService::new(root.join("store"), Box::new(backend));
    verbs::install(
        &mut service,
        "npm:@vendor/float",
        Default::default(),
        &config_root,
        "npm",
        &mut Vec::new(),
    )
    .unwrap();
    verbs::install(
        &mut service,
        "npm:@vendor/pinned@1.2.3",
        Default::default(),
        &config_root,
        "npm",
        &mut Vec::new(),
    )
    .unwrap();
    service
        .install_from_value(fixture("@vendor/unmanaged", "0.1.0"))
        .unwrap();

    let mut out = Vec::new();
    verbs::update_extensions(&mut service, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("Updated @vendor/float@1.0.0 from npm:@vendor/float"));
    assert!(text.contains("Not enabled, not adopted"));
    assert!(text.contains(verbs::PINNED_SKIP_HINT));
    assert!(text.contains("Skipped @vendor/pinned"));
    assert!(!text.contains("unmanaged"));
    assert!(!text.contains("Updating Clay"));
    assert!(!service.inspect("@vendor/float").unwrap().is_enabled);
    assert!(!service.inspect("@vendor/pinned").unwrap().is_enabled);
    assert!(service.install_record("@vendor/unmanaged").is_none());
    let init_js = fs::read_to_string(config_root.join("init.js")).unwrap();
    assert_eq!(
        init_js
            .matches("await loadPackage(\"@vendor/float\");")
            .count(),
        1
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn update_one_package_skips_pinned_and_unmanaged() {
    let root = scratch("update-one");
    let backend = FakeBackend::new()
        .will_install("npm:@vendor/float", fixture("@vendor/float", "2.0.0"))
        .will_install(
            "npm:@vendor/pinned@9.9.9",
            fixture("@vendor/pinned", "9.9.9"),
        );
    let mut service = PackageService::new(root.join("store"), Box::new(backend));
    service
        .install("npm:@vendor/float", Default::default())
        .unwrap();
    service
        .install("npm:@vendor/pinned@9.9.9", Default::default())
        .unwrap();
    service
        .install_from_value(fixture("@vendor/unmanaged", "0.1.0"))
        .unwrap();

    let mut floating = Vec::new();
    verbs::update_package(&mut service, "npm:@vendor/float", &mut floating).unwrap();
    assert!(
        String::from_utf8(floating)
            .unwrap()
            .contains("Updated @vendor/float@2.0.0")
    );

    let mut pinned = Vec::new();
    verbs::update_package(&mut service, "npm:@vendor/pinned@9.9.9", &mut pinned).unwrap();
    let pinned_text = String::from_utf8(pinned).unwrap();
    assert!(pinned_text.contains(verbs::PINNED_SKIP_HINT));
    assert!(!pinned_text.contains("Updated"));

    let mut unmanaged = Vec::new();
    verbs::update_package(&mut service, "npm:@vendor/unmanaged", &mut unmanaged).unwrap();
    assert!(
        String::from_utf8(unmanaged)
            .unwrap()
            .contains("not a Clay-managed install")
    );
    let _ = fs::remove_dir_all(&root);
}

// ── Plan 136 task 5: `clay package authorize` ────────────────────────────────

/// Fixture with two declared capabilities, one of them ungranted in the tests
/// that exercise the `Ungranted:` inspect line.
fn grant_fixture(name: &str) -> serde_json::Value {
    json!({
        "name": name,
        "version": "1.0.0",
        "type": "module",
        "exports": { ".": "./index.js" },
        "clay": {
            "apiPrefix": "grantfixture",
            "entry": "./index.js",
            "loadEntry": "./index.js",
            "permissions": ["mode-registration", "completion-provider"],
            "modes": ["grantfixture"],
            "docs": "./docs.md"
        }
    })
}

fn install_and_adopt(service: &mut PackageService, name: &str) -> serde_json::Value {
    let package = grant_fixture(name);
    service
        .install_from_value(package.clone())
        .expect("fixture installs");
    service
        .approve_package(name, "cli")
        .expect("fixture adopts");
    package
}

#[test]
fn cli_authorize_records_capability_grants() {
    let root = scratch("authorize");
    let name = "@vendor/granted";
    let mut service = PackageService::open(&root, Box::<FakeBackend>::default()).expect("store");
    install_and_adopt(&mut service, name);

    let mut out = Vec::new();
    verbs::authorize(
        &mut service,
        name,
        &["completion-provider".to_string()],
        None,
        "cli",
        &mut out,
    )
    .expect("authorize succeeds");
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("Authorized @vendor/granted: completion-provider (native-trust)"),
        "unexpected output: {text}"
    );
    assert!(
        text.contains("granted by:  cli"),
        "unexpected output: {text}"
    );

    let inspection = service.inspect(name).expect("inspects");
    assert_eq!(
        inspection.approved_capabilities,
        vec!["completion-provider"]
    );
    assert_eq!(inspection.runtime_profile.as_deref(), Some("native-trust"));
    let provenance = inspection
        .grant_provenance
        .as_ref()
        .expect("grant provenance is recorded");
    assert_eq!(provenance.granted_by, "cli");
    assert_eq!(provenance.approved_by, "cli");
    assert!(!provenance.granted_at.is_empty() && !provenance.approved_at.is_empty());

    let lines = verbs::format_grant_lines(&inspection).join("\n");
    assert!(lines.contains("Grants:      completion-provider (native-trust)"));
    assert!(lines.contains("Granted by:  cli at "));
    assert!(lines.contains("Approved by: cli at "));
    assert!(
        lines.contains("Ungranted:   mode-registration (declared, not granted)"),
        "declared-but-ungranted capabilities must be visible: {lines}"
    );

    // A fresh process reads the same grant: it is durable, not just in-memory.
    drop(service);
    let mut reopened = PackageService::open(&root, Box::<FakeBackend>::default()).expect("store");
    reopened
        .install_from_value(grant_fixture(name))
        .expect("re-discovery installs");
    assert_eq!(
        reopened
            .inspect(name)
            .expect("inspects")
            .approved_capabilities,
        vec!["completion-provider"]
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_authorize_rejects_undeclared_capability() {
    let root = scratch("authorize-undeclared");
    let name = "@vendor/granted";
    let mut service = PackageService::open(&root, Box::<FakeBackend>::default()).expect("store");
    install_and_adopt(&mut service, name);

    let undeclared = verbs::authorize(
        &mut service,
        name,
        &["command-registration".to_string()],
        None,
        "cli",
        &mut Vec::new(),
    )
    .expect_err("an undeclared capability must be rejected");
    assert!(
        undeclared.contains("does not declare capability `command-registration`"),
        "unexpected error: {undeclared}"
    );
    let unknown = verbs::authorize(
        &mut service,
        name,
        &["not-a-capability".to_string()],
        None,
        "cli",
        &mut Vec::new(),
    )
    .expect_err("an unknown capability name must be rejected");
    assert!(
        unknown.contains("unknown capability `not-a-capability`"),
        "unexpected error: {unknown}"
    );
    let bad_profile = verbs::authorize(
        &mut service,
        name,
        &["completion-provider".to_string()],
        Some("godmode"),
        "cli",
        &mut Vec::new(),
    )
    .expect_err("an unknown runtime profile must be rejected");
    assert!(
        bad_profile.contains("unknown runtime profile `godmode`"),
        "unexpected error: {bad_profile}"
    );
    assert!(
        service
            .inspect(name)
            .expect("inspects")
            .approved_capabilities
            .is_empty(),
        "a rejected authorize must record nothing"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cli_authorize_then_enable_succeeds_and_without_it_fails_closed() {
    let root = scratch("authorize-enable");
    let name = "@vendor/granted";
    let mut service = PackageService::open(&root, Box::<FakeBackend>::default()).expect("store");
    service
        .install_from_value(grant_fixture(name))
        .expect("fixture installs");

    // Adoption first: a CLI grant on an unadopted package would vanish with
    // the process, so the verb refuses instead of pretending it persisted.
    let unadopted = verbs::authorize(
        &mut service,
        name,
        &["completion-provider".to_string()],
        None,
        "cli",
        &mut Vec::new(),
    )
    .expect_err("authorize requires adoption");
    assert!(
        unadopted.contains("is not adopted"),
        "unexpected error: {unadopted}"
    );
    service.approve_package(name, "cli").expect("adopts");

    let error = service
        .enable(name)
        .expect_err("an adopted package with no grant fails closed");
    assert!(
        matches!(error, PackageServiceError::MissingCapabilityGrant { .. }),
        "expected MissingCapabilityGrant, got {error}"
    );

    // A partial grant still fails closed: every declared capability needs one.
    verbs::authorize(
        &mut service,
        name,
        &["completion-provider".to_string()],
        None,
        "cli",
        &mut Vec::new(),
    )
    .expect("authorize succeeds");
    assert!(
        service.enable(name).is_err(),
        "an ungranted declared capability must still block enable"
    );

    // A grant is a complete set, not an increment: re-authorizing with one
    // capability replaces the previous grant.
    verbs::authorize(
        &mut service,
        name,
        &["mode-registration".to_string()],
        None,
        "cli",
        &mut Vec::new(),
    )
    .expect("authorize succeeds");
    assert!(
        service.enable(name).is_err(),
        "the replaced grant must no longer cover completion-provider"
    );

    let mut out = Vec::new();
    verbs::authorize(
        &mut service,
        name,
        &[
            "completion-provider".to_string(),
            "mode-registration".to_string(),
        ],
        Some("sandboxed"),
        "user",
        &mut out,
    )
    .expect("authorize succeeds");
    assert!(String::from_utf8(out).unwrap().contains(
        "Authorized @vendor/granted: completion-provider, mode-registration (sandboxed)"
    ));
    service.enable(name).expect("fully granted package enables");
    let inspection = service.inspect(name).expect("inspects");
    assert!(inspection.is_enabled);
    assert!(
        !verbs::format_grant_lines(&inspection)
            .join("\n")
            .contains("Ungranted:"),
        "every declared capability is granted"
    );
    let _ = fs::remove_dir_all(&root);
}
