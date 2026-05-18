use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const GENERATED_REGISTRY_PATH: &str = "docs/generated/clay-js-api-registry.json";
pub const UPDATE_COMMAND: &str = "cargo run --bin update-doc-registry";

const REGISTRY_SECTION: &str = "## Clay JS API Registry Source Files";
const REGISTRY_RULES_SECTION: &str = "## Registry Rules";
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomProperty {
    pub name: String,
    pub property_type: String,
    pub default: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    pub id: String,
    pub kind: String,
    pub js_module: String,
    pub js_export: String,
    pub js_facade: String,
    pub backing_rust: String,
    pub deno_op: String,
    pub deno_op_path: String,
    pub name: String,
    pub user_facing_name: String,
    pub summary: String,
    pub owner: String,
    pub phase: String,
    pub visibility: String,
    pub permissions: Vec<String>,
    pub key_bindings: Vec<String>,
    pub custom_properties: Vec<CustomProperty>,
    pub security: String,
    pub agent_guidance: String,
    pub lookup_tags: Vec<String>,
    pub app_visible: bool,
    pub help_visible: bool,
    pub stability: String,
    pub is_async: bool,
    pub documentation_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClayJsApiRegistry {
    pub entries: Vec<RegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryError {
    message: String,
}

impl RegistryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RegistryError {}

pub type RegistryResult<T> = Result<T, RegistryError>;

impl ClayJsApiRegistry {
    pub fn from_docs(root: &Path) -> RegistryResult<Self> {
        let source_paths = registry_source_paths(root)?;
        let mut entries = source_paths
            .iter()
            .map(|path| parse_api_doc(root, path))
            .collect::<RegistryResult<Vec<_>>>()?;
        entries.sort_by(|left, right| left.id.cmp(&right.id));

        let mut ids = BTreeSet::new();
        for entry in &entries {
            if !ids.insert(entry.id.clone()) {
                return Err(RegistryError::new(format!(
                    "duplicate Clay JS API registry id {}",
                    entry.id
                )));
            }
        }

        Ok(Self { entries })
    }

    pub fn from_generated() -> RegistryResult<Self> {
        Self::from_generated_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/generated/clay-js-api-registry.json"
        )))
    }

    pub fn from_generated_json(text: &str) -> RegistryResult<Self> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|err| {
            RegistryError::new(format!("parse generated Clay JS API registry JSON: {err}"))
        })?;
        let schema_version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                RegistryError::new("generated Clay JS API registry is missing schema_version")
            })?;
        if schema_version != 1 {
            return Err(RegistryError::new(format!(
                "unsupported Clay JS API registry schema_version {schema_version}"
            )));
        }

        let entries_value = value
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                RegistryError::new("generated Clay JS API registry is missing entries array")
            })?;
        let mut entries = entries_value
            .iter()
            .enumerate()
            .map(|(index, value)| registry_entry_from_json(value, index))
            .collect::<RegistryResult<Vec<_>>>()?;
        entries.sort_by(|left, right| left.id.cmp(&right.id));

        let mut ids = BTreeSet::new();
        for entry in &entries {
            validate_entry(entry)?;
            if !ids.insert(entry.id.clone()) {
                return Err(RegistryError::new(format!(
                    "duplicate Clay JS API registry id {} in generated registry",
                    entry.id
                )));
            }
        }

        Ok(Self { entries })
    }

    pub fn by_id(&self, id: &str) -> Option<&RegistryEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn by_js_export(&self, js_module: &str, js_export: &str) -> Option<&RegistryEntry> {
        self.entries
            .iter()
            .find(|entry| entry.js_module == js_module && entry.js_export == js_export)
    }

    pub fn by_user_facing_name(&self, user_facing_name: &str) -> Vec<&RegistryEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                entry
                    .user_facing_name
                    .eq_ignore_ascii_case(user_facing_name)
            })
            .collect()
    }

    pub fn by_kind_owner(&self, kind: Option<&str>, owner: Option<&str>) -> Vec<&RegistryEntry> {
        self.entries
            .iter()
            .filter(|entry| kind.is_none_or(|kind| entry.kind == kind))
            .filter(|entry| owner.is_none_or(|owner| entry.owner == owner))
            .collect()
    }

    pub fn by_lookup_tag(&self, lookup_tag: &str) -> Vec<&RegistryEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.lookup_tags.iter().any(|tag| tag == lookup_tag))
            .collect()
    }

    pub fn by_key_binding(&self, key_binding: &str) -> Vec<&RegistryEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                entry
                    .key_bindings
                    .iter()
                    .any(|binding| binding == key_binding)
            })
            .collect()
    }

    pub fn by_custom_property(&self, property_name: &str) -> Vec<&RegistryEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                entry
                    .custom_properties
                    .iter()
                    .any(|property| property.name == property_name)
            })
            .collect()
    }

    pub fn to_generated_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str("  \"schema_version\": 1,\n");
        out.push_str("  \"source\": \"docs/index.md#Clay JS API Registry Source Files\",\n");
        out.push_str("  \"entries\": [\n");
        for (index, entry) in self.entries.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            write_entry_json(&mut out, entry);
        }
        out.push_str("\n  ]\n");
        out.push_str("}\n");
        out
    }
}

