use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    fs::read_to_string(root().join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

fn documentation_contracts() -> serde_json::Value {
    serde_json::from_str(&read("docs/reference/documentation-contracts.json"))
        .expect("parse docs/reference/documentation-contracts.json")
}

fn contract_entries<'a>(contracts: &'a serde_json::Value, group: &str) -> &'a [serde_json::Value] {
    contracts
        .get(group)
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("documentation-contracts.json missing array {group}"))
}

fn required_string<'a>(entry: &'a serde_json::Value, field: &str, group: &str) -> &'a str {
    entry
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            panic!("documentation-contracts.json {group} entry missing {field}: {entry}")
        })
}

fn markdown_files(directory: &str) -> BTreeSet<String> {
    fs::read_dir(root().join(directory))
        .unwrap_or_else(|error| panic!("read {directory}: {error}"))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .map(|path| {
            path.strip_prefix(root())
                .expect("documentation path under repository")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

fn index_links(index_path: &str, document_path: &str) -> bool {
    let index = read(index_path);
    let relative = Path::new(document_path)
        .strip_prefix(Path::new(index_path).parent().unwrap_or(Path::new("")))
        .unwrap_or(Path::new(document_path))
        .to_string_lossy()
        .replace('\\', "/");
    index.contains(&format!("]({relative})")) || index.contains(&format!("]({document_path})"))
}

fn validate_security_markers(path: &str, text: &str, markers: &[serde_json::Value]) -> Vec<String> {
    markers
        .iter()
        .filter_map(|marker| marker.as_str())
        .filter(|marker| !text.contains(marker))
        .map(|marker| format!("{path}: missing security marker {marker:?}"))
        .collect()
}

#[test]
fn documentation_contract_inventory_is_complete_and_indexed() {
    let contracts = documentation_contracts();
    assert_eq!(
        contracts
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(1),
        "documentation-contracts.json schema_version must be 1"
    );
    assert!(
        read("docs/index.md").contains("reference/documentation-contracts.json"),
        "docs/index.md must link documentation-contracts.json"
    );

    for (group, directory) in [
        ("primitive_documents", "docs/reference/primitives"),
        ("package_documents", "docs/reference/packages"),
    ] {
        let entries = contract_entries(&contracts, group);
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for entry in entries {
            let id = required_string(entry, "id", group);
            let path = required_string(entry, "path", group);
            assert!(ids.insert(id), "{group}: duplicate id {id}");
            assert!(
                paths.insert(path.to_string()),
                "{group}: duplicate path {path}"
            );
            let text = read(path);
            assert!(
                text.starts_with("# "),
                "{path}: document must start with one H1 heading"
            );
            assert!(
                text.contains("\n## "),
                "{path}: document must contain at least one H2 section"
            );

            let indexes = entry
                .get("indexes")
                .and_then(serde_json::Value::as_array)
                .unwrap_or_else(|| panic!("{path}: missing indexes array"));
            assert!(!indexes.is_empty(), "{path}: indexes must not be empty");
            for index in indexes {
                let index = index
                    .as_str()
                    .unwrap_or_else(|| panic!("{path}: index path must be a string"));
                assert!(
                    index_links(index, path),
                    "{path}: not linked from required index {index}"
                );
            }
        }
        assert_eq!(
            paths,
            markdown_files(directory),
            "{group}: inventory must enumerate every Markdown file in {directory} exactly once"
        );
    }
}

#[test]
fn primitive_registry_matrix_has_one_complete_row_per_primitive() {
    let registry = read("docs/reference/primitives/registry.md");
    let matrix = registry
        .split_once("## Category Matrix")
        .map(|(_, rest)| rest)
        .expect("registry.md missing Category Matrix")
        .split_once("## Primitive Category Notes")
        .map(|(matrix, _)| matrix)
        .expect("registry.md missing Primitive Category Notes");
    let mut primitives = BTreeSet::new();
    let mut rows = 0;

    for line in matrix.lines().filter(|line| line.starts_with('|')) {
        let fields: Vec<_> = line.trim_matches('|').split('|').map(str::trim).collect();
        if fields.first() == Some(&"Primitive")
            || fields
                .iter()
                .all(|field| field.chars().all(|ch| ch == '-' || ch == ' '))
        {
            continue;
        }
        assert_eq!(fields.len(), 15, "registry.md malformed matrix row: {line}");
        let primitive = fields[0].trim_matches('`');
        assert!(
            primitives.insert(primitive.to_string()),
            "registry.md duplicate primitive row {primitive}"
        );
        for (index, field) in fields.iter().enumerate() {
            assert!(
                !field.is_empty(),
                "registry.md primitive {primitive} has empty column {index}"
            );
        }
        assert!(
            matches!(
                fields[14],
                "Exists" | "Extend" | "Exists/Extend" | "New" | "Deferred"
            ),
            "registry.md primitive {primitive} has invalid status {}",
            fields[14]
        );
        rows += 1;
    }
    assert!(
        rows >= 20,
        "registry.md unexpectedly contains only {rows} primitive rows"
    );
}

#[test]
fn narrow_security_markers_are_present_with_actionable_paths() {
    let contracts = documentation_contracts();
    for entry in contract_entries(&contracts, "security_contracts") {
        let path = required_string(entry, "path", "security_contracts");
        let markers = entry
            .get("markers")
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("{path}: missing markers array"));
        assert!(
            !markers.is_empty(),
            "{path}: security marker set must not be empty"
        );
        let errors = validate_security_markers(path, &read(path), markers);
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }

    let errors = validate_security_markers(
        "docs/example.md",
        "# Example\n\nHarmless prose.\n",
        &[serde_json::Value::String("trusted boundary".to_string())],
    );
    assert_eq!(
        errors,
        ["docs/example.md: missing security marker \"trusted boundary\""]
    );
}

#[test]
fn semantically_harmless_prose_is_not_a_contract() {
    let markers = [serde_json::Value::String(
        "authority remains host-owned".to_string(),
    )];
    let first =
        "# Page\n\n## Security\n\nauthority remains host-owned\n\nOriginal explanatory prose.";
    let rewritten = "# Page\n\n## Security\n\nauthority remains host-owned\n\nCompletely rewritten explanation.";
    assert!(validate_security_markers("page.md", first, &markers).is_empty());
    assert!(validate_security_markers("page.md", rewritten, &markers).is_empty());
}

#[test]
fn wiki_index_links_every_wiki_page() {
    fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        {
            let path = entry.expect("read wiki entry").path();
            if path.is_dir() {
                collect(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                files.push(path);
            }
        }
    }

    let wiki_root = root().join("docs/wiki");
    let index = read("docs/wiki/index.md");
    let mut files = Vec::new();
    collect(&wiki_root, &mut files);
    for path in files {
        if path == wiki_root.join("index.md") {
            continue;
        }
        let relative = path
            .strip_prefix(&wiki_root)
            .expect("wiki page under wiki root")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            index.contains(&format!("]({relative})")),
            "docs/wiki/index.md missing link to {relative}"
        );
    }
}

#[test]
fn plan061_runtime_package_authority_rebaseline_matches_source_inventory() {
    fn marked_section<'a>(text: &'a str, name: &str) -> &'a str {
        let start = format!("<!-- plan061-task1-{name}:start -->");
        let end = format!("<!-- plan061-task1-{name}:end -->");
        text.split_once(&start)
            .and_then(|(_, remaining)| remaining.split_once(&end))
            .map(|(section, _)| section)
            .unwrap_or_else(|| panic!("Plan 061 must contain {name} inventory markers"))
    }

    fn assert_exact_inventory(section: &str, values: &BTreeSet<String>, expected: usize) {
        assert_eq!(values.len(), expected, "unexpected source inventory count");
        for value in values {
            let token = format!("`{value}`");
            assert_eq!(
                section.matches(&token).count(),
                1,
                "Plan 061 inventory must classify {token} exactly once"
            );
        }
    }

    let plan = read("plans/061-Two-Package-Runtime-Trust-Domains-and-Extension-Authority.md");
    let ops_source = read("src/server/ops/mod.rs");
    let mut ops = BTreeSet::new();
    for extension_name in [
        "extension!(\n    clay_runtime_trusted_extension,",
        "extension!(\n    clay_runtime_package_extension,",
    ] {
        let body = ops_source
            .split_once(extension_name)
            .and_then(|(_, remaining)| remaining.split_once("\n);").map(|(body, _)| body))
            .unwrap_or_else(|| panic!("find {extension_name} op list"));
        for line in body.lines().map(str::trim) {
            if let Some(name) = line
                .strip_suffix(',')
                .filter(|name| name.starts_with("op_clay_"))
            {
                ops.insert(name.to_string());
            }
        }
    }
    assert_exact_inventory(marked_section(&plan, "op-inventory"), &ops, 67);

    let facades = read("src/server/facades.rs")
        .lines()
        .filter_map(|line| line.split_once("\"clay:").map(|(_, rest)| rest))
        .filter_map(|rest| rest.split_once('"').map(|(name, _)| format!("clay:{name}")))
        .collect::<BTreeSet<_>>();
    assert_exact_inventory(marked_section(&plan, "facade-inventory"), &facades, 21);

    let mut packages = BTreeSet::new();
    for entry in fs::read_dir(root().join("packages")).expect("read packages directory") {
        let package_json = entry
            .expect("read package entry")
            .path()
            .join("package.json");
        if !package_json.is_file() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(package_json).expect("read package manifest"))
                .expect("parse package manifest");
        if value.get("clay").is_some()
            && let Some(name) = value.get("name").and_then(serde_json::Value::as_str)
        {
            packages.insert(name.to_string());
        }
    }
    let package_section = marked_section(&plan, "package-inventory");
    assert_exact_inventory(package_section, &packages, 11);
    assert_eq!(package_section.matches("`packages/lsp-shared`").count(), 1);
}

