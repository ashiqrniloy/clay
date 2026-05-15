use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug)]
struct InventoryEntry {
    fields: BTreeMap<String, String>,
}

impl InventoryEntry {
    fn get(&self, key: &str) -> &str {
        self.fields.get(key).map(String::as_str).unwrap_or("")
    }

    fn has_key(&self, key: &str) -> bool {
        self.fields.contains_key(key)
    }

    fn is_public_registry_api(&self) -> bool {
        self.get("registry_public") == "true"
    }
}

fn inventory_entries() -> Vec<InventoryEntry> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/reference/clay-js-api/api-inventory.toml"
    );
    let text = fs::read_to_string(path).expect("read api inventory");
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
        let fields = current
            .as_mut()
            .expect("inventory key/value appears inside an [[api]] table");
        fields.insert(key.to_string(), value.trim().trim_matches('"').to_string());
    }

    if let Some(fields) = current {
        entries.push(InventoryEntry { fields });
    }

    assert!(!entries.is_empty(), "inventory must contain API entries");
    entries
}

fn public_inventory_entries() -> Vec<InventoryEntry> {
    inventory_entries()
        .into_iter()
        .filter(InventoryEntry::is_public_registry_api)
        .collect()
}

fn markdown_frontmatter(path: &Path) -> BTreeMap<String, String> {
    let text = fs::read_to_string(path).unwrap_or_else(|err| panic!("read {path:?}: {err}"));
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("---"),
        "{path:?} must start with YAML frontmatter"
    );

    let mut fields = BTreeMap::new();
    for line in lines.by_ref() {
        if line == "---" {
            return fields;
        }
        if line.starts_with("  - ") || line.starts_with("    ") {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            fields.insert(key.to_string(), value.trim().trim_matches('"').to_string());
        }
    }
    panic!("{path:?} is missing closing frontmatter delimiter");
}

fn docs_index_registry_links() -> BTreeSet<String> {
    let index_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/index.md");
    let text = fs::read_to_string(&index_path).expect("read docs/index.md");
    let section = text
        .split("## Clay JS API Registry Source Files")
        .nth(1)
        .expect("docs/index.md has registry source section")
        .split("## Registry Rules")
        .next()
        .expect("docs/index.md has registry rules section");

    section
        .lines()
        .filter_map(|line| {
            line.split_once("](")
                .and_then(|(_, rest)| rest.split_once(')'))
        })
        .map(|(path, _)| format!("docs/{path}"))
        .collect()
}

fn parse_toml_string_list(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed == "[]" || !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Vec::new();
    }

    trimmed
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|item| item.trim().trim_matches('"'))
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn inventory_custom_property_names(value: &str) -> Vec<String> {
    parse_toml_string_list(value)
        .into_iter()
        .filter_map(|property| {
            property
                .split_once(':')
                .map(|(name, _)| name.to_string())
                .or_else(|| property.split_once('=').map(|(name, _)| name.to_string()))
        })
        .collect()
}

fn is_lower_camel_case(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_lowercase())
        && chars.all(|ch| ch.is_ascii_alphanumeric())
        && !name.contains('_')
}

fn facade_exports_function(facade_path: &str, export_name: &str) -> bool {
    let Some((path, symbol)) = facade_path.split_once("::") else {
        return false;
    };
    if symbol != export_name {
        return false;
    }
    let source_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|err| panic!("read facade source {source_path:?}: {err}"));
    source.contains(&format!("export function {export_name}"))
        || source.contains(&format!("export async function {export_name}"))
}