pub fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn expected_generated_registry(root: &Path) -> RegistryResult<String> {
    Ok(ClayJsApiRegistry::from_docs(root)?.to_generated_json())
}

pub fn update_generated_registry(root: &Path) -> RegistryResult<PathBuf> {
    let generated = expected_generated_registry(root)?;
    let path = root.join(GENERATED_REGISTRY_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            RegistryError::new(format!(
                "create generated registry dir {}: {err}",
                parent.display()
            ))
        })?;
    }
    fs::write(&path, generated)
        .map_err(|err| RegistryError::new(format!("write {}: {err}", path.display())))?;
    Ok(path)
}

pub fn check_generated_registry_current(root: &Path) -> RegistryResult<()> {
    let expected = expected_generated_registry(root)?;
    let path = root.join(GENERATED_REGISTRY_PATH);
    let actual = fs::read_to_string(&path).map_err(|err| {
        RegistryError::new(format!(
            "read generated Clay JS API registry {}: {err}. Run `{}` to create it.",
            path.display(),
            UPDATE_COMMAND
        ))
    })?;

    if actual != expected {
        return Err(RegistryError::new(format!(
            "{} is stale or malformed. Run `{}` and commit the updated generated artifact.",
            GENERATED_REGISTRY_PATH, UPDATE_COMMAND
        )));
    }

    Ok(())
}

pub fn registry_source_paths(root: &Path) -> RegistryResult<Vec<String>> {
    let index_path = root.join("docs/index.md");
    let text = fs::read_to_string(&index_path)
        .map_err(|err| RegistryError::new(format!("read {}: {err}", index_path.display())))?;
    let section = text
        .split(REGISTRY_SECTION)
        .nth(1)
        .ok_or_else(|| {
            RegistryError::new("docs/index.md is missing Clay JS API registry source section")
        })?
        .split(REGISTRY_RULES_SECTION)
        .next()
        .ok_or_else(|| RegistryError::new("docs/index.md is missing Registry Rules section"))?;

    let mut paths = Vec::new();
    for line in section.lines() {
        let Some((_, rest)) = line.split_once("](") else {
            continue;
        };
        let Some((link, _)) = rest.split_once(')') else {
            continue;
        };
        let doc_path = format!("docs/{link}");
        if !doc_path.starts_with("docs/reference/clay-js-api/") {
            return Err(RegistryError::new(format!(
                "registry source link must be under docs/reference/clay-js-api/: {doc_path}"
            )));
        }
        paths.push(doc_path);
    }

    if paths.is_empty() {
        return Err(RegistryError::new(
            "docs/index.md registry source section did not contain any Markdown links",
        ));
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn registry_entry_from_json(
    value: &serde_json::Value,
    index: usize,
) -> RegistryResult<RegistryEntry> {
    let path = format!("generated registry entry #{index}");
    let custom_properties = json_array(value, &path, "custom_properties")?
        .iter()
        .enumerate()
        .map(|(property_index, property)| {
            let property_path = format!("{path} custom_properties[{property_index}]");
            Ok(CustomProperty {
                name: json_string(property, &property_path, "name")?,
                property_type: json_string(property, &property_path, "type")?,
                default: json_string(property, &property_path, "default")?,
                description: json_string(property, &property_path, "description")?,
            })
        })
        .collect::<RegistryResult<Vec<_>>>()?;

    Ok(RegistryEntry {
        id: json_string(value, &path, "id")?,
        kind: json_string(value, &path, "kind")?,
        js_module: json_string(value, &path, "js_module")?,
        js_export: json_string(value, &path, "js_export")?,
        js_facade: json_string(value, &path, "js_facade")?,
        backing_rust: json_string(value, &path, "backing_rust")?,
        deno_op: json_string(value, &path, "deno_op")?,
        deno_op_path: json_string(value, &path, "deno_op_path")?,
        name: json_string(value, &path, "name")?,
        user_facing_name: json_string(value, &path, "user_facing_name")?,
        summary: json_string(value, &path, "summary")?,
        owner: json_string(value, &path, "owner")?,
        phase: json_string(value, &path, "phase")?,
        visibility: json_string(value, &path, "visibility")?,
        permissions: json_string_list(value, &path, "permissions")?,
        key_bindings: json_string_list(value, &path, "key_bindings")?,
        custom_properties,
        security: json_string(value, &path, "security")?,
        agent_guidance: json_string(value, &path, "agent_guidance")?,
        lookup_tags: json_string_list(value, &path, "lookup_tags")?,
        app_visible: json_bool(value, &path, "app_visible")?,
        help_visible: json_bool(value, &path, "help_visible")?,
        stability: json_string(value, &path, "stability")?,
        is_async: json_bool(value, &path, "async")?,
        documentation_path: json_string(value, &path, "documentation_path")?,
    })
}

fn json_string(value: &serde_json::Value, path: &str, field: &str) -> RegistryResult<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            RegistryError::new(format!("{path} is missing non-empty string field {field}"))
        })
}

