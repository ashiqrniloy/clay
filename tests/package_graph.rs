use clay::packages::authorization::RuntimeProfile;
use clay::packages::manager::FakeBackend;
use clay::packages::permissions::PackagePermission;
use clay::packages::record::assemble_package_record;
use clay::packages::service::{PackageService, PackageServiceError};
use serde_json::{Value, json};

fn graph_fixture(name: &str, prefix: &str, extra_clay: serde_json::Map<String, Value>) -> Value {
    let mut clay = serde_json::Map::from_iter([
        ("apiPrefix".to_string(), json!(prefix)),
        ("entry".to_string(), json!("./dist/index.js")),
        ("loadEntry".to_string(), json!("./dist/load.js")),
        (
            "permissions".to_string(),
            json!(["mode-registration", "mode-activation"]),
        ),
        ("modes".to_string(), json!([prefix])),
        ("docs".to_string(), json!("./docs/index.md")),
    ]);
    clay.extend(extra_clay);
    json!({
        "name": name,
        "version": "0.1.0",
        "type": "module",
        "clay": clay,
    })
}

fn install_and_authorize(
    service: &mut PackageService,
    package_json: Value,
    extra_grants: &[PackagePermission],
) {
    let record = assemble_package_record(&package_json).expect("graph fixture manifest validates");
    let mut grants = record.manifest.clay.permissions.clone();
    grants.extend(extra_grants.iter().copied());
    service
        .install_from_value(package_json)
        .expect("fixture install succeeds");
    service
        .authorize_package(
            &record.manifest.name,
            grants,
            RuntimeProfile::NativeTrust,
            "test-user",
        )
        .expect("fixture authorization succeeds");
}

fn service() -> PackageService {
    PackageService::new("target/test-package-graph", Box::<FakeBackend>::default())
}

#[test]
fn package_with_package_control_disables_first_party_package() {
    let mut service = service();
    let markdown = serde_json::from_str::<Value>(
        &std::fs::read_to_string("packages/markdown/package.json")
            .expect("bundled markdown package exists"),
    )
    .expect("bundled markdown package.json parses");
    install_and_authorize(&mut service, markdown, &[]);
    service.approve_package("@clay/markdown", "test").unwrap();
    service
        .enable("@clay/markdown")
        .expect("markdown enables before package-control package");

    let controller = graph_fixture(
        "@vendor/controller",
        "controller",
        serde_json::Map::from_iter([
            ("permissions".to_string(), json!(["mode-registration"])),
            ("disables".to_string(), json!(["@clay/markdown"])),
        ]),
    );
    install_and_authorize(
        &mut service,
        controller,
        &[PackagePermission::PackageControl],
    );
    service
        .approve_package("@vendor/controller", "test")
        .unwrap();

    service
        .enable("@vendor/controller")
        .expect("package-control grant permits disabling target");

    assert!(service.inspect("@vendor/controller").unwrap().is_enabled);
    assert!(!service.inspect("@clay/markdown").unwrap().is_enabled);
}

#[test]
fn package_extends_target_and_both_remain_active() {
    let mut service = service();
    let base = graph_fixture("@vendor/base", "base", serde_json::Map::new());
    let extension = graph_fixture(
        "@vendor/extension",
        "extension",
        serde_json::Map::from_iter([("extends".to_string(), json!(["@vendor/base"]))]),
    );
    install_and_authorize(&mut service, base, &[]);
    service.approve_package("@vendor/base", "test").unwrap();
    install_and_authorize(&mut service, extension, &[]);
    service
        .approve_package("@vendor/extension", "test")
        .unwrap();

    service
        .enable("@vendor/extension")
        .expect("extension enables target first");

    assert!(service.inspect("@vendor/base").unwrap().is_enabled);
    assert!(service.inspect("@vendor/extension").unwrap().is_enabled);
}

#[test]
fn package_graph_reports_missing_target_deterministically() {
    let mut service = service();
    let package = graph_fixture(
        "@vendor/needs-missing",
        "needsmissing",
        serde_json::Map::from_iter([("dependsOn".to_string(), json!(["@vendor/missing"]))]),
    );
    install_and_authorize(&mut service, package, &[]);

    let err = service.enable("@vendor/needs-missing").unwrap_err();

    match err {
        PackageServiceError::MissingGraphTarget {
            package_name,
            target,
        } => {
            assert_eq!(package_name, "@vendor/needs-missing");
            assert_eq!(target, "@vendor/missing");
        }
        other => panic!("expected missing graph target, got {other:?}"),
    }
}