/// Every RustSec exception ignored by cargo-audit must be documented with one
/// unexpired owner-reviewed expiry. CI invokes this test by name.
#[test]
fn audit_exceptions_are_documented_and_unexpired() {
    let audit_toml = read(".cargo/audit.toml");
    let security_doc = read("docs/development/security.md");
    let ignored: Vec<_> = audit_toml
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix('"')
                .map(|rest| rest.trim_end_matches(',').trim_end_matches('"'))
        })
        .filter(|id| id.starts_with("RUSTSEC-"))
        .collect();
    assert!(
        !ignored.is_empty(),
        "audit.toml must list ignored advisories explicitly"
    );
    for id in &ignored {
        assert!(
            security_doc.contains(id),
            "ignored advisory {id} missing from docs/development/security.md"
        );
    }

    let today = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .expect("date command available on Linux");
    let today = String::from_utf8(today.stdout)
        .expect("UTF-8 date")
        .trim()
        .to_string();
    let expiries: Vec<_> = security_doc
        .lines()
        .filter_map(|line| line.split_once("**Expiry:**").map(|(_, rest)| rest))
        .map(|rest| {
            rest.trim()
                .chars()
                .take_while(|ch| ch.is_ascii_digit() || *ch == '-')
                .collect::<String>()
        })
        .collect();
    assert_eq!(
        expiries.len(),
        ignored.len(),
        "each ignored advisory needs exactly one expiry"
    );
    for expiry in expiries {
        assert_eq!(
            expiry.len(),
            10,
            "expiry must be YYYY-MM-DD, got {expiry:?}"
        );
        assert!(
            expiry > today,
            "audit exception expired on {expiry} (today {today})"
        );
    }
}

#[test]
fn documentation_validators_do_not_mutate_files() {
    let contract_path = root().join("docs/reference/documentation-contracts.json");
    let registry_path = root().join("docs/generated/clay-js-api-registry.json");
    let before = (
        fs::read(&contract_path).unwrap(),
        fs::read(&registry_path).unwrap(),
    );
    let _ = documentation_contracts();
    let _ = markdown_files("docs/reference/primitives");
    let after = (
        fs::read(&contract_path).unwrap(),
        fs::read(&registry_path).unwrap(),
    );
    assert_eq!(
        before, after,
        "documentation tests must not mutate source or generated artifacts"
    );
}