fn json_bool(value: &serde_json::Value, path: &str, field: &str) -> RegistryResult<bool> {
    value
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| RegistryError::new(format!("{path} is missing boolean field {field}")))
}

fn json_array<'a>(
    value: &'a serde_json::Value,
    path: &str,
    field: &str,
) -> RegistryResult<&'a Vec<serde_json::Value>> {
    value
        .get(field)
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| RegistryError::new(format!("{path} is missing array field {field}")))
}

fn json_string_list(
    value: &serde_json::Value,
    path: &str,
    field: &str,
) -> RegistryResult<Vec<String>> {
    json_array(value, path, field)?
        .iter()
        .enumerate()
        .map(|(index, item)| {
            item.as_str().map(ToString::to_string).ok_or_else(|| {
                RegistryError::new(format!("{path} field {field}[{index}] must be a string"))
            })
        })
        .collect()
}

fn parse_api_doc(root: &Path, documentation_path: &str) -> RegistryResult<RegistryEntry> {
    let path = root.join(documentation_path);
    let text = fs::read_to_string(&path)
        .map_err(|err| RegistryError::new(format!("read {}: {err}", path.display())))?;
    let frontmatter = frontmatter_text(&text, documentation_path)?;
    let scalar = parse_scalar_frontmatter(frontmatter);
    let custom_properties = parse_custom_properties(frontmatter, documentation_path)?;

    let entry = RegistryEntry {
        id: required_scalar(&scalar, documentation_path, "id")?,
        kind: required_scalar(&scalar, documentation_path, "kind")?,
        js_module: required_scalar(&scalar, documentation_path, "js_module")?,
        js_export: required_scalar(&scalar, documentation_path, "js_export")?,
        js_facade: required_scalar(&scalar, documentation_path, "js_facade")?,
        backing_rust: required_scalar(&scalar, documentation_path, "backing_rust")?,
        deno_op: required_scalar(&scalar, documentation_path, "deno_op")?,
        deno_op_path: required_scalar(&scalar, documentation_path, "deno_op_path")?,
        name: required_scalar(&scalar, documentation_path, "name")?,
        user_facing_name: required_scalar(&scalar, documentation_path, "user_facing_name")?,
        summary: required_scalar(&scalar, documentation_path, "summary")?,
        owner: required_scalar(&scalar, documentation_path, "owner")?,
        phase: required_scalar(&scalar, documentation_path, "phase")?,
        visibility: required_scalar(&scalar, documentation_path, "visibility")?,
        permissions: required_list(&scalar, documentation_path, "permissions")?,
        key_bindings: required_list(&scalar, documentation_path, "key_bindings")?,
        custom_properties,
        security: required_scalar(&scalar, documentation_path, "security")?,
        agent_guidance: required_scalar(&scalar, documentation_path, "agent_guidance")?,
        lookup_tags: required_list(&scalar, documentation_path, "lookup_tags")?,
        app_visible: required_bool(&scalar, documentation_path, "app_visible")?,
        help_visible: required_bool(&scalar, documentation_path, "help_visible")?,
        stability: required_scalar(&scalar, documentation_path, "stability")?,
        is_async: required_bool(&scalar, documentation_path, "async")?,
        documentation_path: documentation_path.to_string(),
    };

    validate_entry(&entry)?;
    Ok(entry)
}