#[test]
fn package_graph_reports_dependency_cycles_deterministically() {
    let mut service = service();
    let package_a = graph_fixture(
        "@vendor/a",
        "vendora",
        serde_json::Map::from_iter([("dependsOn".to_string(), json!(["@vendor/b"]))]),
    );
    let package_b = graph_fixture(
        "@vendor/b",
        "vendorb",
        serde_json::Map::from_iter([("dependsOn".to_string(), json!(["@vendor/a"]))]),
    );
    install_and_authorize(&mut service, package_a, &[]);
    install_and_authorize(&mut service, package_b, &[]);

    let err = service.enable("@vendor/a").unwrap_err();

    match err {
        PackageServiceError::PackageGraphCycle { cycle } => {
            assert_eq!(cycle, vec!["@vendor/a", "@vendor/b", "@vendor/a"]);
        }
        other => panic!("expected graph cycle, got {other:?}"),
    }
}

#[test]
fn disables_requires_explicit_package_control_grant() {
    let mut service = service();
    let target = graph_fixture("@vendor/target", "target", serde_json::Map::new());
    let controller = graph_fixture(
        "@vendor/no-control",
        "nocontrol",
        serde_json::Map::from_iter([("disables".to_string(), json!(["@vendor/target"]))]),
    );
    install_and_authorize(&mut service, target, &[]);
    install_and_authorize(&mut service, controller, &[]);

    let err = service.enable("@vendor/no-control").unwrap_err();

    match err {
        PackageServiceError::MissingPackageControlGrant { package_name } => {
            assert_eq!(package_name, "@vendor/no-control");
        }
        other => panic!("expected missing package-control grant, got {other:?}"),
    }
}

// ── Plan 061 task 6: versioned extension points + durable approvals ──────────

fn extension_point_fixture() -> serde_json::Map<String, Value> {
    serde_json::Map::from_iter([(
        "extensionPoints".to_string(),
        json!([{
            "id": "base.completionProviders",
            "version": 1,
            "operations": ["append"],
            "contributionKinds": ["completionProvider"],
            "scopes": ["base.*"]
        }]),
    )])
}

fn relation_request_fixture(operation: &str) -> serde_json::Map<String, Value> {
    serde_json::Map::from_iter([(
        "extends".to_string(),
        json!([{
            "package": "@vendor/base",
            "extensionPoint": "base.completionProviders",
            "version": 1,
            "operation": operation,
            "scopes": ["ext.wikilinks"]
        }]),
    )])
}

fn approval_for(
    requester: &str,
    scopes: Vec<&str>,
) -> clay::packages::approvals::PackageApprovalRecord {
    clay::packages::approvals::PackageApprovalRecord {
        package: requester.to_string(),
        resolved_version: "0.1.0".to_string(),
        source: requester.to_string(),
        integrity: None,
        package_root: "<in-memory>".to_string(),
        api_prefix: "ext".to_string(),
        capabilities: vec![
            "mode-registration".to_string(),
            "mode-activation".to_string(),
        ],
        processes: Vec::new(),
        relations: vec![clay::packages::approvals::ApprovedRelation {
            package: "@vendor/base".to_string(),
            extension_point: "base.completionProviders".to_string(),
            version: 1,
            operation: "append".to_string(),
            scopes: scopes.into_iter().map(str::to_string).collect(),
        }],
        replacements: Vec::new(),
        approved_by: "user".to_string(),
        approved_at: "2026-07-21T00:00:00Z".to_string(),
        revoked: false,
    }
}

#[test]
fn structured_relation_fails_without_owner_extension_point() {
    let mut service = service();
    // Target declares NO extension points: owner consent is absent.
    install_and_authorize(
        &mut service,
        graph_fixture("@vendor/base", "base", serde_json::Map::new()),
        &[],
    );
    service.approve_package("@vendor/base", "test").unwrap();
    install_and_authorize(
        &mut service,
        graph_fixture("@vendor/ext", "ext", relation_request_fixture("append")),
        &[],
    );
    let error = service.enable("@vendor/ext").unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("package_relation.unknown_extension_point"),
        "missing owner declaration must fail closed, got {message}"
    );
    assert!(!service.inspect("@vendor/ext").unwrap().is_enabled);
}

#[test]
fn structured_relation_fails_on_operation_not_offered_by_owner() {
    let mut service = service();
    install_and_authorize(
        &mut service,
        graph_fixture("@vendor/base", "base", extension_point_fixture()),
        &[],
    );
    service.approve_package("@vendor/base", "test").unwrap();
    install_and_authorize(
        &mut service,
        graph_fixture("@vendor/ext", "ext", relation_request_fixture("replace")),
        &[],
    );
    let error = service.enable("@vendor/ext").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("package_relation.operation_not_offered"),
        "operation outside the owner declaration must fail, got {error}"
    );
}