#[test]
fn api_inventory_has_required_fields() {
    let entries = inventory_entries();
    let required_fields = [
        "id",
        "category",
        "visibility",
        "status",
        "js_module",
        "js_export",
        "user_facing_name",
        "authority",
        "runtime_path",
        "hot_path_policy",
        "facade_path",
        "backing_rust",
        "deno_op",
        "deno_op_path",
        "documentation_path",
        "key_bindings",
        "custom_properties",
        "permissions",
        "security_notes",
        "current_rust_owner",
        "registry_public",
    ];

    let mut ids = BTreeSet::new();
    for entry in &entries {
        let id = entry.get("id");
        assert!(!id.is_empty(), "inventory entry is missing id: {entry:?}");
        assert!(ids.insert(id.to_string()), "duplicate inventory id {id}");

        for field in required_fields {
            assert!(
                entry.has_key(field),
                "{id} is missing required field {field}"
            );
        }

        if entry.is_public_registry_api() {
            for field in required_fields {
                let value = entry.get(field);
                let may_be_empty_list =
                    matches!(field, "key_bindings" | "custom_properties" | "permissions");
                assert!(
                    may_be_empty_list || !value.trim().is_empty(),
                    "public inventory entry {id} has empty required field {field}"
                );
            }
            assert!(
                entry.get("id").starts_with("clay."),
                "public inventory entry {id} must use the clay.* stable ID namespace"
            );
            assert!(
                entry
                    .get("documentation_path")
                    .starts_with("docs/reference/clay-js-api/"),
                "public inventory entry {id} must point at Clay JS API reference docs"
            );
            assert!(
                entry
                    .get("security_notes")
                    .contains("does not grant filesystem"),
                "public inventory entry {id} must explicitly state authority not granted"
            );
        }
    }
}

#[test]
fn api_inventory_classifies_current_editor_behavior() {
    let entries = inventory_entries();
    let categories: BTreeSet<_> = entries
        .iter()
        .filter(|entry| entry.is_public_registry_api())
        .map(|entry| entry.get("category").to_string())
        .collect();
    let required_categories = [
        "text-insertion",
        "newline",
        "backspace-delete",
        "cursor-movement",
        "selection",
        "scrolling",
        "resize-viewport",
        "cursor-style-customization",
        "key-binding-management",
        "behavior-manifest-routing",
        "lease-read-only-state",
        "escape-quit-application-actions",
    ];

    for category in required_categories {
        assert!(
            categories.contains(category),
            "inventory is missing required Phase 7 functionality category {category}"
        );
    }

    let hot_path_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry.get("runtime_path").contains("hot-path"))
        .collect();
    assert!(
        hot_path_entries
            .iter()
            .any(|entry| entry.get("hot_path_policy").contains("asynchronously")),
        "hot-path inventory must record that ordinary editing is async to the server"
    );
}

#[test]
fn api_inventory_does_not_mark_internal_details_public() {
    let entries = inventory_entries();
    let internal_ids = [
        "internal.editor.buffer",
        "internal.editor.layoutPaint",
        "internal.protocol.dto",
        "internal.server.ipcRuntime",
    ];

    for internal_id in internal_ids {
        let entry = entries
            .iter()
            .find(|entry| entry.get("id") == internal_id)
            .unwrap_or_else(|| panic!("missing internal inventory entry {internal_id}"));
        assert_eq!(
            entry.get("visibility"),
            "internal",
            "{internal_id} must be marked internal"
        );
        assert_eq!(
            entry.get("registry_public"),
            "false",
            "{internal_id} must not be included in public registry generation"
        );
        assert!(
            entry.get("js_module").is_empty() && entry.get("js_export").is_empty(),
            "{internal_id} must not expose a Clay JS module/export"
        );
    }
}

#[test]
fn inventory_future_ops_are_not_user_facing_exports() {
    for entry in public_inventory_entries() {
        let id = entry.get("id");
        let js_export = entry.get("js_export");
        let deno_op = entry.get("deno_op");

        assert!(
            !js_export.starts_with("op_") && !js_export.starts_with("opClay"),
            "public inventory entry {id} exposes raw op-shaped JS export {js_export}"
        );
        assert!(
            deno_op.starts_with("op_clay_"),
            "public inventory entry {id} must map to an explicit future op_clay_* wrapper, got {deno_op}"
        );
        assert_ne!(
            js_export, deno_op,
            "public inventory entry {id} must not make the future op wrapper the user-facing JS export"
        );
    }
}