fn frontmatter_text<'a>(text: &'a str, documentation_path: &str) -> RegistryResult<&'a str> {
    let rest = text.strip_prefix("---\n").ok_or_else(|| {
        RegistryError::new(format!(
            "{documentation_path} must start with YAML frontmatter"
        ))
    })?;
    rest.split_once("\n---")
        .map(|(frontmatter, _)| frontmatter)
        .ok_or_else(|| {
            RegistryError::new(format!(
                "{documentation_path} is missing closing frontmatter delimiter"
            ))
        })
}

fn parse_scalar_frontmatter(frontmatter: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for line in frontmatter.lines() {
        if line.starts_with(' ') || line.trim().is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            fields.insert(key.to_string(), clean_scalar(value));
        }
    }
    fields
}

fn parse_custom_properties(
    frontmatter: &str,
    documentation_path: &str,
) -> RegistryResult<Vec<CustomProperty>> {
    let lines: Vec<_> = frontmatter.lines().collect();
    let Some(start) = lines
        .iter()
        .position(|line| line.starts_with("custom_properties:"))
    else {
        return Err(RegistryError::new(format!(
            "{documentation_path} is missing custom_properties frontmatter"
        )));
    };

    if clean_scalar(
        lines[start]
            .split_once(':')
            .map(|(_, value)| value)
            .unwrap_or_default(),
    ) == "[]"
    {
        return Ok(Vec::new());
    }

    let mut properties = Vec::new();
    let mut current: BTreeMap<String, String> = BTreeMap::new();
    for line in lines.iter().skip(start + 1) {
        if !line.starts_with(' ') {
            break;
        }
        if let Some(rest) = line.strip_prefix("  - ") {
            if !current.is_empty() {
                properties.push(custom_property_from_map(&current, documentation_path)?);
                current.clear();
            }
            let (key, value) = rest.split_once(':').ok_or_else(|| {
                RegistryError::new(format!(
                    "{documentation_path} has malformed custom property line: {line}"
                ))
            })?;
            current.insert(key.to_string(), clean_scalar(value));
            continue;
        }
        if let Some(rest) = line.strip_prefix("    ") {
            let (key, value) = rest.split_once(':').ok_or_else(|| {
                RegistryError::new(format!(
                    "{documentation_path} has malformed custom property field: {line}"
                ))
            })?;
            current.insert(key.to_string(), clean_scalar(value));
        }
    }
    if !current.is_empty() {
        properties.push(custom_property_from_map(&current, documentation_path)?);
    }

    Ok(properties)
}

fn custom_property_from_map(
    fields: &BTreeMap<String, String>,
    documentation_path: &str,
) -> RegistryResult<CustomProperty> {
    let field = |key: &str| -> RegistryResult<String> {
        let value = fields.get(key).map(String::as_str).unwrap_or_default();
        if value.trim().is_empty() {
            return Err(RegistryError::new(format!(
                "{documentation_path} custom property is missing non-empty {key}"
            )));
        }
        Ok(value.to_string())
    };
    Ok(CustomProperty {
        name: field("name")?,
        property_type: field("type")?,
        default: field("default")?,
        description: field("description")?,
    })
}

fn required_scalar(
    fields: &BTreeMap<String, String>,
    documentation_path: &str,
    key: &str,
) -> RegistryResult<String> {
    let value = fields.get(key).map(String::as_str).unwrap_or_default();
    if value.trim().is_empty() || value == "[]" {
        return Err(RegistryError::new(format!(
            "{documentation_path} is missing non-empty frontmatter field {key}"
        )));
    }
    Ok(value.to_string())
}

fn required_list(
    fields: &BTreeMap<String, String>,
    documentation_path: &str,
    key: &str,
) -> RegistryResult<Vec<String>> {
    let value = fields.get(key).ok_or_else(|| {
        RegistryError::new(format!(
            "{documentation_path} is missing list frontmatter field {key}"
        ))
    })?;
    parse_inline_list(value).ok_or_else(|| {
        RegistryError::new(format!(
            "{documentation_path} frontmatter field {key} must be an inline list"
        ))
    })
}

fn required_bool(
    fields: &BTreeMap<String, String>,
    documentation_path: &str,
    key: &str,
) -> RegistryResult<bool> {
    match fields.get(key).map(String::as_str) {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        _ => Err(RegistryError::new(format!(
            "{documentation_path} frontmatter field {key} must be true or false"
        ))),
    }
}