#[test]
fn third_party_relation_requires_exact_durable_user_approval() {
    let mut service = service();
    install_and_authorize(
        &mut service,
        graph_fixture("@vendor/base", "base", extension_point_fixture()),
        &[],
    );
    service.approve_package("@vendor/base", "test").unwrap();
    install_and_authorize(
        &mut service,
        graph_fixture("@vendor/ext", "ext", relation_request_fixture("append")),
        &[],
    );

    // No durable approval: fail closed before the requester enables.
    let error = service.enable("@vendor/ext").unwrap_err();
    assert!(
        error.to_string().contains("package_approval.missing"),
        "missing approval must fail closed, got {error}"
    );
    assert!(!service.inspect("@vendor/ext").unwrap().is_enabled);

    // Approval with an expanded (wrong) version fails as a relation expansion.
    let mut stale = approval_for("@vendor/ext", vec!["ext.wikilinks"]);
    stale.relations[0].version = 2;
    service.record_package_approval(stale).unwrap();
    let error = service.enable("@vendor/ext").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("package_approval.relation_expansion"),
        "stale approval must not cover the request, got {error}"
    );

    // Exact durable approval: enable succeeds and provenance is preserved.
    service
        .record_package_approval(approval_for("@vendor/ext", vec!["ext.wikilinks"]))
        .unwrap();
    service
        .enable("@vendor/ext")
        .expect("exact approved append succeeds");
    assert!(service.inspect("@vendor/ext").unwrap().is_enabled);
    assert!(service.inspect("@vendor/base").unwrap().is_enabled);
}

#[test]
fn structured_relation_rejects_unknown_operation_at_manifest_validation() {
    let mut clay = relation_request_fixture("mutate");
    clay.get_mut("extends").unwrap().as_array_mut().unwrap()[0]["operation"] = json!("mutate");
    let manifest = graph_fixture("@vendor/ext", "ext", clay);
    let error = assemble_package_record(&manifest).unwrap_err();
    assert!(
        error.message.contains("not append/replace"),
        "unknown operation must fail closed, got {}",
        error.message
    );
}