#[test]
fn clay_js_api_docs_have_required_frontmatter_and_body_sections() {
    let required_frontmatter = [
        "id",
        "kind",
        "js_module",
        "js_export",
        "js_facade",
        "backing_rust",
        "deno_op",
        "deno_op_path",
        "name",
        "user_facing_name",
        "summary",
        "owner",
        "phase",
        "visibility",
        "permissions",
        "key_bindings",
        "custom_properties",
        "security",
        "agent_guidance",
        "lookup_tags",
        "app_visible",
        "help_visible",
        "stability",
        "async",
    ];
    let required_sections = [
        "## Summary",
        "## Description",
        "## When to use",
        "## JavaScript usage",
        "## Example",
        "## Options",
        "## Key bindings",
        "## Custom properties",
        "## Return and async behavior",
        "## Errors",
        "## Permissions and security",
        "## Agent guidance",
        "## Backing implementation",
        "## Lookup metadata",
    ];

    for entry in public_inventory_entries() {
        let id = entry.get("id");
        let doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.get("documentation_path"));
        assert!(
            doc_path.exists(),
            "{id} documentation file is missing: {doc_path:?}"
        );

        let fields = markdown_frontmatter(&doc_path);
        for field in required_frontmatter {
            assert!(
                fields.contains_key(field),
                "{id} documentation is missing frontmatter field {field}"
            );
        }
        assert_eq!(fields.get("kind").map(String::as_str), Some("clay-js-api"));
        assert_eq!(fields.get("visibility").map(String::as_str), Some("public"));
        assert_eq!(fields.get("stability").map(String::as_str), Some("planned"));
        assert!(
            fields
                .get("security")
                .is_some_and(|security| security.contains("does not grant filesystem")),
            "{id} documentation must state authority not granted"
        );
        assert!(
            fields.get("lookup_tags").is_some_and(|tags| tags != "[]"),
            "{id} documentation must include lookup tags"
        );

        let text = fs::read_to_string(&doc_path).expect("read API doc");
        assert!(
            text.contains(&format!("# {}", entry.get("js_export"))),
            "{id} documentation must title the JS export"
        );
        for section in required_sections {
            assert!(
                text.contains(section),
                "{id} documentation is missing {section}"
            );
        }
        assert!(
            text.contains("```ts") && text.contains(entry.get("js_module")),
            "{id} documentation must include a TypeScript usage example"
        );
    }
}

#[test]
fn docs_index_links_all_public_inventory_docs() {
    let linked_paths = docs_index_registry_links();
    for entry in public_inventory_entries() {
        let doc_path = entry.get("documentation_path");
        assert!(
            linked_paths.contains(doc_path),
            "docs/index.md must link public API documentation for {} at {doc_path}",
            entry.get("id")
        );
    }
}

#[test]
fn api_docs_match_inventory_ids_and_exports() {
    for entry in public_inventory_entries() {
        let id = entry.get("id");
        let doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.get("documentation_path"));
        let fields = markdown_frontmatter(&doc_path);
        for (doc_field, inventory_field) in [
            ("id", "id"),
            ("js_module", "js_module"),
            ("js_export", "js_export"),
            ("js_facade", "facade_path"),
            ("backing_rust", "backing_rust"),
            ("deno_op", "deno_op"),
            ("deno_op_path", "deno_op_path"),
            ("user_facing_name", "user_facing_name"),
        ] {
            assert_eq!(
                fields.get(doc_field).map(String::as_str),
                Some(entry.get(inventory_field)),
                "{id} documentation frontmatter field {doc_field} must match inventory field {inventory_field}"
            );
        }
    }
}

