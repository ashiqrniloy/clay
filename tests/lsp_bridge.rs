use std::fs;
use std::path::Path;
use std::process::Command;

use clay::packages::{
    manager::FakeBackend, permissions::PackagePermission, record::assemble_package_record,
    service::PackageService,
};
use clay::protocol::language_intelligence::LanguageIntelligenceFeature;

const SHARED_FILES: &[&str] = &[
    "utf8.js",
    "framing.js",
    "positions.js",
    "mapping.js",
    "client.js",
    "typescript-language-server.js",
];
const BRIDGE_PACKAGES: &[&str] = &[
    "lsp-rust",
    "lsp-typescript",
    "lsp-javascript",
    "lsp-markdown",
];

fn run_node(arguments: &[&str]) {
    let output = Command::new("node")
        .args(arguments)
        .output()
        .expect("Node.js is required for package adapter tests");
    assert!(
        output.status.success(),
        "node {} failed\nstdout:\n{}\nstderr:\n{}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn shared_lsp_adapter_protocol_suite_passes() {
    run_node(&["--test", "packages/lsp-shared/adapter.test.mjs"]);
}

#[test]
fn generic_fake_server_fixture_and_package_matrix_pass() {
    run_node(&[
        "--test",
        "tests/fixtures/lsp/fake-server/fake-server.test.mjs",
        "tests/fixtures/lsp/fake-server/matrix.test.mjs",
    ]);
    assert!(Path::new("tests/fixtures/lsp/fake-server/server.mjs").is_file());
    assert!(Path::new("tests/fixtures/lsp/fake-server/profiles.mjs").is_file());
    assert!(Path::new("tests/fixtures/lsp/fake-server/session.mjs").is_file());
    assert!(Path::new("tests/fixtures/lsp/workspaces/README.md").is_file());
}

#[test]
fn rust_bridge_package_suite_passes() {
    run_node(&[
        "--test",
        "packages/lsp-rust/rust-package.test.mjs",
        "packages/lsp-rust/rust-real-smoke.test.mjs",
    ]);
}

#[test]
fn typescript_javascript_bridge_package_suite_passes() {
    run_node(&[
        "--test",
        "packages/lsp-typescript/typescript-package.test.mjs",
        "packages/lsp-typescript/typescript-real-smoke.test.mjs",
        "packages/lsp-javascript/javascript-package.test.mjs",
        "packages/lsp-javascript/javascript-real-smoke.test.mjs",
    ]);
}

#[test]
fn markdown_bridge_package_suite_passes() {
    run_node(&[
        "--test",
        "packages/lsp-markdown/markdown-package.test.mjs",
        "packages/lsp-markdown/markdown-real-smoke.test.mjs",
    ]);
}

#[test]
fn markdown_bridge_manifest_is_fixed_opt_in_and_load_tolerates_missing_grant() {
    let manifest_text = fs::read_to_string("packages/lsp-markdown/package.json").unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text).unwrap();
    let record = assemble_package_record(&manifest).expect("@clay/lsp-markdown manifest validates");
    let descriptor = &record.contributions.language_servers[0];
    assert_eq!(record.manifest.name, "@clay/lsp-markdown");
    assert_eq!(record.manifest.clay.modes, Vec::<String>::new());
    assert_eq!(descriptor.id, "lsp-markdown.server");
    assert_eq!(descriptor.executable, "marksman");
    assert_eq!(descriptor.args, ["server"]);
    assert!(descriptor.inherit_environment.is_empty());
    assert_eq!(
        record.contributions.language_intelligence_providers[0].modes,
        ["markdown"]
    );
    assert_eq!(
        record.contributions.language_intelligence_providers[0].features,
        [
            LanguageIntelligenceFeature::Hover,
            LanguageIntelligenceFeature::GoToDefinition,
            LanguageIntelligenceFeature::CodeAction,
        ]
    );
    assert_eq!(record.contributions.completion_providers[0].priority, 100);
    assert!(!record.contributions.completion_providers[0].exclusive);
    assert!(
        record
            .manifest
            .clay
            .permissions
            .contains(&PackagePermission::LanguageServer)
    );

    let root =
        std::env::temp_dir().join(format!("clay-lsp-markdown-no-grant-{}", std::process::id()));
    let mut service = PackageService::new(root, Box::new(FakeBackend::new()));
    service
        .install_from_value_at_root(manifest, "packages/lsp-markdown".into())
        .unwrap();
    service
        .authorize_bundled_defaults("@clay/lsp-markdown", "test")
        .unwrap();
    service
        .approve_package("@clay/lsp-markdown", "test")
        .unwrap();
    // Phase 24.5 decision (2026-08-13-2223): the missing language-server
    // grant is tolerated at load; the capability stays inert because the
    // grant store is empty (session start remains grant-gated).
    service
        .enable("@clay/lsp-markdown")
        .expect("missing language-server grant is tolerated at load");
    assert!(
        service
            .language_server_grant("@clay/lsp-markdown", "lsp-markdown.server")
            .is_none(),
        "bundled defaults must not auto-grant language-server authority"
    );

    let load = fs::read_to_string("packages/lsp-markdown/dist/load.js").unwrap();
    assert!(load.contains("serverRegisterDocumentAnalyzer"));
    assert!(!load.contains("authorizeLanguageServer"));
    assert!(Path::new("tests/fixtures/lsp/markdown/.marksman.toml").is_file());
    assert!(Path::new("tests/fixtures/lsp/markdown/README.md").is_file());
    assert!(Path::new("tests/fixtures/lsp/markdown/other.md").is_file());
    assert!(Path::new("tests/fixtures/lsp/markdown/broken.md").is_file());
}