fn validate_entry(entry: &RegistryEntry) -> RegistryResult<()> {
    if entry.kind != "clay-js-api" {
        return Err(RegistryError::new(format!(
            "{} kind must be clay-js-api, got {}",
            entry.documentation_path, entry.kind
        )));
    }
    if !entry.id.starts_with("clay.") {
        return Err(RegistryError::new(format!(
            "{} id must use clay.* namespace, got {}",
            entry.documentation_path, entry.id
        )));
    }
    if entry.visibility == "public" && entry.lookup_tags.is_empty() {
        return Err(RegistryError::new(format!(
            "{} public API must have lookup_tags",
            entry.id
        )));
    }
    for denied in DENIED_AUTHORITIES {
        if !entry.security.contains(denied) {
            return Err(RegistryError::new(format!(
                "{} security notes in {} must explicitly say the API does not grant {denied} authority",
                entry.id, entry.documentation_path
            )));
        }
    }
    Ok(())
}

fn parse_inline_list(value: &str) -> Option<Vec<String>> {
    let trimmed = value.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    Some(
        inner
            .split(',')
            .map(|item| clean_scalar(item))
            .filter(|item| !item.is_empty())
            .collect(),
    )
}

fn clean_scalar(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

fn write_entry_json(out: &mut String, entry: &RegistryEntry) {
    out.push_str("    {\n");
    write_string_field(out, 6, "id", &entry.id, true);
    write_string_field(out, 6, "kind", &entry.kind, true);
    write_string_field(
        out,
        6,
        "documentation_path",
        &entry.documentation_path,
        true,
    );
    write_string_field(out, 6, "js_module", &entry.js_module, true);
    write_string_field(out, 6, "js_export", &entry.js_export, true);
    write_string_field(out, 6, "js_facade", &entry.js_facade, true);
    write_string_field(out, 6, "backing_rust", &entry.backing_rust, true);
    write_string_field(out, 6, "deno_op", &entry.deno_op, true);
    write_string_field(out, 6, "deno_op_path", &entry.deno_op_path, true);
    write_string_field(out, 6, "name", &entry.name, true);
    write_string_field(out, 6, "user_facing_name", &entry.user_facing_name, true);
    write_string_field(out, 6, "summary", &entry.summary, true);
    write_string_field(out, 6, "owner", &entry.owner, true);
    write_string_field(out, 6, "phase", &entry.phase, true);
    write_string_field(out, 6, "visibility", &entry.visibility, true);
    write_string_array_field(out, 6, "permissions", &entry.permissions, true);
    write_string_array_field(out, 6, "key_bindings", &entry.key_bindings, true);
    write_custom_properties_field(out, entry, true);
    write_string_field(out, 6, "security", &entry.security, true);
    write_string_field(out, 6, "agent_guidance", &entry.agent_guidance, true);
    write_string_array_field(out, 6, "lookup_tags", &entry.lookup_tags, true);
    write_bool_field(out, 6, "app_visible", entry.app_visible, true);
    write_bool_field(out, 6, "help_visible", entry.help_visible, true);
    write_string_field(out, 6, "stability", &entry.stability, true);
    write_bool_field(out, 6, "async", entry.is_async, false);
    out.push_str("    }");
}

fn write_custom_properties_field(out: &mut String, entry: &RegistryEntry, comma: bool) {
    out.push_str("      \"custom_properties\": [");
    if entry.custom_properties.is_empty() {
        out.push(']');
    } else {
        out.push('\n');
        for (index, property) in entry.custom_properties.iter().enumerate() {
            if index > 0 {
                out.push_str(",\n");
            }
            out.push_str("        {\n");
            write_string_field(out, 10, "name", &property.name, true);
            write_string_field(out, 10, "type", &property.property_type, true);
            write_string_field(out, 10, "default", &property.default, true);
            write_string_field(out, 10, "description", &property.description, false);
            out.push_str("        }");
        }
        out.push_str("\n      ]");
    }
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn write_string_field(out: &mut String, indent: usize, key: &str, value: &str, comma: bool) {
    out.push_str(&" ".repeat(indent));
    out.push('"');
    out.push_str(key);
    out.push_str("\": ");
    write_json_string(out, value);
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn write_string_array_field(
    out: &mut String,
    indent: usize,
    key: &str,
    values: &[String],
    comma: bool,
) {
    out.push_str(&" ".repeat(indent));
    out.push('"');
    out.push_str(key);
    out.push_str("\": [");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        write_json_string(out, value);
    }
    out.push(']');
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn write_bool_field(out: &mut String, indent: usize, key: &str, value: bool, comma: bool) {
    out.push_str(&" ".repeat(indent));
    out.push('"');
    out.push_str(key);
    out.push_str("\": ");
    out.push_str(if value { "true" } else { "false" });
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn write_json_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if character.is_control() => {
                out.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => out.push(character),
        }
    }
    out.push('"');
}
