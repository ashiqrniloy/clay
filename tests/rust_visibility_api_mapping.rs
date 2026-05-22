use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
struct InventoryEntry {
    fields: BTreeMap<String, String>,
}

impl InventoryEntry {
    fn get(&self, key: &str) -> &str {
        self.fields.get(key).map(String::as_str).unwrap_or("")
    }
}

fn inventory_entries() -> Vec<InventoryEntry> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/reference/clay-js-api/api-inventory.toml"
    );
    let text = std::fs::read_to_string(path).expect("read api inventory");
    let mut entries = Vec::new();
    let mut current: Option<BTreeMap<String, String>> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[api]]" {
            if let Some(fields) = current.take() {
                entries.push(InventoryEntry { fields });
            }
            current = Some(BTreeMap::new());
            continue;
        }
        let Some((key, value)) = line.split_once(" = ") else {
            continue;
        };
        current
            .as_mut()
            .expect("inventory key/value appears inside an [[api]] table")
            .insert(key.to_string(), value.trim().trim_matches('"').to_string());
    }

    if let Some(fields) = current {
        entries.push(InventoryEntry { fields });
    }

    entries
}

fn inventory_rust_mapping_text() -> String {
    inventory_entries()
        .into_iter()
        .map(|entry| {
            format!(
                "{}\n{}\n{}\n{}",
                entry.get("backing_rust"),
                entry.get("current_rust_owner"),
                entry.get("deno_op"),
                entry.get("facade_path")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn public_items_in_dir(relative_dir: &str) -> Vec<String> {
    let source_dir = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), relative_dir);
    let mut items = Vec::new();

    for entry in
        std::fs::read_dir(&source_dir).unwrap_or_else(|err| panic!("read {relative_dir}: {err}"))
    {
        let entry = entry.expect("source dir entry");
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("utf-8 source file name")
            .to_string();
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let mut current_impl: Option<String> = None;
        let mut impl_brace_depth = 0isize;

        for line in source.lines() {
            let trimmed = line.trim();
            let starts_impl = trimmed.starts_with("impl ");
            if let Some(rest) = trimmed.strip_prefix("impl ") {
                current_impl = rest
                    .split(|character: char| character == '{' || character.is_whitespace())
                    .next()
                    .filter(|name| !name.is_empty())
                    .map(str::to_string);
                impl_brace_depth = 0;
            }

            if trimmed.starts_with("pub ") {
                let tokens: Vec<_> = trimmed
                    .split(|character: char| character.is_whitespace() || character == '(')
                    .filter(|token| !token.is_empty())
                    .collect();
                if let Some(kind_index) = tokens.iter().position(|token| {
                    matches!(
                        *token,
                        "struct" | "enum" | "trait" | "type" | "const" | "static" | "fn"
                    )
                }) {
                    if let Some(name) = tokens.get(kind_index + 1) {
                        let name = name.trim_end_matches(':');
                        let rust_path = match tokens[kind_index] {
                            "fn" => match &current_impl {
                                Some(owner) => {
                                    format!("{relative_dir}/{file_name}::{owner}::{name}")
                                }
                                None => format!("{relative_dir}/{file_name}::{name}"),
                            },
                            _ => format!("{relative_dir}/{file_name}::{name}"),
                        };
                        items.push(rust_path);
                    }
                }
            }

            if current_impl.is_some() {
                impl_brace_depth += trimmed.matches('{').count() as isize;
                impl_brace_depth -= trimmed.matches('}').count() as isize;
                if !starts_impl && impl_brace_depth <= 0 {
                    current_impl = None;
                }
            }
        }
    }

    items.sort();
    items
}

fn public_server_items() -> Vec<String> {
    public_items_in_dir("src/server")
}

fn public_docs_items() -> Vec<String> {
    public_items_in_dir("src/docs")
}

#[test]
fn server_public_items_have_api_inventory_entries_or_are_allowlisted() {
    let inventory_text = inventory_rust_mapping_text();
    let allowlisted_infrastructure: BTreeSet<&str> = [
        "src/server/mod.rs::IpcServer::new",
        "src/server/mod.rs::IpcServer::try_new",
        "src/server/mod.rs::IpcServer::run",
        "src/server/mod.rs::ServerConfig::new",
    ]
    .into_iter()
    .collect();

    let unmapped: Vec<_> = public_server_items()
        .into_iter()
        .filter(|item| !allowlisted_infrastructure.contains(item.as_str()))
        .filter(|item| !inventory_text.contains(item))
        .collect();

    assert!(
        unmapped.is_empty(),
        "public server Rust items must be either mapped in docs/reference/clay-js-api/api-inventory.toml or explicitly allowlisted as non-JS server infrastructure: {unmapped:?}"
    );
}

#[test]
fn docs_public_items_are_internal_registry_infrastructure() {
    let allowlisted_docs_infrastructure: BTreeSet<&str> = [
        "src/docs/registry.rs::GENERATED_REGISTRY_PATH",
        "src/docs/registry.rs::UPDATE_COMMAND",
        "src/docs/registry.rs::CustomProperty",
        "src/docs/registry.rs::RegistryEntry",
        "src/docs/registry.rs::ClayJsApiRegistry",
        "src/docs/registry.rs::RegistryError",
        "src/docs/registry.rs::RegistryResult<T>",
        "src/docs/registry.rs::ClayJsApiRegistry::from_docs",
        "src/docs/registry.rs::ClayJsApiRegistry::from_generated",
        "src/docs/registry.rs::ClayJsApiRegistry::from_generated_json",
        "src/docs/registry.rs::ClayJsApiRegistry::by_id",
        "src/docs/registry.rs::ClayJsApiRegistry::by_js_export",
        "src/docs/registry.rs::ClayJsApiRegistry::by_user_facing_name",
        "src/docs/registry.rs::ClayJsApiRegistry::by_kind_owner",
        "src/docs/registry.rs::ClayJsApiRegistry::by_lookup_tag",
        "src/docs/registry.rs::ClayJsApiRegistry::by_key_binding",
        "src/docs/registry.rs::ClayJsApiRegistry::by_custom_property",
        "src/docs/registry.rs::ClayJsApiRegistry::to_generated_json",
        "src/docs/registry.rs::repository_root",
        "src/docs/registry.rs::expected_generated_registry",
        "src/docs/registry.rs::update_generated_registry",
        "src/docs/registry.rs::check_generated_registry_current",
        "src/docs/registry.rs::registry_source_paths",
    ]
    .into_iter()
    .collect();

    let unclassified: Vec<_> = public_docs_items()
        .into_iter()
        .filter(|item| !allowlisted_docs_infrastructure.contains(item.as_str()))
        .collect();

    assert!(
        unclassified.is_empty(),
        "public src/docs Rust items must be classified as internal documentation-registry infrastructure or promoted through Clay JS API docs/inventory before becoming user-facing APIs: {unclassified:?}"
    );
}
