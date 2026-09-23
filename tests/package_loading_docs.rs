use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    fs::read_to_string(root().join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn contracts() -> serde_json::Value {
    serde_json::from_str(&read("docs/reference/documentation-contracts.json"))
        .expect("parse documentation contracts")
}

fn package_contracts() -> Vec<serde_json::Value> {
    contracts()
        .get("package_documents")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .expect("documentation-contracts.json missing package_documents")
}

fn required<'a>(entry: &'a serde_json::Value, field: &str) -> &'a str {
    entry
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("package document contract missing {field}: {entry}"))
}

fn api_inventory_ids() -> BTreeSet<String> {
    read("docs/reference/clay-js-api/api-inventory.toml")
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("id = \"")
                .and_then(|value| value.strip_suffix('"'))
        })
        .map(ToOwned::to_owned)
        .collect()
}

#[test]
fn package_reference_docs_match_structured_manifest_contracts() {
    let mut package_names = BTreeSet::new();
    for entry in package_contracts() {
        let id = required(&entry, "id");
        let path = required(&entry, "path");
        let text = read(path);
        assert!(
            text.starts_with("# "),
            "{path}: package document must start with H1"
        );
        assert!(
            text.contains("\n## "),
            "{path}: package document must have sections"
        );

        let Some(manifest_path) = entry.get("manifest").and_then(serde_json::Value::as_str) else {
            assert_eq!(
                id, "creating-packages",
                "{path}: only package author guide may omit manifest binding"
            );
            continue;
        };
        let expected_name = required(&entry, "package_name");
        assert!(
            package_names.insert(expected_name.to_string()),
            "duplicate documented package {expected_name}"
        );
        let manifest: serde_json::Value = serde_json::from_str(&read(manifest_path))
            .unwrap_or_else(|error| panic!("{path}: parse {manifest_path}: {error}"));
        assert_eq!(
            manifest.get("name").and_then(serde_json::Value::as_str),
            Some(expected_name),
            "{path}: package_name differs from {manifest_path}"
        );
        assert!(
            text.contains(expected_name),
            "{path}: must identify manifest package {expected_name}"
        );

        let clay = manifest
            .get("clay")
            .unwrap_or_else(|| panic!("{manifest_path}: missing clay metadata"));
        let api_prefix = clay
            .get("apiPrefix")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{manifest_path}: missing clay.apiPrefix"));
        let load_entry = clay
            .get("loadEntry")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{manifest_path}: missing clay.loadEntry"));
        assert!(
            !api_prefix.is_empty(),
            "{manifest_path}: empty clay.apiPrefix"
        );
        assert!(
            root()
                .join(manifest_path)
                .parent()
                .unwrap()
                .join(load_entry)
                .is_file(),
            "{manifest_path}: clay.loadEntry does not exist: {load_entry}"
        );

        if let Some(role) = clay
            .get("contributions")
            .and_then(|value| value.get("editor"))
            .and_then(|value| value.get("defaultFontRole"))
            .and_then(serde_json::Value::as_str)
        {
            assert!(
                text.contains(&format!("defaultFontRole: \"{role}\"")),
                "{path}: defaultFontRole differs from {manifest_path}"
            );
        }
    }
}

#[test]
fn package_author_guide_uses_public_facades_not_raw_ops() {
    let guide = read("docs/reference/packages/creating-packages.md");
    let api_ids = api_inventory_ids();
    assert!(api_ids.contains("packages.loadPackage"));
    assert!(guide.contains("`packages.loadPackage`"));
    assert!(
        guide.contains("execute-only"),
        "package author guide must document execute-only load entries"
    );
    for preset in ["code-mode", "prose-mode", "lsp-bridge"] {
        assert!(
            guide.contains(preset),
            "package author guide must document preset `{preset}`"
        );
    }
    assert!(
        guide.contains("createLspBridge") && guide.contains("lsp-shared/bridge.js"),
        "package author guide must document the shared LSP factory"
    );
    assert!(
        guide.contains("clay package inspect"),
        "package author guide must document CLI inspect"
    );
    assert!(
        guide.contains("raw `Deno.core.ops`"),
        "package author guide must retain raw-op boundary marker"
    );
}

#[test]
fn language_package_docs_have_no_hidden_configuration_surface() {
    let inventory = read("docs/reference/clay-js-api/api-inventory.toml");
    let package_names: BTreeSet<_> = package_contracts()
        .into_iter()
        .filter_map(|entry| {
            entry
                .get("package_name")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect();
    for name in package_names {
        let prefix = name.trim_start_matches("@clay/");
        assert!(
            !inventory.contains(&format!("configuration.{prefix}")),
            "{name}: package-specific hidden configuration API found; use generic documented APIs"
        );
    }
}

#[test]
fn package_document_contract_errors_name_path_and_field() {
    let broken = serde_json::json!({
        "id": "broken",
        "path": "docs/reference/packages/broken.md",
        "manifest": "packages/broken/package.json"
    });
    let error = std::panic::catch_unwind(|| required(&broken, "package_name"))
        .expect_err("missing package_name must fail");
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(message.contains("package_name") && message.contains("broken.md"));
}

#[test]
fn package_document_validation_is_read_only() {
    let contract_path = root().join("docs/reference/documentation-contracts.json");
    let manifests: Vec<_> = package_contracts()
        .into_iter()
        .filter_map(|entry| {
            entry
                .get("manifest")
                .and_then(serde_json::Value::as_str)
                .map(|path| root().join(path))
        })
        .collect();
    let before = (
        fs::read(&contract_path).unwrap(),
        manifests
            .iter()
            .map(|path| fs::read(path).unwrap())
            .collect::<Vec<_>>(),
    );
    let _ = package_contracts();
    let after = (
        fs::read(&contract_path).unwrap(),
        manifests
            .iter()
            .map(|path| fs::read(path).unwrap())
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        before, after,
        "package documentation validators must not mutate contracts or manifests"
    );
}
