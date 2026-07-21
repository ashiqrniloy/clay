use clay::packages::authorization::RuntimeProfile;
use clay::packages::conflict::{
    PackageConflictKind, PackageConflictResolutionReason, check_enabled_packages,
};
use clay::packages::manager::FakeBackend;
use clay::packages::permissions::PackagePermission;
use clay::packages::record::{PackageRecord, assemble_package_record};
use clay::packages::service::{PackageService, PackageServiceError};
use serde_json::{Value, json};

fn package_fixture(
    name: &str,
    prefix: &str,
    mode: &str,
    extra_clay: serde_json::Map<String, Value>,
) -> Value {
    let mut clay = serde_json::Map::from_iter([
        ("apiPrefix".to_string(), json!(prefix)),
        ("entry".to_string(), json!("./dist/index.js")),
        ("loadEntry".to_string(), json!("./dist/load.js")),
        (
            "permissions".to_string(),
            json!(["mode-registration", "mode-activation"]),
        ),
        ("modes".to_string(), json!([mode])),
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

fn keybinding_fixture(name: &str, prefix: &str, priority: i32) -> Value {
    package_fixture(
        name,
        prefix,
        prefix,
        serde_json::Map::from_iter([
            (
                "permissions".to_string(),
                json!([
                    "mode-registration",
                    "mode-activation",
                    "command-registration"
                ]),
            ),
            (
                "contributions".to_string(),
                json!({
                    "commands": [{
                        "id": format!("{prefix}.run"),
                        "displayName": "Run",
                        "routingPolicy": "server-first"
                    }],
                    "keyRouting": [{
                        "commandId": format!("{prefix}.run"),
                        "key": "Ctrl+K",
                        "routingPolicy": "server-first",
                        "priority": priority
                    }]
                }),
            ),
        ]),
    )
}

fn record(package_json: Value) -> PackageRecord {
    assemble_package_record(&package_json).expect("conflict fixture validates")
}

fn install_and_authorize(
    service: &mut PackageService,
    package_json: Value,
    extra_grants: &[PackagePermission],
) {
    let record = assemble_package_record(&package_json).expect("conflict fixture validates");
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
    service
        .approve_package(&record.manifest.name, "test")
        .expect("fixture adoption approval succeeds");
}

fn service() -> PackageService {
    PackageService::new(
        "target/test-package-conflicts",
        Box::<FakeBackend>::default(),
    )
}

#[test]
fn duplicate_mode_is_rejected_without_resolution_policy() {
    let first = record(package_fixture(
        "@vendor/first",
        "shared",
        "shared",
        serde_json::Map::new(),
    ));
    let second = record(package_fixture(
        "@vendor/second",
        "shared",
        "shared",
        serde_json::Map::new(),
    ));

    let err = check_enabled_packages([&first, &second]).unwrap_err();

    assert_eq!(err.kind, PackageConflictKind::DuplicateMode);
    assert_eq!(&*err.contribution_id, "shared");
    assert_eq!(err.first.package_name, "@vendor/first");
    assert_eq!(err.second.package_name, "@vendor/second");
}

#[test]
fn replacement_wins_with_package_control_grant_and_records_resolution() {
    let mut service = service();
    let target = package_fixture("@vendor/target", "shared", "shared", serde_json::Map::new());
    let replacement = package_fixture(
        "@vendor/replacement",
        "shared",
        "shared.replacement",
        serde_json::Map::from_iter([
            (
                "permissions".to_string(),
                json!(["mode-registration", "mode-activation"]),
            ),
            ("replaces".to_string(), json!(["@vendor/target"])),
        ]),
    );
    install_and_authorize(&mut service, target, &[]);
    install_and_authorize(
        &mut service,
        replacement,
        &[PackagePermission::PackageControl],
    );
    service
        .enable("@vendor/target")
        .expect("target enables before replacement");

    service
        .enable("@vendor/replacement")
        .expect("package-control replacement resolves duplicate mode");

    assert!(!service.inspect("@vendor/target").unwrap().is_enabled);
    assert!(service.inspect("@vendor/replacement").unwrap().is_enabled);
    let diagnostic = service
        .conflict_resolution_diagnostics()
        .last()
        .expect("replacement resolution diagnostic exists");
    assert_eq!(
        diagnostic.reason,
        PackageConflictResolutionReason::PackageReplaces
    );
    assert_eq!(diagnostic.winner.package_name, "@vendor/replacement");
    assert_eq!(diagnostic.loser.package_name, "@vendor/target");
    assert!(diagnostic.message.contains("package-control replaced"));
}

#[test]
fn user_conflict_override_selects_winner_without_package_control() {
    let mut service = service();
    let first = package_fixture("@vendor/first", "shared", "shared", serde_json::Map::new());
    let second = package_fixture(
        "@vendor/second",
        "shared",
        "shared.second",
        serde_json::Map::new(),
    );
    install_and_authorize(&mut service, first, &[]);
    install_and_authorize(&mut service, second, &[]);
    service.set_conflict_override("shared", "@vendor/second");
    service
        .enable("@vendor/first")
        .expect("first package enables");

    service
        .enable("@vendor/second")
        .expect("explicit user conflict override resolves duplicate mode");

    assert!(!service.inspect("@vendor/first").unwrap().is_enabled);
    assert!(service.inspect("@vendor/second").unwrap().is_enabled);
    let diagnostic = service
        .conflict_resolution_diagnostics()
        .last()
        .expect("user override diagnostic exists");
    assert_eq!(
        diagnostic.reason,
        PackageConflictResolutionReason::UserOverride
    );
    assert_eq!(diagnostic.winner.package_name, "@vendor/second");
    assert_eq!(diagnostic.loser.package_name, "@vendor/first");
}

#[test]
fn explicit_keybinding_priority_prevents_ambiguous_conflict() {
    let first = record(keybinding_fixture("@vendor/first", "first", 10));
    let second = record(keybinding_fixture("@vendor/second", "second", 20));

    check_enabled_packages([&first, &second])
        .expect("distinct explicit priorities avoid silent keybinding conflict");
}

#[test]
fn same_keybinding_priority_falls_back_to_deterministic_diagnostic() {
    let first = record(keybinding_fixture("@vendor/first", "first", 10));
    let second = record(keybinding_fixture("@vendor/second", "second", 10));

    let err = check_enabled_packages([&first, &second]).unwrap_err();

    assert_eq!(err.kind, PackageConflictKind::AmbiguousKeyBinding);
    assert_eq!(&*err.contribution_id, "Ctrl+K:server-first:10");
    assert!(err.message.contains("@vendor/first"));
    assert!(err.message.contains("@vendor/second"));
}

#[test]
fn package_cannot_replace_without_package_control_grant() {
    let mut service = service();
    let target = package_fixture("@vendor/target", "shared", "shared", serde_json::Map::new());
    let replacement = package_fixture(
        "@vendor/replacement",
        "shared",
        "shared.replacement",
        serde_json::Map::from_iter([("replaces".to_string(), json!(["@vendor/target"]))]),
    );
    install_and_authorize(&mut service, target, &[]);
    install_and_authorize(&mut service, replacement, &[]);
    service
        .enable("@vendor/target")
        .expect("target enables before replacement");

    let err = service.enable("@vendor/replacement").unwrap_err();

    match err {
        PackageServiceError::MissingPackageControlGrant { package_name } => {
            assert_eq!(package_name, "@vendor/replacement");
        }
        other => panic!("expected missing package-control grant, got {other:?}"),
    }
    assert!(service.inspect("@vendor/target").unwrap().is_enabled);
    assert!(!service.inspect("@vendor/replacement").unwrap().is_enabled);
}
