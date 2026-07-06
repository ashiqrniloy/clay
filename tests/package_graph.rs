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
    install_and_authorize(&mut service, extension, &[]);

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