#[test]
fn typescript_javascript_bridge_manifests_are_fixed_opt_in_mode_separated_and_load_tolerates_missing_grant()
 {
    for (package, contribution, mode, fixture_root, fixture_files) in [
        (
            "lsp-typescript",
            "lsp-typescript.server",
            "typescript",
            "tests/fixtures/lsp/typescript",
            &["tsconfig.json", "src/main.ts", "src/badge.tsx"][..],
        ),
        (
            "lsp-javascript",
            "lsp-javascript.server",
            "javascript",
            "tests/fixtures/lsp/javascript",
            &["jsconfig.json", "src/main.js", "src/badge.jsx"][..],
        ),
    ] {
        let manifest_text = fs::read_to_string(format!("packages/{package}/package.json")).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&manifest_text).unwrap();
        let record = assemble_package_record(&manifest)
            .unwrap_or_else(|error| panic!("@clay/{package} manifest validates: {error:?}"));
        let descriptor = &record.contributions.language_servers[0];
        assert_eq!(record.manifest.name, format!("@clay/{package}"));
        assert_eq!(record.manifest.clay.modes, Vec::<String>::new());
        assert_eq!(descriptor.id, contribution);
        assert_eq!(descriptor.executable, "typescript-language-server");
        assert_eq!(descriptor.args, ["--stdio"]);
        assert_eq!(descriptor.inherit_environment, ["HOME", "PATH"]);
        assert_eq!(
            record.contributions.language_intelligence_providers[0].modes,
            [mode]
        );
        assert_eq!(record.contributions.completion_providers[0].priority, 100);
        assert!(!record.contributions.completion_providers[0].exclusive);
        assert!(
            record
                .manifest
                .clay
                .permissions
                .contains(&PackagePermission::LanguageServer)
        );

        let root =
            std::env::temp_dir().join(format!("clay-{package}-no-grant-{}", std::process::id()));
        let mut service = PackageService::new(root, Box::new(FakeBackend::new()));
        service
            .install_from_value_at_root(manifest, format!("packages/{package}").into())
            .unwrap();
        service
            .authorize_bundled_defaults(&format!("@clay/{package}"), "test")
            .unwrap();
        service
            .approve_package(&format!("@clay/{package}"), "test")
            .unwrap();
        // Phase 24.5 decision (2026-08-13-2223): tolerated at load, inert
        // without a grant in the store.
        service
            .enable(&format!("@clay/{package}"))
            .expect("missing language-server grant is tolerated at load");
        assert!(
            service
                .language_server_grant(&format!("@clay/{package}"), &format!("{package}.server"))
                .is_none(),
            "bundled defaults must not auto-grant language-server authority"
        );

        let load = fs::read_to_string(format!("packages/{package}/dist/load.js")).unwrap();
        assert!(load.contains("serverRegisterDocumentAnalyzer"));
        assert!(!load.contains("authorizeLanguageServer"));
        let shared_policy = fs::read_to_string(format!(
            "packages/{package}/dist/shared/typescript-language-server.js"
        ))
        .unwrap();
        let canonical =
            fs::read_to_string("packages/lsp-shared/typescript-language-server.js").unwrap();
        assert_eq!(shared_policy, canonical);
        for file in fixture_files {
            assert!(
                Path::new(fixture_root).join(file).is_file(),
                "missing fixture {fixture_root}/{file}"
            );
        }
    }
}