#[test]
fn service_open_fails_closed_on_corrupt_approval_store() {
    let root = std::env::temp_dir().join(format!(
        "clay-approval-store-gate-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let store_path = root.join(clay::packages::approvals::APPROVAL_STORE_FILE_NAME);
    std::fs::write(&store_path, b"{ corrupt").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&store_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let error = match PackageService::open(&root, Box::<FakeBackend>::default()) {
        Ok(_) => panic!("corrupt store must fail closed at service construction"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("package_approval_store.corrupt"),
        "corrupt store must fail closed at service construction, got {error}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn durable_approval_store_round_trips_through_service() {
    let root = std::env::temp_dir().join(format!(
        "clay-approval-store-roundtrip-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut service =
        PackageService::open(&root, Box::<FakeBackend>::default()).expect("empty store opens");
    service
        .record_package_approval(approval_for("@vendor/ext", vec!["ext.wikilinks"]))
        .unwrap();
    drop(service);

    let service = PackageService::open(&root, Box::<FakeBackend>::default()).unwrap();
    let records: Vec<_> = service.package_approvals().collect();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].package, "@vendor/ext");
    assert_eq!(
        records[0].relations[0].extension_point,
        "base.completionProviders"
    );
    let _ = std::fs::remove_dir_all(&root);
}

fn temp_root(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "clay-adoption-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// Plan 061 task 10: no third-party package executes without an exact current
/// durable approval; revocation closes execution again.
#[test]
fn third_party_enable_requires_exact_durable_adoption() {
    let mut service = service();
    install_and_authorize(
        &mut service,
        graph_fixture("@vendor/tool", "tool", serde_json::Map::new()),
        &[],
    );
    let error = service.enable("@vendor/tool").unwrap_err();
    assert!(
        matches!(
            error,
            PackageServiceError::AdoptionRequired { code, .. } if code == "package_approval.missing"
        ),
        "unapproved third-party enable must fail closed, got {error}"
    );
    assert_eq!(
        service.adoption_state("@vendor/tool"),
        Some(clay::packages::service::AdoptionState::Pending)
    );

    service.approve_package("@vendor/tool", "test").unwrap();
    assert_eq!(
        service.adoption_state("@vendor/tool"),
        Some(clay::packages::service::AdoptionState::Approved)
    );
    service
        .enable("@vendor/tool")
        .expect("exact approval enables");

    assert!(service.revoke_package_approval("@vendor/tool").unwrap());
    service.disable("@vendor/tool").unwrap();
    assert_eq!(
        service.adoption_state("@vendor/tool"),
        Some(clay::packages::service::AdoptionState::Revoked)
    );
    let error = service.enable("@vendor/tool").unwrap_err();
    assert!(
        matches!(
            error,
            PackageServiceError::AdoptionRequired { code, .. } if code == "package_approval.revoked"
        ),
        "revoked approval must fail closed, got {error}"
    );
}

/// Plan 061 task 10: approval survives a service restart through the durable
/// store, so future one-line `loadPackage` needs no re-approval.
#[test]
fn adoption_survives_service_restart() {
    let root = temp_root("restart");
    let package_json = graph_fixture("@vendor/tool", "tool", serde_json::Map::new());
    {
        let mut service =
            PackageService::open(&root, Box::<FakeBackend>::default()).expect("store opens");
        let record = assemble_package_record(&package_json).unwrap();
        service.install_from_value(package_json.clone()).unwrap();
        service
            .authorize_package(
                &record.manifest.name,
                record.manifest.clay.permissions.clone(),
                RuntimeProfile::NativeTrust,
                "test-user",
            )
            .unwrap();
        service.approve_package("@vendor/tool", "test").unwrap();
    }
    {
        let mut service =
            PackageService::open(&root, Box::<FakeBackend>::default()).expect("store reopens");
        let record = assemble_package_record(&package_json).unwrap();
        service.install_from_value(package_json).unwrap();
        service
            .authorize_package(
                &record.manifest.name,
                record.manifest.clay.permissions.clone(),
                RuntimeProfile::NativeTrust,
                "test-user",
            )
            .unwrap();
        service
            .enable("@vendor/tool")
            .expect("durable approval covers enable after restart");
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// Plan 061 task 10: version/source drift invalidates the approval (stale),
/// requiring fresh adoption.
#[test]
fn package_update_stales_adoption() {
    let mut service = service();
    install_and_authorize(
        &mut service,
        graph_fixture("@vendor/tool", "tool", serde_json::Map::new()),
        &[],
    );
    service.approve_package("@vendor/tool", "test").unwrap();
    service
        .enable("@vendor/tool")
        .expect("approval enables v0.1.0");
    service.disable("@vendor/tool").unwrap();

    let mut updated = graph_fixture("@vendor/tool", "tool", serde_json::Map::new());
    updated["version"] = json!("0.2.0");
    install_and_authorize(&mut service, updated, &[]);
    assert_eq!(
        service.adoption_state("@vendor/tool"),
        Some(clay::packages::service::AdoptionState::Stale)
    );
    let error = service.enable("@vendor/tool").unwrap_err();
    assert!(
        matches!(
            error,
            PackageServiceError::AdoptionRequired { code, .. } if code == "package_approval.identity_changed"
        ),
        "updated package must require re-adoption, got {error}"
    );
}

/// Plan 061 task 11: a `replaces` edge is user-owned graph control — it must
/// be covered by the durable approval, and a committed replacement revokes
/// the replaced target's approval (stale-on-replacement).
#[test]
fn replacement_edge_requires_approval_and_stales_target_on_commit() {
    let mut service = service();
    install_and_authorize(
        &mut service,
        graph_fixture("@vendor/base", "base", serde_json::Map::new()),
        &[],
    );
    service.approve_package("@vendor/base", "test").unwrap();
    service.enable("@vendor/base").unwrap();

    install_and_authorize(
        &mut service,
        graph_fixture(
            "@vendor/repl",
            "repl",
            serde_json::Map::from_iter([("replaces".to_string(), json!(["@vendor/base"]))]),
        ),
        &[PackagePermission::PackageControl],
    );
    // Approval lacking the replacement edge never authorizes the replacement.
    let mut approval = approval_for("@vendor/repl", vec![]);
    approval.package = "@vendor/repl".to_string();
    approval.source = "@vendor/repl".to_string();
    approval.api_prefix = "repl".to_string();
    approval.relations.clear();
    approval.replacements.clear();
    service.record_package_approval(approval).unwrap();
    let error = service.enable("@vendor/repl").unwrap_err();
    assert!(
        matches!(
            error,
            PackageServiceError::AdoptionRequired { code, .. }
                if code == "package_approval.replacement_expansion"
        ),
        "unapproved replacement edge must fail closed, got {error}"
    );
    assert!(service.inspect("@vendor/base").unwrap().is_enabled);

    // Exact host-built approval covers the edge; commit withdraws the target
    // and revokes its durable approval.
    service.approve_package("@vendor/repl", "test").unwrap();
    service.enable("@vendor/repl").unwrap();
    assert!(!service.inspect("@vendor/base").unwrap().is_enabled);
    assert!(service.inspect("@vendor/repl").unwrap().is_enabled);
    assert_eq!(
        service.adoption_state("@vendor/base"),
        Some(clay::packages::service::AdoptionState::Revoked),
        "replaced target's approval must be revoked at commit"
    );
}

/// Plan 061 task 11: explicit rollback disables the replacement, re-adopts
/// the third-party target, and re-enables it — never an automatic reversal.
#[test]
fn rollback_replacement_restores_target() {
    let mut service = service();
    install_and_authorize(
        &mut service,
        graph_fixture("@vendor/base", "base", serde_json::Map::new()),
        &[],
    );
    service.approve_package("@vendor/base", "test").unwrap();
    service.enable("@vendor/base").unwrap();
    install_and_authorize(
        &mut service,
        graph_fixture(
            "@vendor/repl",
            "repl",
            serde_json::Map::from_iter([("replaces".to_string(), json!(["@vendor/base"]))]),
        ),
        &[PackagePermission::PackageControl],
    );
    service.approve_package("@vendor/repl", "test").unwrap();
    service.enable("@vendor/repl").unwrap();

    let rolled_back = service.rollback_replacement("@vendor/base").unwrap();
    assert_eq!(rolled_back, "@vendor/repl");
    assert!(service.inspect("@vendor/base").unwrap().is_enabled);
    assert!(!service.inspect("@vendor/repl").unwrap().is_enabled);
    assert_eq!(
        service.adoption_state("@vendor/base"),
        Some(clay::packages::service::AdoptionState::Approved),
        "rollback must re-adopt the restored target"
    );
    // Second rollback without an active replacement fails closed.
    let error = service.rollback_replacement("@vendor/base").unwrap_err();
    assert!(matches!(
        error,
        PackageServiceError::NoActiveReplacement { .. }
    ));
}

/// Plan 061 task 11: Clay core/bootstrap is not package-managed — a
/// `replaces` edge naming it can never resolve, let alone execute.
#[test]
fn replacement_cannot_target_clay_core() {
    let mut service = service();
    install_and_authorize(
        &mut service,
        graph_fixture(
            "@vendor/repl",
            "repl",
            serde_json::Map::from_iter([("replaces".to_string(), json!(["core.text"]))]),
        ),
        &[PackagePermission::PackageControl],
    );
    service.approve_package("@vendor/repl", "test").unwrap();
    let error = service.enable("@vendor/repl").unwrap_err();
    assert!(
        matches!(error, PackageServiceError::MissingGraphTarget { .. }),
        "core/bootstrap targets must be unresolvable, got {error}"
    );
}

/// Plan 061 task 11: a dependency edge must not silently re-enable a replaced
/// target; restoration is the explicit user rollback path.
#[test]
fn dependency_cannot_silently_restore_replaced_target() {
    let mut service = service();
    install_and_authorize(
        &mut service,
        graph_fixture("@vendor/base", "base", serde_json::Map::new()),
        &[],
    );
    service.approve_package("@vendor/base", "test").unwrap();
    service.enable("@vendor/base").unwrap();
    install_and_authorize(
        &mut service,
        graph_fixture(
            "@vendor/repl",
            "repl",
            serde_json::Map::from_iter([("replaces".to_string(), json!(["@vendor/base"]))]),
        ),
        &[PackagePermission::PackageControl],
    );
    service.approve_package("@vendor/repl", "test").unwrap();
    service.enable("@vendor/repl").unwrap();
    assert!(!service.inspect("@vendor/base").unwrap().is_enabled);

    install_and_authorize(
        &mut service,
        graph_fixture(
            "@vendor/dep",
            "dep",
            serde_json::Map::from_iter([("dependsOn".to_string(), json!(["@vendor/base"]))]),
        ),
        &[],
    );
    service.approve_package("@vendor/dep", "test").unwrap();
    let error = service.enable("@vendor/dep").unwrap_err();
    assert!(
        matches!(
            error,
            PackageServiceError::RelationDenied { code, .. }
                if code == "package_replacement.target_replaced"
        ),
        "dependency on a replaced target must fail closed, got {error}"
    );
    assert!(!service.inspect("@vendor/base").unwrap().is_enabled);
}
