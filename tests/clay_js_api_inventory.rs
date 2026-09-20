use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use clay::docs::registry::{ClayJsApiRegistry, RegistryEntry};

const REQUIRED_FIELDS: &[&str] = &[
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

const REQUIRED_DOC_SECTIONS: &[&str] = &[
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

const DENIED_AUTHORITIES: &[&str] = &[
    "filesystem",
    "network",
    "shell",
    "extension loading",
    "AI mutation",
    "workspace",
    "package",
    "WASM",
    "client-side JavaScript",
];

#[derive(Clone, Debug)]
struct InventoryEntry {
    fields: BTreeMap<String, String>,
}

impl InventoryEntry {
    fn get(&self, key: &str) -> &str {
        self.fields.get(key).map(String::as_str).unwrap_or("")
    }

    fn is_public(&self) -> bool {
        self.get("registry_public") == "true"
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn inventory_entries() -> Vec<InventoryEntry> {
    let text = fs::read_to_string(root().join("docs/reference/clay-js-api/api-inventory.toml"))
        .expect("read API inventory");
    let mut entries = Vec::new();
    let mut fields = None;

    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[api]]" {
            if let Some(fields) = fields.replace(BTreeMap::new()) {
                entries.push(InventoryEntry { fields });
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(" = ") {
            fields
                .as_mut()
                .expect("inventory fields must follow [[api]]")
                .insert(key.to_string(), value.trim_matches('"').to_string());
        }
    }
    if let Some(fields) = fields {
        entries.push(InventoryEntry { fields });
    }
    entries
}

fn parse_string_list(value: &str) -> Vec<String> {
    let value = value.trim();
    if value == "[]" || !value.starts_with('[') || !value.ends_with(']') {
        return Vec::new();
    }
    value[1..value.len() - 1]
        .split(',')
        .map(|item| item.trim().trim_matches('"'))
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn custom_property_names(value: &str) -> Vec<String> {
    parse_string_list(value)
        .into_iter()
        .filter_map(|property| {
            property
                .split_once(':')
                .or_else(|| property.split_once('='))
                .map(|(name, _)| name.to_string())
        })
        .collect()
}

fn docs_index_registry_links() -> BTreeSet<String> {
    let text = fs::read_to_string(root().join("docs/index.md")).expect("read docs index");
    let section = text
        .split_once("## Clay JS API Registry Source Files")
        .and_then(|(_, rest)| rest.split_once("## Registry Rules"))
        .map(|(section, _)| section)
        .expect("docs/index.md must contain bounded registry source section");
    section
        .lines()
        .filter_map(|line| {
            line.split_once("](")
                .and_then(|(_, rest)| rest.split_once(')'))
        })
        .map(|(path, _)| format!("docs/{path}"))
        .collect()
}

fn exported_function(facade: &str, export: &str) -> bool {
    let Some((path, symbol)) = facade.split_once("::") else {
        return false;
    };
    if symbol != export {
        return false;
    }
    fs::read_to_string(root().join(path)).is_ok_and(|source| {
        source.contains(&format!("export function {export}"))
            || source.contains(&format!("export async function {export}"))
    })
}

fn validate_inventory(entries: &[InventoryEntry]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut ids = BTreeSet::new();
    for entry in entries {
        let id = entry.get("id");
        if id.is_empty() {
            errors.push("api-inventory.toml entry missing field id".to_string());
            continue;
        }
        if !ids.insert(id.to_string()) {
            errors.push(format!("api-inventory.toml duplicate id {id}"));
        }
        for field in REQUIRED_FIELDS {
            if !entry.fields.contains_key(*field) {
                errors.push(format!("{id}: missing inventory field {field}"));
            }
        }
        if !matches!(entry.get("visibility"), "public" | "internal") {
            errors.push(format!(
                "{id}: invalid visibility {}",
                entry.get("visibility")
            ));
        }
        if !matches!(
            entry.get("status"),
            "planned" | "runtime-backed" | "runtime-backed-command" | "current-internal"
        ) {
            errors.push(format!("{id}: invalid status {}", entry.get("status")));
        }
        if !matches!(entry.get("registry_public"), "true" | "false") {
            errors.push(format!(
                "{id}: registry_public must be true or false, got {}",
                entry.get("registry_public")
            ));
        }
        if entry.is_public() {
            for field in REQUIRED_FIELDS {
                if !matches!(*field, "key_bindings" | "custom_properties" | "permissions")
                    && entry.get(field).is_empty()
                {
                    errors.push(format!("{id}: empty public inventory field {field}"));
                }
            }
            if entry.get("visibility") != "public" {
                errors.push(format!(
                    "{id}: registry-public API must have public visibility"
                ));
            }
            let domain = entry.get("id").split('.').next().unwrap_or_default();
            if entry.get("id").starts_with("clay.")
                || !clay::packages::manifest::RESERVED_CORE_API_DOMAINS.contains(&domain)
            {
                errors.push(format!(
                    "{id}: public stable id must use a bare Clay core API domain (<domain>.<name>)"
                ));
            }
            for authority in DENIED_AUTHORITIES {
                if !entry.get("security_notes").contains(authority) {
                    errors.push(format!(
                        "{id}: security_notes missing denied authority {authority}"
                    ));
                }
            }
        } else if entry.get("visibility") == "internal"
            && (!entry.get("js_module").is_empty() || !entry.get("js_export").is_empty())
        {
            errors.push(format!("{id}: internal entry exposes a JS module/export"));
        }
    }
    errors
}

fn assert_valid(errors: Vec<String>) {
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

fn registry_by_id(registry: &ClayJsApiRegistry) -> BTreeMap<&str, &RegistryEntry> {
    registry
        .entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect()
}

#[test]
fn api_inventory_schema_is_complete_and_actionable() {
    let entries = inventory_entries();
    assert!(
        !entries.is_empty(),
        "api-inventory.toml has no [[api]] entries"
    );
    assert_valid(validate_inventory(&entries));

    let mut broken = entries[0].clone();
    let id = broken.get("id").to_string();
    broken.fields.remove("security_notes");
    let error = validate_inventory(&[broken]).join("\n");
    assert!(error.contains(&format!("{id}: missing inventory field security_notes")));
}

#[test]
fn configuration_surface_is_closed_and_security_controls_are_not_properties() {
    let expected = BTreeSet::from([
        "getConfigurationState",
        "loadConfigurationModule",
        "setDecorationTheme",
        "setModePreference",
        "setPackageOption",
        "setParsePolicy",
    ]);
    let configuration_entries: Vec<_> = inventory_entries()
        .into_iter()
        .filter(|entry| entry.get("js_module") == "clay:configuration")
        .collect();
    assert_eq!(
        configuration_entries
            .iter()
            .map(|entry| entry.get("js_export"))
            .collect::<BTreeSet<_>>(),
        expected
    );
    for entry in &configuration_entries {
        let implemented = matches!(
            entry.get("js_export"),
            "getConfigurationState" | "loadConfigurationModule" | "setPackageOption"
        );
        assert_eq!(entry.get("registry_public") == "true", implemented);
        assert_eq!(entry.get("status") == "runtime-backed", implemented);
        for property in custom_property_names(entry.get("custom_properties")) {
            assert!(
                ![
                    "runtimeDomain",
                    "packageContext",
                    "clientId",
                    "queueCapacity",
                    "maxActiveConnections",
                    "maxDocumentsPerClient",
                    "atomicSaveMode",
                    "directoryListingConcurrency",
                    "gitRootConcurrency",
                    "languageServerSessionQueueCapacity",
                    "dialogGeneration",
                    "clipboardBackend",
                    "debugProfile",
                    "targetDirectory",
                ]
                .contains(&property.as_str()),
                "{} exposes internal configuration property {property}",
                entry.get("id")
            );
        }
    }

    let facade = fs::read_to_string(root().join("runtime/js/configuration.js"))
        .expect("read configuration facade");
    let exports = facade
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("export function ")
                .or_else(|| line.strip_prefix("export async function "))
                .and_then(|rest| rest.split_once('(').map(|(name, _)| name))
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(exports, expected);

    let facade_table =
        fs::read_to_string(root().join("src/server/facades.rs")).expect("read facade table");
    assert!(facade_table.contains("Facade::trusted(\n        \"clay:configuration\","));
    assert!(!facade_table.contains("Facade::public(\"clay:configuration\""));
}

#[test]
fn public_inventory_docs_index_and_generated_matrix_match_exactly() {
    let entries: Vec<_> = inventory_entries()
        .into_iter()
        .filter(InventoryEntry::is_public)
        .collect();
    let inventory_ids: BTreeSet<_> = entries.iter().map(|entry| entry.get("id")).collect();
    let inventory_docs: BTreeSet<_> = entries
        .iter()
        .map(|entry| entry.get("documentation_path").to_string())
        .collect();
    let registry = ClayJsApiRegistry::from_generated().expect("load generated API registry");
    let generated_ids: BTreeSet<_> = registry
        .entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();

    assert_eq!(
        generated_ids, inventory_ids,
        "generated API matrix IDs must exactly match registry-public inventory IDs; run cargo run --bin update-doc-registry after intentional changes"
    );
    assert_eq!(
        docs_index_registry_links(),
        inventory_docs,
        "docs/index.md registry links must exactly match registry-public documentation_path fields"
    );
}

#[test]
fn every_public_api_contract_matches_generated_markdown_metadata() {
    let entries: Vec<_> = inventory_entries()
        .into_iter()
        .filter(InventoryEntry::is_public)
        .collect();
    let registry = ClayJsApiRegistry::from_generated().expect("load generated API registry");
    let generated = registry_by_id(&registry);

    for inventory in entries {
        let id = inventory.get("id");
        let entry = generated
            .get(id)
            .unwrap_or_else(|| panic!("{id}: missing generated registry entry"));
        for (field, actual, expected) in [
            (
                "js_module",
                entry.js_module.as_str(),
                inventory.get("js_module"),
            ),
            (
                "js_export",
                entry.js_export.as_str(),
                inventory.get("js_export"),
            ),
            (
                "js_facade",
                entry.js_facade.as_str(),
                inventory.get("facade_path"),
            ),
            (
                "backing_rust",
                entry.backing_rust.as_str(),
                inventory.get("backing_rust"),
            ),
            ("deno_op", entry.deno_op.as_str(), inventory.get("deno_op")),
            (
                "deno_op_path",
                entry.deno_op_path.as_str(),
                inventory.get("deno_op_path"),
            ),
            (
                "user_facing_name",
                entry.user_facing_name.as_str(),
                inventory.get("user_facing_name"),
            ),
            (
                "stability",
                entry.stability.as_str(),
                inventory.get("status"),
            ),
            (
                "documentation_path",
                entry.documentation_path.as_str(),
                inventory.get("documentation_path"),
            ),
        ] {
            assert_eq!(
                actual, expected,
                "{id}: generated/Markdown field {field} differs from api-inventory.toml"
            );
        }
        assert_eq!(
            entry.key_bindings,
            parse_string_list(inventory.get("key_bindings")),
            "{id}: key_bindings differ"
        );
        assert_eq!(
            entry.permissions,
            parse_string_list(inventory.get("permissions")),
            "{id}: permissions differ"
        );
        let generated_properties: BTreeSet<_> = entry
            .custom_properties
            .iter()
            .map(|property| property.name.as_str())
            .collect();
        let inventory_properties = custom_property_names(inventory.get("custom_properties"));
        let inventory_properties: BTreeSet<_> =
            inventory_properties.iter().map(String::as_str).collect();
        assert_eq!(
            generated_properties, inventory_properties,
            "{id}: custom_properties differ"
        );
    }
}

#[test]
fn every_public_api_has_generic_sections_facade_and_naming_contract() {
    for entry in inventory_entries()
        .into_iter()
        .filter(InventoryEntry::is_public)
    {
        let id = entry.get("id");
        let path = root().join(entry.get("documentation_path"));
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{id}: read {}: {error}", path.display()));
        for section in REQUIRED_DOC_SECTIONS {
            assert!(
                text.contains(section),
                "{id}: {} missing section {section}",
                path.display()
            );
        }
        assert!(
            text.contains("```ts") && text.contains(entry.get("js_module")),
            "{id}: {} missing TypeScript usage for {}",
            path.display(),
            entry.get("js_module")
        );
        assert!(
            exported_function(entry.get("facade_path"), entry.get("js_export")),
            "{id}: facade_path {} does not export {}",
            entry.get("facade_path"),
            entry.get("js_export")
        );

        let module = entry
            .get("js_module")
            .strip_prefix("clay:")
            .unwrap_or_else(|| panic!("{id}: js_module must start with clay:"));
        assert_eq!(
            id,
            format!("{module}.{}", entry.get("js_export")),
            "{id}: stable id must derive from module/export"
        );
        let export = entry.get("js_export");
        assert!(
            export
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase())
                && export.chars().all(|ch| ch.is_ascii_alphanumeric()),
            "{id}: export must be flat lowerCamelCase"
        );
        assert!(
            !export.starts_with("op_")
                && !export.starts_with("opClay")
                && export != entry.get("deno_op"),
            "{id}: raw op name exposed as public export"
        );
        assert!(
            entry.get("deno_op").starts_with("op_clay_"),
            "{id}: deno_op must use op_clay_* wrapper"
        );
    }
}

#[test]
fn permissions_and_authority_remain_structured_security_fields() {
    let registry = ClayJsApiRegistry::from_generated().expect("load generated API registry");
    for entry in &registry.entries {
        for authority in DENIED_AUTHORITIES {
            assert!(
                entry.security.contains(authority),
                "{}: generated security field missing denied authority {authority}",
                entry.id
            );
        }
        assert!(
            entry
                .permissions
                .iter()
                .all(|permission| !permission.is_empty()),
            "{}: permissions must be explicit non-empty identifiers",
            entry.id
        );
    }
}

#[test]
fn harmless_api_prose_is_not_a_validation_input() {
    let mut body = REQUIRED_DOC_SECTIONS.join("\n");
    body.push_str("\n```ts\nimport {} from \"clay:test\";\n```\n");
    for section in REQUIRED_DOC_SECTIONS {
        assert!(body.contains(section));
    }
    body.push_str("This paragraph may be rewritten without changing structured metadata.\n");
    assert!(
        REQUIRED_DOC_SECTIONS
            .iter()
            .all(|section| body.contains(section))
    );
}

#[test]
fn source_paths_named_by_public_metadata_exist() {
    for entry in inventory_entries()
        .into_iter()
        .filter(InventoryEntry::is_public)
    {
        let id = entry.get("id");
        let facade = entry.get("facade_path");
        let facade_path = facade.split_once("::").map_or(facade, |(path, _)| path);
        assert!(
            root().join(facade_path).is_file(),
            "{id}: facade_path names missing source file {facade_path}"
        );
        if entry.get("status").starts_with("runtime-backed") {
            let op = entry.get("deno_op_path");
            let op_path = op.split_once("::").map_or(op, |(path, _)| path);
            assert!(
                root().join(op_path).is_file(),
                "{id}: runtime-backed deno_op_path names missing source file {op_path}"
            );
            for owner in entry
                .get("backing_rust")
                .split(';')
                .map(str::trim)
                .filter(|owner| owner.starts_with("src/"))
            {
                let end = owner
                    .find(".rs")
                    .map(|index| index + 3)
                    .unwrap_or(owner.len());
                let path = &owner[..end];
                assert!(
                    root().join(path).is_file(),
                    "{id}: runtime-backed backing_rust names missing source file {path}"
                );
            }
        }
    }
}

#[test]
fn inventory_rust_paths_name_existing_source_files() {
    // Migration guard: every `src/*.rs` path named in inventory metadata must
    // exist today. Removed native-client modules (masonry_*,
    // editor/surface, app_driver, driver/restore) must not be cited as
    // backing/current owners; use retained renderer-neutral Rust or the
    // React/CodeMirror frontend owner instead.
    fn names_rust_paths(value: &str) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        let bytes = value.as_bytes();
        let mut index = 0;
        while let Some(offset) = value[index..].find("src/") {
            let start = index + offset;
            let at_token_start = start == 0
                || !bytes[start - 1].is_ascii_alphanumeric()
                    && !matches!(bytes[start - 1], b'-' | b'_' | b'/' | b'.');
            if at_token_start && let Some(end) = value[start..].find(".rs") {
                names.insert(value[start..start + end + 3].to_string());
            }
            index = start + 4;
        }
        names
    }
    for entry in inventory_entries() {
        let id = entry.get("id");
        for field in ["backing_rust", "current_rust_owner"] {
            for path in names_rust_paths(entry.get(field)) {
                let message = format!(
                    concat!(
                        "{id}: {field} names missing source file {path}; point at a retained ",
                        "renderer-neutral owner (src/client_commands.rs, src/server/*, ",
                        "src/shell/layout/mod.rs) or the React/CodeMirror frontend owner"
                    ),
                    id = id,
                    field = field,
                    path = path
                );
                assert!(root().join(&path).is_file(), "{message}");
            }
        }
    }
}

#[test]
fn documentation_validation_is_read_only() {
    let inventory = root().join("docs/reference/clay-js-api/api-inventory.toml");
    let registry = root().join("docs/generated/clay-js-api-registry.json");
    let before = (fs::read(&inventory).unwrap(), fs::read(&registry).unwrap());
    assert_valid(validate_inventory(&inventory_entries()));
    ClayJsApiRegistry::from_generated().expect("validate generated registry");
    let after = (fs::read(&inventory).unwrap(), fs::read(&registry).unwrap());
    assert_eq!(
        before, after,
        "documentation validators must never mutate source/generated files"
    );
}

#[test]
fn inventory_paths_are_repository_relative() {
    for entry in inventory_entries() {
        for field in ["documentation_path", "facade_path", "deno_op_path"] {
            let value = entry.get(field);
            if value.is_empty() {
                continue;
            }
            let path = value.split_once("::").map_or(value, |(path, _)| path);
            assert!(
                !Path::new(path).is_absolute() && !path.contains(".."),
                "{}: {field} must be repository-relative: {path}",
                entry.get("id")
            );
        }
    }
}

#[test]
fn agent_configuration_options_are_documented_custom_properties_with_decision_defaults() {
    // Plan 107 task 12 + decisions 2157/2158/2026-09-20-2049: autonomy
    // default (opt-out, on), compaction
    // strategy, and OM compactAfterTokens must be documented Clay JS API
    // custom properties (not hidden config keys), with the decision
    // defaults stated in the inventory itself.
    let entries: Vec<_> = inventory_entries()
        .into_iter()
        .filter(|entry| entry.get("id").starts_with("agent."))
        .collect();
    assert!(!entries.is_empty(), "agent facade entries must exist");

    let autonomy = entries
        .iter()
        .find(|entry| entry.get("id") == "agent.setFullAutonomy")
        .expect("agent.setFullAutonomy entry");
    let autonomy_properties = custom_property_names(autonomy.get("custom_properties"));
    assert!(
        autonomy_properties.contains(&"default".to_string()),
        "agent.setFullAutonomy must document the autonomy default as a custom property"
    );
    assert!(
        autonomy
            .get("custom_properties")
            .contains("default:boolean=true"),
        "decision 2026-09-20-2049: full autonomy defaults to on (opt-out)"
    );
    // The documented default is only worth pinning if the daemon agrees: a
    // session is autonomous unless its creator passed `fullAutonomy: false`,
    // and the resume path must restore the recorded value rather than a
    // hardcoded one. Both sides of the doc-vs-code divergence are gated here
    // so neither can drift alone.
    let sessions_ts = fs::read_to_string(root().join("clay-agent/src/host/sessions.ts")).unwrap();
    assert!(
        sessions_ts.contains("params.fullAutonomy !== false"),
        "the daemon default must stay opt-out (decision 2026-09-20-2049)"
    );
    assert!(
        sessions_ts.contains("const fullAutonomy = metadata.fullAutonomy !== false;"),
        "a resumed session must restore the autonomy recorded with it"
    );
    assert!(
        !sessions_ts.contains("fullAutonomy: false,\n    observationalMemory"),
        "ensureLive must not hardcode the live record's autonomy to false"
    );

    let compact = entries
        .iter()
        .find(|entry| entry.get("id") == "agent.compact")
        .expect("agent.compact entry");
    let compact_properties = custom_property_names(compact.get("custom_properties"));
    assert!(
        compact_properties.contains(&"compactAfterTokens".to_string()),
        "agent.compact must document compactAfterTokens as a custom property"
    );
    assert!(
        compact
            .get("custom_properties")
            .contains("compactAfterTokens:number=80000"),
        "decision 2158: compactAfterTokens default is 80000"
    );
    let compact_doc = root().join(compact.get("documentation_path"));
    assert!(
        compact_doc.is_file()
            && fs::read_to_string(&compact_doc)
                .unwrap()
                .contains("compactAfterTokens"),
        "agent.compact docs must document compactAfterTokens"
    );
    // The strategy name is an option of agent.compact (documented in Options),
    // and no separate strategy configuration API exists.
    assert!(
        !entries.iter().any(|entry| {
            let id = entry.get("id");
            id.contains("strategy") || id.contains("compactionStrategy")
        }),
        "compaction strategy is selected via agent.compact's strategy option, not a separate API"
    );
    // No hidden configuration keys: the daemon default must match the docs.
    let compaction_ts = fs::read_to_string(root().join("clay-agent/src/compaction.ts")).unwrap();
    assert!(
        compaction_ts.contains("DEFAULT_COMPACT_AFTER_TOKENS = 80_000"),
        "daemon default compactAfterTokens must stay 80000 (decision 2158)"
    );
}

/// Plan 118: the design-system/theme selection surface changed values, not
/// APIs. This gate pins the documented configuration contract: the choice set
/// and removal fallback are stated where users configure them, and every
/// behaviour-changing option a configuration API declares in frontmatter is
/// actually documented in its page body — so an option that exists only in
/// metadata (or only in the inventory) cannot ship undocumented.
#[test]
fn plan118_configuration_documents_choice_set_fallback_and_every_option() {
    let configuration =
        fs::read_to_string(root().join("docs/reference/clay-js-api/configuration.md"))
            .expect("read configuration guide");
    for marker in [
        "## Plan 118 design-system, theme, and chat-surface configuration review",
        "### Shipped configuration surfaces",
        "`settings.setTheme`",
        "`settings.setAppearance`",
        "`settings.setTypography`",
        "`settings.setDesignSystem`",
        "preferences.json",
        "ui_choices",
        "### Shipped choice set",
        "@clay/theme-modus-operandi",
        "@clay/theme-modus-vivendi",
        "@clay/theme-gruvbox-material-dark",
        "@clay/theme-gruvbox-material-light",
        "@clay/design-instrument",
        "### Removed-specifier fallback",
        "theme.load_failed",
        "kept",
        "### Removed chat-surface options",
        "### Rejected hidden configuration keys",
        "landingPackage",
        "### Authority",
        "no new configuration authority",
        "adds no file-watch, polling, or reload work",
    ] {
        assert!(
            configuration.contains(marker),
            "configuration guide must document plan 118 marker {marker}"
        );
    }

    // Both selection surfaces must be discoverable from the configuration guide.
    for api in [
        "theme.setDesignSystem",
        "theme.setTheme",
        "settings.setDesignSystem",
    ] {
        assert!(
            configuration.contains(api),
            "configuration guide must link {api}"
        );
    }

    // Every custom property a configuration API declares must be documented in
    // the page body, not only in frontmatter metadata or the inventory.
    let registry = ClayJsApiRegistry::from_docs(&root()).expect("build registry from docs");
    for id in [
        "theme.setTheme",
        "theme.setAppearance",
        "theme.setDesignSystem",
        "settings.setDesignSystem",
    ] {
        let entry = registry
            .by_id(id)
            .unwrap_or_else(|| panic!("{id} must stay a public configuration API"));
        assert!(
            !entry.custom_properties.is_empty(),
            "{id} must declare its behaviour-changing options"
        );
        let path = root().join(&entry.documentation_path);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{id}: read {}: {error}", path.display()));
        let body = text
            .strip_prefix("---")
            .and_then(|rest| rest.split_once("\n---\n").map(|(_, body)| body))
            .unwrap_or_else(|| panic!("{id}: {} must keep frontmatter", path.display()));
        // The option must be documented where a user looks for it — inside the
        // page's `## Options` section — not merely somewhere in prose.
        let options = body
            .find("## Options")
            .map(|start| {
                let rest = &body[start + "## Options".len()..];
                let end = rest.find("\n## ").unwrap_or(rest.len());
                &rest[..end]
            })
            .unwrap_or_else(|| panic!("{id}: {} must keep an Options section", path.display()));
        for property in &entry.custom_properties {
            // The repo's two Options conventions both backtick the option name:
            // a bullet (`- \`option\`: ...`) or the object/positional form
            // (`{ option }`, `pass \`option\``). Requiring the backticked form is
            // what makes "documented" mean declared-here rather than mentioned
            // somewhere in the same paragraph.
            let bare = format!("`{}`", property.name);
            let object = format!("`{{ {} }}`", property.name);
            assert!(
                options.contains(&bare) || options.contains(&object),
                "{id}: option {} is declared but not documented in the Options section of {}",
                bare,
                path.display()
            );
        }
    }

    // The removed surface must not survive as a live configuration option, and
    // the canonical example must not carry it at all. Both documents are allowed
    // to *name* rejected keys (`landingPackage`, `chatPanel.*`) in the rejected-key
    // sections that teach the closed boundary — that is the opposite of shipping
    // them — so the identifiers asserted absent here are the ones no prose needs.
    let init_js = fs::read_to_string(root().join("examples/config/init.js"))
        .expect("read canonical example config");
    for retired in ["@clay/chat", "chat.entry", "chat.default."] {
        assert!(
            !configuration.contains(retired),
            "removed chat surface `{retired}` must not appear in the configuration guide"
        );
    }
    for retired in [
        "@clay/chat",
        "chat.entry",
        "chat.default.",
        "chatPanel.",
        "landingPackage",
        "design-neobrutal",
        "design-glass",
    ] {
        assert!(
            !init_js.contains(retired),
            "canonical example config must not mention `{retired}`"
        );
    }
    // Rejected keys are documented as rejected, which is what keeps them closed.
    for rejected in ["landingPackage", "chatPanel."] {
        assert!(
            configuration.contains(rejected),
            "configuration guide must name rejected key {rejected} in the closed boundary"
        );
    }
}