#[test]
fn rust_bridge_manifest_is_fixed_opt_in_and_load_tolerates_missing_grant() {
    let manifest_text = fs::read_to_string("packages/lsp-rust/package.json").unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text).unwrap();
    let record = assemble_package_record(&manifest).expect("@clay/lsp-rust manifest validates");
    let descriptor = &record.contributions.language_servers[0];
    assert_eq!(record.manifest.name, "@clay/lsp-rust");
    assert_eq!(record.manifest.clay.modes, Vec::<String>::new());
    assert_eq!(descriptor.id, "lsp-rust.server");
    assert_eq!(descriptor.executable, "rustup");
    assert_eq!(descriptor.args, ["run", "stable", "rust-analyzer"]);
    assert_eq!(descriptor.inherit_environment, ["HOME", "PATH"]);
    assert_eq!(record.contributions.completion_providers[0].priority, 100);
    assert!(!record.contributions.completion_providers[0].exclusive);
    assert!(
        record
            .manifest
            .clay
            .permissions
            .contains(&PackagePermission::LanguageServer)
    );

    let root = std::env::temp_dir().join(format!("clay-lsp-rust-no-grant-{}", std::process::id()));
    let mut service = PackageService::new(root, Box::new(FakeBackend::new()));
    service
        .install_from_value_at_root(manifest, "packages/lsp-rust".into())
        .unwrap();
    service
        .authorize_bundled_defaults("@clay/lsp-rust", "test")
        .unwrap();
    service.approve_package("@clay/lsp-rust", "test").unwrap();
    // Phase 24.5 decision (2026-08-13-2223): tolerated at load, inert
    // without a grant in the store.
    service
        .enable("@clay/lsp-rust")
        .expect("missing language-server grant is tolerated at load");
    assert!(
        service
            .language_server_grant("@clay/lsp-rust", "lsp-rust.server")
            .is_none(),
        "bundled defaults must not auto-grant language-server authority"
    );

    let load = fs::read_to_string("packages/lsp-rust/dist/load.js").unwrap();
    assert!(load.contains("serverRegisterDocumentAnalyzer"));
    assert!(!load.contains("authorizeLanguageServer"));
    assert!(Path::new("tests/fixtures/lsp/rust/Cargo.toml").is_file());
    assert!(Path::new("tests/fixtures/lsp/rust/src/main.rs").is_file());
}

#[test]
fn first_party_shared_adapter_copies_are_fresh() {
    for package in BRIDGE_PACKAGES {
        for file in SHARED_FILES {
            let canonical = fs::read(Path::new("packages/lsp-shared").join(file))
                .unwrap_or_else(|error| panic!("failed to read canonical {file}: {error}"));
            let target = Path::new("packages")
                .join(package)
                .join("dist/shared")
                .join(file);
            assert_eq!(
                fs::read(&target)
                    .unwrap_or_else(|error| panic!("failed to read {}: {error}", target.display())),
                canonical,
                "{} is stale; run node scripts/update-first-party-lsp-shared.mjs",
                target.display(),
            );
        }
    }
    run_node(&["scripts/update-first-party-lsp-shared.mjs", "--check"]);
}

#[test]
fn rust_core_remains_lsp_wire_neutral() {
    let forbidden = [
        "Content-Length",
        "jsonrpc",
        "textDocument/",
        "$/cancelRequest",
        "rust-analyzer",
        "typescript-language-server",
        "marksman",
    ];
    for path in [
        "src/server/document_analysis.rs",
        "src/server/connection.rs",
        "src/server/language_server.rs",
        "src/server/language_intelligence.rs",
        "src/server/completion.rs",
    ] {
        let source = fs::read_to_string(path).unwrap();
        for marker in forbidden {
            assert!(
                !source.contains(marker),
                "{path} must keep LSP wire/server marker `{marker}` package-side"
            );
        }
    }
}

#[test]
fn lsp_language_packages_fixture_grants_before_one_line_loads() {
    let fixture = fs::read_to_string("tests/fixtures/configuration/lsp-language-packages/init.js")
        .expect("lsp-language-packages fixture exists");
    assert!(
        !fixture.contains("pnpm")
            && !fixture.contains("npm install")
            && !fixture.contains("autoInstall"),
        "fixture must not auto-install language servers"
    );
    for package in [
        "@clay/lsp-rust",
        "@clay/lsp-typescript",
        "@clay/lsp-javascript",
        "@clay/lsp-markdown",
        "@clay/rust",
        "@clay/typescript",
        "@clay/javascript",
        "@clay/markdown",
    ] {
        assert!(
            fixture.contains(&format!("loadPackage(\"{package}\")")),
            "fixture must one-line load {package}"
        );
    }
    for (package, contribution) in [
        ("@clay/lsp-rust", "lsp-rust.server"),
        ("@clay/lsp-typescript", "lsp-typescript.server"),
        ("@clay/lsp-javascript", "lsp-javascript.server"),
        ("@clay/lsp-markdown", "lsp-markdown.server"),
    ] {
        assert!(
            fixture.contains(package) && fixture.contains(contribution),
            "fixture must authorize {package}/{contribution} before load"
        );
    }

    let first_load = fixture
        .find("await loadPackage")
        .expect("loadPackage present");
    let last_grant = fixture
        .rmatch_indices("await authorizeLanguageServer")
        .next()
        .expect("authorizeLanguageServer present")
        .0;
    assert!(
        last_grant < first_load,
        "every authorizeLanguageServer call must precede the first loadPackage"
    );

    for package in BRIDGE_PACKAGES {
        let docs = fs::read_to_string(format!("packages/{package}/docs/index.md")).unwrap();
        assert!(
            docs.contains("authorizeLanguageServer") && docs.contains("loadPackage"),
            "{package} docs must document grant-before-load"
        );
        assert!(
            docs.contains("serverDisableCompletion") || docs.contains("non-exclusive"),
            "{package} docs must document completion precedence/fallback"
        );
    }
}