#[test]
fn clay_js_api_inventory_docs_and_index_are_consistent() {
    let public_entries = public_inventory_entries();
    let inventory_doc_paths: BTreeSet<_> = public_entries
        .iter()
        .map(|entry| entry.get("documentation_path").to_string())
        .collect();
    let linked_paths = docs_index_registry_links();

    assert_eq!(
        linked_paths, inventory_doc_paths,
        "docs/index.md Clay JS API Registry Source Files must exactly match public api-inventory.toml entries; add/remove the named link instead of relying on generated artifacts"
    );

    for entry in public_entries {
        let id = entry.get("id");
        let doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.get("documentation_path"));
        let doc_text = fs::read_to_string(&doc_path).expect("read API doc");
        assert!(
            facade_exports_function(entry.get("facade_path"), entry.get("js_export")),
            "{id} facade_path {} must point at a file exporting {}",
            entry.get("facade_path"),
            entry.get("js_export")
        );
        assert!(
            doc_text.contains(entry.get("facade_path"))
                && doc_text.contains(entry.get("deno_op_path"))
                && doc_text.contains(entry.get("backing_rust")),
            "{} must document facade, future op path, and backing Rust owner in {}",
            id,
            entry.get("documentation_path")
        );
    }
}

#[test]
fn clay_js_api_names_follow_project_conventions() {
    for entry in public_inventory_entries() {
        let id = entry.get("id");
        let js_module = entry.get("js_module");
        let js_export = entry.get("js_export");
        let expected_id = format!(
            "clay.{}.{}",
            js_module
                .strip_prefix("clay:")
                .unwrap_or_else(|| panic!("{id} js_module must start with clay:, got {js_module}")),
            js_export
        );

        assert_eq!(
            id, expected_id,
            "{id} stable ID must be clay.<module>.<export>"
        );
        assert!(
            is_lower_camel_case(js_export),
            "{id} js_export {js_export} must be flat lower-camel-case"
        );
        assert!(
            !js_export.contains("clay")
                && !js_export.contains("Clay")
                && !js_export.contains("op")
                && !js_export.contains("Rust"),
            "{id} js_export {js_export} must not expose Clay/project, raw op, or Rust implementation names"
        );

        if matches!(
            entry.get("category"),
            "text-insertion"
                | "newline"
                | "backspace-delete"
                | "cursor-movement"
                | "selection"
                | "scrolling"
                | "resize-viewport"
                | "cursor-style-customization"
                | "lease-read-only-state"
        ) {
            assert!(
                js_export.starts_with("server") || js_export.starts_with("client"),
                "{id} editor/document state API export {js_export} must carry server/client authority marker"
            );
        }
    }
}

#[test]
fn public_api_docs_include_security_keybinding_and_custom_properties() {
    let denied_authorities = [
        "filesystem",
        "network",
        "shell",
        "extension loading",
        "AI mutation",
        "workspace",
        "client-side JavaScript",
    ];

    for entry in public_inventory_entries() {
        let id = entry.get("id");
        let doc_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.get("documentation_path"));
        let fields = markdown_frontmatter(&doc_path);
        let doc_text = fs::read_to_string(&doc_path).expect("read API doc");

        assert!(
            fields.contains_key("key_bindings"),
            "{id} is missing key_bindings frontmatter"
        );
        assert!(
            fields.contains_key("custom_properties"),
            "{id} is missing custom_properties frontmatter"
        );
        assert!(
            doc_text.contains("## Key bindings") && doc_text.contains("## Custom properties"),
            "{id} must include discoverability sections for key bindings and custom properties"
        );

        for key_binding in parse_toml_string_list(entry.get("key_bindings")) {
            assert!(
                doc_text.contains(&key_binding),
                "{id} documentation must mention inventory key binding {key_binding}"
            );
        }
        for property in inventory_custom_property_names(entry.get("custom_properties")) {
            assert!(
                doc_text.contains(&format!("- `{property}`"))
                    && doc_text.contains(&format!("- name: {property}")),
                "{id} documentation must include custom property metadata for {property} in frontmatter and body"
            );
        }

        let security = fields
            .get("security")
            .map(String::as_str)
            .unwrap_or_default();
        for denied in denied_authorities {
            assert!(
                security.contains(denied) && doc_text.contains(denied),
                "{id} security metadata/body must explicitly say it does not grant {denied} authority"
            );
        }
    }
}
