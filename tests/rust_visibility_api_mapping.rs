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

fn public_server_items() -> Vec<String> {
    let server_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/server");
    let mut items = Vec::new();

    for entry in std::fs::read_dir(server_dir).expect("read src/server") {
        let entry = entry.expect("server dir entry");
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("utf-8 server file name")
            .to_string();
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let mut current_impl: Option<String> = None;

        for line in source.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("impl ") {
                current_impl = rest
                    .split(|character: char| character == '{' || character.is_whitespace())
                    .next()
                    .filter(|name| !name.is_empty())
                    .map(str::to_string);
            }

            let public_decl = trimmed.starts_with("pub ");
            if !public_decl {
                continue;
            }

            let tokens: Vec<_> = trimmed
                .split(|character: char| character.is_whitespace() || character == '(')
                .filter(|token| !token.is_empty())
                .collect();
            let Some(kind_index) = tokens.iter().position(|token| {
                matches!(
                    *token,
                    "struct" | "enum" | "trait" | "type" | "const" | "static" | "fn"
                )
            }) else {
                continue;
            };
            let Some(name) = tokens.get(kind_index + 1) else {
                continue;
            };
            let rust_path = match tokens[kind_index] {
                "fn" => match &current_impl {
                    Some(owner) => format!("src/server/{file_name}::{owner}::{name}"),
                    None => format!("src/server/{file_name}::{name}"),
                },
                _ => format!("src/server/{file_name}::{name}"),
            };
            items.push(rust_path);
        }
    }

    items.sort();
    items
}

#[test]
fn server_public_items_have_api_inventory_entries_or_are_allowlisted() {
    let inventory_text = inventory_rust_mapping_text();
    let allowlisted_infrastructure: BTreeSet<&str> = [
        "src/server/mod.rs::IpcServer::new",
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
