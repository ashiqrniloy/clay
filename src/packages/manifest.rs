use std::collections::HashSet;

use serde_json::{Value, json};

use crate::packages::permissions::{
    PackagePermission, PermissionValidationError, is_prohibited_authority, parse_permission,
};
use crate::perf::budgets::BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClayPackageManifest {
    pub name: String,
    pub version: String,
    pub clay: ClayPackageMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClayPackageMetadata {
    pub api_prefix: String,
    /// Declared `clay.preset`, if any. Permissions on this struct are the
    /// expanded set after preset fill-in.
    pub preset: Option<String>,
    pub permissions: Vec<PackagePermission>,
    pub modes: Vec<String>,
    /// Mode IDs declared under `clay.editorControl.modes`: the exact major
    /// modes in which this package may exercise `editor-control` authority
    /// (movement/selection/caret ops and the programmatic execution channel).
    /// May reference foreign modes (e.g. `core.code`); enforced per call.
    pub editor_control_modes: Vec<String>,
    pub entry: String,
    pub load_entry: Option<String>,
    pub graph: PackageGraphRelations,
    /// Owner-declared versioned extension points (`clay-extension-point-v1`).
    pub extension_points: Vec<crate::packages::extension_points::ExtensionPointDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageGraphRelations {
    pub depends_on: Vec<String>,
    pub extends: Vec<String>,
    pub disables: Vec<String>,
    pub replaces: Vec<String>,
    /// Structured mutation-bearing relation requests
    /// (`clay-package-relation-v1`) from `clay.extends`/`clay.imports`/
    /// `clay.overrides` object entries. Their `package` targets also appear in
    /// `extends` (extends entries) or `depends_on` (imports/overrides entries)
    /// so existing graph resolution and cycle detection cover them.
    pub relation_requests: Vec<crate::packages::extension_points::StructuredRelationRequest>,
}

impl PackageGraphRelations {
    pub fn requires_package_control(&self) -> bool {
        !self.disables.is_empty() || !self.replaces.is_empty()
    }

    pub fn all_targets(&self) -> impl Iterator<Item = &String> {
        self.depends_on
            .iter()
            .chain(self.extends.iter())
            .chain(self.disables.iter())
            .chain(self.replaces.iter())
    }
}

/// Known `clay.preset` names. Shorthand only — validation runs on the expanded set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackagePreset {
    CodeMode,
    ProseMode,
    LspBridge,
}

impl PackagePreset {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "code-mode" => Some(Self::CodeMode),
            "prose-mode" => Some(Self::ProseMode),
            "lsp-bridge" => Some(Self::LspBridge),
            _ => None,
        }
    }

    fn permissions(self) -> &'static [&'static str] {
        match self {
            Self::CodeMode | Self::ProseMode => &[
                "mode-registration",
                "mode-activation",
                "command-registration",
                "completion-provider",
                "parse-document",
                "render-decorations",
            ],
            // `language-server` is never implied. Packages still declare it in
            // `clay.capabilities` and it stays deny-by-default at grant time.
            Self::LspBridge => &[
                "parse-document",
                "completion-provider",
                "render-decorations",
            ],
        }
    }

    fn api_dependencies(self) -> &'static [&'static str] {
        match self {
            Self::CodeMode => &[
                "syntax.serverRegisterSyntaxGrammar",
                "modes.serverRegisterModePattern",
                "behavior.buildCodeEditingManifest",
                "commands.serverRegisterCommand",
                "completion.serverRegisterCompletionProvider",
                "completion.completionTriggerCharactersFromEditorRules",
                "ui.serverRegisterComponentContribution",
            ],
            Self::ProseMode => &[
                "syntax.serverRegisterSyntaxGrammar",
                "modes.serverRegisterModePattern",
                "modes.serverActivateMajorMode",
                "behavior.buildCodeEditingManifest",
                "commands.serverRegisterCommand",
                "completion.serverRegisterCompletionProvider",
                "completion.completionTriggerCharactersFromEditorRules",
                "parse.serverRegisterParseHandler",
                "decorations.serverPublishDecorations",
                "ui.serverRegisterComponentContribution",
            ],
            Self::LspBridge => &[
                "language.serverRegisterDocumentAnalyzer",
                "decorations.serverPublishDecorations",
                "diagnostics.serverPublishDiagnostics",
            ],
        }
    }

    fn extension_points(self, api_prefix: &str) -> Value {
        match self {
            Self::CodeMode => json!([
                {
                    "id": format!("{api_prefix}.completionProviders"),
                    "version": 1,
                    "operations": ["append", "replace"],
                    "contributionKinds": ["completionProvider"],
                    "summary": "Add or replace completion providers."
                },
                {
                    "id": format!("{api_prefix}.languageLayers"),
                    "version": 1,
                    "operations": ["append"],
                    "contributionKinds": ["analyzer", "diagnosticSource", "intelligenceProvider", "decorationLayer"],
                    "summary": "Append package-owned language layers."
                },
                {
                    "id": format!("{api_prefix}.commands"),
                    "version": 1,
                    "operations": ["append", "replace"],
                    "contributionKinds": ["command"],
                    "summary": "Add or replace commands."
                },
                {
                    "id": format!("{api_prefix}.ui"),
                    "version": 1,
                    "operations": ["append", "replace"],
                    "contributionKinds": ["componentContribution", "statusItem", "panelContribution"],
                    "summary": "Add or replace status and panel contributions."
                },
                {
                    "id": format!("{api_prefix}.grammar"),
                    "version": 1,
                    "operations": ["replace"],
                    "contributionKinds": ["grammar"],
                    "summary": "Replace the grammar, highlights query, and style map."
                },
                {
                    "id": format!("{api_prefix}.modePattern"),
                    "version": 1,
                    "operations": ["append"],
                    "contributionKinds": ["modePattern"],
                    "summary": "Extend file and mode patterns."
                }
            ]),
            Self::ProseMode => json!([
                {
                    "id": format!("{api_prefix}.completionProviders"),
                    "version": 1,
                    "operations": ["append", "replace"],
                    "contributionKinds": ["completionProvider"],
                    "summary": "Add or replace completion providers."
                },
                {
                    "id": format!("{api_prefix}.languageLayers"),
                    "version": 1,
                    "operations": ["append"],
                    "contributionKinds": ["decorationLayer", "diagnosticSource", "analyzer", "intelligenceProvider"],
                    "summary": "Append package-owned parse and language layers."
                },
                {
                    "id": format!("{api_prefix}.commands"),
                    "version": 1,
                    "operations": ["append", "replace"],
                    "contributionKinds": ["command", "keyRoute", "textTransform"],
                    "summary": "Add or replace commands, key routes, and text transforms."
                },
                {
                    "id": format!("{api_prefix}.ui"),
                    "version": 1,
                    "operations": ["append", "replace"],
                    "contributionKinds": ["panelContribution", "componentContribution", "sduiRegion", "statusItem"],
                    "summary": "Add or replace preview and status UI contributions."
                },
                {
                    "id": format!("{api_prefix}.grammar"),
                    "version": 1,
                    "operations": ["replace"],
                    "contributionKinds": ["grammar"],
                    "summary": "Replace the grammar, highlights query, and style map."
                },
                {
                    "id": format!("{api_prefix}.modePattern"),
                    "version": 1,
                    "operations": ["append"],
                    "contributionKinds": ["modePattern"],
                    "summary": "Extend file and mode patterns."
                }
            ]),
            Self::LspBridge => json!([
                {
                    "id": format!("{api_prefix}.providers"),
                    "version": 1,
                    "operations": ["append", "replace"],
                    "contributionKinds": ["completionProvider", "intelligenceProvider", "analyzer"],
                    "summary": "Add or replace language-server providers. The language-server descriptor and grant are not mutable."
                }
            ]),
        }
    }
}

fn union_string_array(clay: &mut serde_json::Map<String, Value>, key: &str, extras: &[&str]) {
    match clay.get_mut(key) {
        None => {
            clay.insert(
                key.to_string(),
                Value::Array(
                    extras
                        .iter()
                        .map(|value| Value::String((*value).to_string()))
                        .collect(),
                ),
            );
        }
        Some(Value::Array(values)) => {
            for extra in extras {
                if !values.iter().any(|value| value.as_str() == Some(extra)) {
                    values.push(Value::String((*extra).to_string()));
                }
            }
        }
        Some(_) => {}
    }
}

/// Expand `clay.preset` into missing permission / apiDependency / extension-point
/// keys. Explicit `permissions` replace the preset list. `apiDependencies` union
/// with the preset. Explicit `extensionPoints` replace the generated set.
/// `language-server` is never injected.
pub(crate) fn expand_capability_preset(
    value: &Value,
    context: &DiagnosticContext,
) -> Result<Value, PackageDiagnostic> {
    let mut expanded = value.clone();
    let Some(clay) = expanded.get_mut("clay").and_then(Value::as_object_mut) else {
        return Ok(expanded);
    };
    let Some(preset_value) = clay.get("preset") else {
        return Ok(expanded);
    };
    let Some(preset_name) = preset_value.as_str() else {
        return Err(context.diagnostic(
            PackageValidationRule::UnknownPreset,
            "clay.preset must be a string",
        ));
    };
    let Some(preset) = PackagePreset::parse(preset_name) else {
        return Err(context.diagnostic(
            PackageValidationRule::UnknownPreset,
            format!("unknown clay.preset `{preset_name}`"),
        ));
    };
    if !clay.contains_key("permissions") {
        clay.insert(
            "permissions".to_string(),
            Value::Array(
                preset
                    .permissions()
                    .iter()
                    .map(|permission| Value::String((*permission).to_string()))
                    .collect(),
            ),
        );
    }
    union_string_array(clay, "apiDependencies", preset.api_dependencies());
    if !clay.contains_key("extensionPoints")
        && let Some(api_prefix) = clay.get("apiPrefix").and_then(Value::as_str)
    {
        clay.insert(
            "extensionPoints".to_string(),
            preset.extension_points(api_prefix),
        );
    }
    Ok(expanded)
}

pub fn validate_manifest_value(value: &Value) -> Result<ClayPackageManifest, PackageDiagnostic> {
    let package_name = read_string(value, "name").unwrap_or_default();
    let package_version = read_string(value, "version").unwrap_or_default();
    let api_prefix = value
        .get("clay")
        .and_then(|clay| read_string(clay, "apiPrefix"));
    let context = DiagnosticContext::new(
        optional_non_empty(package_name.clone()),
        optional_non_empty(package_version.clone()),
        api_prefix.clone(),
    );

    let value = expand_capability_preset(value, &context)?;
    if payload_size(&value) > BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES {
        return Err(context.diagnostic(
            PackageValidationRule::PayloadTooLarge,
            "package metadata exceeds the primitive load-time payload budget",
        ));
    }

    if package_name.trim().is_empty() {
        return Err(context.diagnostic(
            PackageValidationRule::MissingField,
            "package manifest must include non-empty name",
        ));
    }
    if package_version.trim().is_empty() {
        return Err(context.diagnostic(
            PackageValidationRule::MissingField,
            "package manifest must include non-empty version",
        ));
    }
    if !is_semver_like(&package_version) {
        return Err(context.diagnostic(
            PackageValidationRule::InvalidVersion,
            "package version must use semver major.minor.patch syntax",
        ));
    }

    let clay = value
        .get("clay")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            context.diagnostic(
                PackageValidationRule::MissingField,
                "package manifest must include a clay metadata object",
            )
        })?;

    reject_forbidden_runtime_metadata(&value, &context)?;

    let api_prefix = api_prefix.ok_or_else(|| {
        context.diagnostic(
            PackageValidationRule::MissingField,
            "clay.apiPrefix must be declared",
        )
    })?;
    if !is_valid_api_prefix(&api_prefix) {
        return Err(DiagnosticContext::new(
            Some(package_name.clone()),
            Some(package_version.clone()),
            Some(api_prefix.clone()),
        )
        .diagnostic(
            PackageValidationRule::InvalidPrefix,
            "clay.apiPrefix must match ^[a-z][a-z0-9-]{1,31}$",
        ));
    }
    if is_reserved_core_domain_for_package(&api_prefix, &package_name) {
        return Err(DiagnosticContext::new(
            Some(package_name.clone()),
            Some(package_version.clone()),
            Some(api_prefix.clone()),
        )
        .diagnostic(
            PackageValidationRule::InvalidPrefix,
            "clay.apiPrefix claims a reserved Clay core API domain; package identifiers must use the package's own prefix",
        ));
    }

    let entry = required_string_field(clay.get("entry"), "clay.entry", &context)?;
    validate_entry_path(&entry, "clay.entry", &context)?;
    let load_entry = match clay.get("loadEntry") {
        Some(Value::String(load_entry)) => {
            validate_entry_path(load_entry, "clay.loadEntry", &context)?;
            Some(load_entry.clone())
        }
        Some(_) => {
            return Err(context.diagnostic(
                PackageValidationRule::InvalidEntry,
                "clay.loadEntry must be a string when present",
            ));
        }
        None => None,
    };

    let permissions = parse_requested_capabilities(clay, &context)?;
    let modes = parse_modes(clay.get("modes"), &api_prefix, &context)?;
    let editor_control_modes =
        parse_editor_control(clay.get("editorControl"), &permissions, &context)?;
    let graph = parse_graph_relations(clay, &api_prefix, &context)?;
    let extension_points = crate::packages::extension_points::parse_extension_points(
        clay.get("extensionPoints"),
        &api_prefix,
        &context,
    )?;

    Ok(ClayPackageManifest {
        name: package_name,
        version: package_version,
        clay: ClayPackageMetadata {
            api_prefix,
            preset: clay
                .get("preset")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            permissions,
            modes,
            editor_control_modes,
            entry,
            load_entry,
            graph,
            extension_points,
        },
    })
}

pub fn validate_manifest_values(
    values: &[Value],
) -> Result<Vec<ClayPackageManifest>, PackageDiagnostic> {
    let mut manifests = Vec::with_capacity(values.len());
    let mut prefixes = HashSet::new();

    for value in values {
        let manifest = validate_manifest_value(value)?;
        if !prefixes.insert(manifest.clay.api_prefix.clone()) {
            return Err(PackageDiagnostic {
                package_name: Some(manifest.name),
                package_version: Some(manifest.version),
                api_prefix: Some(manifest.clay.api_prefix),
                rule: PackageValidationRule::DuplicatePrefix,
                message: "clay.apiPrefix must be unique among enabled packages".to_string(),
            });
        }
        manifests.push(manifest);
    }

    Ok(manifests)
}

pub fn is_valid_api_prefix(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if value.len() < 2 || value.len() > 32 || !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

/// Reserved Clay core API domains. Core command/JS-API identifiers use the
/// bare `<domain>.<name>` shape (for example `shell.clientSplitPaneVertical`,
/// `editor.clientCopySelection`, `runtime.reloadConfiguration`); the legacy
/// `clay.<domain>.*` spelling is retired. Third-party packages must not claim
/// a core domain as their `clay.apiPrefix` — package-owned identifiers always
/// start with the package's own prefix (`<package>.<name>`), so a squatted
/// core domain would make core and package IDs indistinguishable. Bundled
/// first-party packages from the compiled inventory are exempt because their
/// provenance is pinned (e.g. `@clay/git` owns the `git` domain that the core
/// git JS APIs also live under).
pub const RESERVED_CORE_API_DOMAINS: &[&str] = &[
    "application",
    "behavior",
    "clay",
    "client",
    "clipboard",
    "commands",
    "completion",
    "configuration",
    "controlCenter",
    "core",
    "decorations",
    "default",
    "diagnostics",
    "dialog",
    "documents",
    "editor",
    "folding",
    "git",
    "graph",
    "keybindings",
    "language",
    "language-server",
    "modes",
    "packages",
    "parse",
    "runtime",
    "sdui",
    "shell",
    "syntax",
    "tabs",
    "text",
    "theme",
    "ui",
    "workspace",
];

/// A package may use a reserved core API domain as its `clay.apiPrefix` only
/// when it is a bundled first-party package from the compiled inventory
/// (exact name + pinned manifest fingerprint), never a third-party package.
pub(crate) fn is_reserved_core_domain_for_package(api_prefix: &str, package_name: &str) -> bool {
    RESERVED_CORE_API_DOMAINS.contains(&api_prefix)
        && crate::packages::bundled::bundled_entry(package_name).is_none()
}

fn parse_requested_capabilities(
    clay: &serde_json::Map<String, Value>,
    context: &DiagnosticContext,
) -> Result<Vec<PackagePermission>, PackageDiagnostic> {
    let permissions = clay.get("permissions");
    let capabilities = clay.get("capabilities");
    if permissions.is_none() && capabilities.is_none() {
        return Err(context.diagnostic(
            PackageValidationRule::MissingField,
            "clay.permissions or clay.capabilities must be an array of known capability strings",
        ));
    }

    let mut seen = HashSet::new();
    let mut parsed = Vec::new();
    if let Some(values) = permissions {
        parse_permission_array(values, "clay.permissions", &mut seen, &mut parsed, context)?;
    }
    if let Some(values) = capabilities {
        parse_permission_array(values, "clay.capabilities", &mut seen, &mut parsed, context)?;
    }
    Ok(parsed)
}

fn parse_permission_array(
    value: &Value,
    field_name: &str,
    seen: &mut HashSet<String>,
    permissions: &mut Vec<PackagePermission>,
    context: &DiagnosticContext,
) -> Result<(), PackageDiagnostic> {
    let Value::Array(values) = value else {
        return Err(context.diagnostic(
            PackageValidationRule::MissingField,
            format!("{field_name} must be an array of known capability strings"),
        ));
    };

    for value in values {
        let Some(permission) = value.as_str() else {
            return Err(context.diagnostic(
                PackageValidationRule::InvalidPermission,
                format!("{field_name} entries must be strings"),
            ));
        };
        if !seen.insert(permission.to_string()) {
            return Err(context.diagnostic(
                PackageValidationRule::DuplicatePermission,
                "clay.permissions/clay.capabilities entries must be unique",
            ));
        }
        if field_name == "clay.permissions" && is_prohibited_authority(permission) {
            return Err(context.diagnostic(
                PackageValidationRule::ProhibitedAuthority,
                format!("prohibited authority `{permission}` cannot be requested by default"),
            ));
        }
        match parse_permission(permission) {
            Ok(permission) => permissions.push(permission),
            Err(PermissionValidationError::UnknownPermission { .. }) => {
                return Err(context.diagnostic(
                    PackageValidationRule::UnknownPermission,
                    format!("unknown Clay package capability `{permission}`"),
                ));
            }
            Err(PermissionValidationError::ProhibitedAuthority { .. }) => {
                return Err(context.diagnostic(
                    PackageValidationRule::ProhibitedAuthority,
                    format!("prohibited authority `{permission}` cannot be requested by default"),
                ));
            }
        }
    }

    Ok(())
}

fn parse_graph_relations(
    clay: &serde_json::Map<String, Value>,
    api_prefix: &str,
    context: &DiagnosticContext,
) -> Result<PackageGraphRelations, PackageDiagnostic> {
    // `dependsOn`/`disables`/`replaces` stay plain name arrays.
    let mut depends_on = parse_relation_array(clay.get("dependsOn"), "clay.dependsOn", context)?;
    let mut extends = Vec::new();
    let mut relation_requests = Vec::new();
    // `extends` accepts legacy string targets or structured relation objects.
    parse_mixed_relation_array(
        clay.get("extends"),
        "extends",
        api_prefix,
        context,
        &mut extends,
        &mut relation_requests,
    )?;
    // `imports`/`overrides` are structured-relation fields; their targets are
    // recorded as depends_on so the requester enables only after its targets.
    for key in ["imports", "overrides"] {
        parse_mixed_relation_array(
            clay.get(key),
            key,
            api_prefix,
            context,
            &mut depends_on,
            &mut relation_requests,
        )?;
    }
    if relation_requests.len()
        > crate::packages::extension_points::MAX_RELATION_REQUESTS_PER_MANIFEST
    {
        return Err(context.diagnostic(
            PackageValidationRule::InvalidPackageGraph,
            format!(
                "structured relation requests support at most {} entries",
                crate::packages::extension_points::MAX_RELATION_REQUESTS_PER_MANIFEST
            ),
        ));
    }
    Ok(PackageGraphRelations {
        depends_on,
        extends,
        disables: parse_relation_array(clay.get("disables"), "clay.disables", context)?,
        replaces: parse_relation_array(clay.get("replaces"), "clay.replaces", context)?,
        relation_requests,
    })
}

/// Parse one relation field that accepts legacy string targets and structured
/// relation objects. String targets go to `targets`; object entries are
/// validated as `clay-package-relation-v1` requests and their `package`
/// target is appended to `targets` so graph resolution/cycle detection apply.
fn parse_mixed_relation_array(
    value: Option<&Value>,
    relation_key: &str,
    api_prefix: &str,
    context: &DiagnosticContext,
    targets: &mut Vec<String>,
    relation_requests: &mut Vec<crate::packages::extension_points::StructuredRelationRequest>,
) -> Result<(), PackageDiagnostic> {
    let Some(value) = value else {
        return Ok(());
    };
    let Value::Array(values) = value else {
        return Err(context.diagnostic(
            PackageValidationRule::InvalidPackageGraph,
            format!("clay.{relation_key} must be an array"),
        ));
    };
    for entry in values {
        match entry {
            Value::String(relation) => {
                let trimmed = relation.trim();
                if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
                    return Err(context.diagnostic(
                        PackageValidationRule::InvalidPackageGraph,
                        format!("clay.{relation_key} entries must be non-empty package specifiers"),
                    ));
                }
                if !targets.iter().any(|target| target == trimmed) {
                    targets.push(trimmed.to_string());
                }
            }
            Value::Object(object) => {
                let request = crate::packages::extension_points::parse_structured_relation(
                    object,
                    relation_key,
                    api_prefix,
                    context,
                )?;
                if !targets.iter().any(|target| target == &request.package) {
                    targets.push(request.package.clone());
                }
                relation_requests.push(request);
            }
            _ => {
                return Err(context.diagnostic(
                    PackageValidationRule::InvalidPackageGraph,
                    format!("clay.{relation_key} entries must be strings or relation objects"),
                ));
            }
        }
    }
    Ok(())
}

fn parse_relation_array(
    value: Option<&Value>,
    field_name: &str,
    context: &DiagnosticContext,
) -> Result<Vec<String>, PackageDiagnostic> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(values) = value else {
        return Err(context.diagnostic(
            PackageValidationRule::InvalidPackageGraph,
            format!("{field_name} must be an array of package specifier strings"),
        ));
    };

    let mut seen = HashSet::new();
    let mut relations = Vec::with_capacity(values.len());
    for value in values {
        let Some(relation) = value.as_str() else {
            return Err(context.diagnostic(
                PackageValidationRule::InvalidPackageGraph,
                format!("{field_name} entries must be strings"),
            ));
        };
        let trimmed = relation.trim();
        if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
            return Err(context.diagnostic(
                PackageValidationRule::InvalidPackageGraph,
                format!("{field_name} entries must be non-empty package specifiers"),
            ));
        }
        if !seen.insert(trimmed.to_string()) {
            return Err(context.diagnostic(
                PackageValidationRule::InvalidPackageGraph,
                format!("{field_name} entries must be unique"),
            ));
        }
        relations.push(trimmed.to_string());
    }
    Ok(relations)
}

/// Maximum number of mode IDs a package may declare under
/// `clay.editorControl.modes`.
pub const MAX_EDITOR_CONTROL_MODES: usize = 32;
/// Maximum byte length of one declared editor-control mode ID.
pub const MAX_EDITOR_CONTROL_MODE_ID_BYTES: usize = 128;

/// Parse `clay.editorControl` (optional). Shape is closed: only `modes` is
/// accepted, deny-by-default. Mode IDs are exact (no wildcards) and may
/// reference modes the package does not own; declaring the block requires the
/// approved-requested `editor-control` permission.
fn parse_editor_control(
    value: Option<&Value>,
    permissions: &[PackagePermission],
    context: &DiagnosticContext,
) -> Result<Vec<String>, PackageDiagnostic> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if !permissions.contains(&PackagePermission::EditorControl) {
        return Err(context.diagnostic(
            PackageValidationRule::InvalidPermission,
            "clay.editorControl requires the `editor-control` permission declaration",
        ));
    }
    let Some(object) = value.as_object() else {
        return Err(context.diagnostic(
            PackageValidationRule::MissingField,
            "clay.editorControl must be an object when present",
        ));
    };
    for key in object.keys() {
        if key != "modes" {
            return Err(context.diagnostic(
                PackageValidationRule::InvalidModeId,
                format!("unknown clay.editorControl key `{key}`; allowed keys are: modes"),
            ));
        }
    }
    let Some(Value::Array(values)) = object.get("modes") else {
        return Err(context.diagnostic(
            PackageValidationRule::MissingField,
            "clay.editorControl.modes must be an array of exact mode IDs",
        ));
    };
    if values.len() > MAX_EDITOR_CONTROL_MODES {
        return Err(context.diagnostic(
            PackageValidationRule::InvalidModeId,
            format!(
                "clay.editorControl.modes must contain at most {MAX_EDITOR_CONTROL_MODES} entries"
            ),
        ));
    }
    let mut seen = HashSet::new();
    let mut modes = Vec::new();
    for value in values {
        let Some(mode) = value.as_str() else {
            return Err(context.diagnostic(
                PackageValidationRule::InvalidModeId,
                "clay.editorControl.modes entries must be strings",
            ));
        };
        if mode.is_empty()
            || mode.len() > MAX_EDITOR_CONTROL_MODE_ID_BYTES
            || mode.chars().any(char::is_control)
            || mode.contains('*')
        {
            return Err(context.diagnostic(
                PackageValidationRule::InvalidModeId,
                "clay.editorControl.modes entries must be bounded exact mode IDs (no wildcards)",
            ));
        }
        if !seen.insert(mode.to_string()) {
            return Err(context.diagnostic(
                PackageValidationRule::DuplicateModeId,
                "clay.editorControl.modes entries must be unique within a package",
            ));
        }
        modes.push(mode.to_string());
    }
    Ok(modes)
}

fn parse_modes(
    value: Option<&Value>,
    api_prefix: &str,
    context: &DiagnosticContext,
) -> Result<Vec<String>, PackageDiagnostic> {
    let Some(Value::Array(values)) = value else {
        return Err(context.diagnostic(
            PackageValidationRule::MissingField,
            "clay.modes must be an array of declared mode IDs",
        ));
    };

    let mut seen = HashSet::new();
    let mut modes = Vec::new();
    for value in values {
        let Some(mode) = value.as_str() else {
            return Err(context.diagnostic(
                PackageValidationRule::InvalidModeId,
                "clay.modes entries must be strings",
            ));
        };
        if mode.starts_with("clay.") {
            return Err(context.diagnostic(
                PackageValidationRule::ReservedClayId,
                "package-owned mode IDs cannot use the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(mode, api_prefix) {
            return Err(context.diagnostic(
                PackageValidationRule::InvalidModeId,
                "mode IDs must use the package apiPrefix or apiPrefix.* namespace",
            ));
        }
        if !seen.insert(mode.to_string()) {
            return Err(context.diagnostic(
                PackageValidationRule::DuplicateModeId,
                "clay.modes entries must be unique within a package",
            ));
        }
        modes.push(mode.to_string());
    }

    Ok(modes)
}

fn is_package_owned_id(value: &str, api_prefix: &str) -> bool {
    value == api_prefix
        || value
            .strip_prefix(api_prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

fn reject_forbidden_runtime_metadata(
    value: &Value,
    context: &DiagnosticContext,
) -> Result<(), PackageDiagnostic> {
    match value {
        Value::String(text) if text.contains("Deno.core.ops") => Err(context.diagnostic(
            PackageValidationRule::RawDenoOpsExposure,
            "package metadata must not expose raw Deno.core.ops names",
        )),
        Value::Object(object) => {
            for (key, nested) in object {
                if matches!(
                    key.as_str(),
                    "clientHook"
                        | "clientJavaScript"
                        | "drawCallback"
                        | "widgetCallback"
                        | "rawOps"
                ) {
                    return Err(context.diagnostic(
                        PackageValidationRule::ClientJavaScriptHook,
                        "package metadata must not declare client-side JavaScript hooks or raw op fields",
                    ));
                }
                reject_forbidden_runtime_metadata(nested, context)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_forbidden_runtime_metadata(nested, context)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_entry_path(
    entry: &str,
    field: &str,
    context: &DiagnosticContext,
) -> Result<(), PackageDiagnostic> {
    let valid = entry.starts_with("./")
        && entry.ends_with(".js")
        && !entry.contains('\\')
        && !entry.contains("Deno.core.ops")
        && entry[2..]
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if !valid {
        return Err(context.diagnostic(
            PackageValidationRule::InvalidEntry,
            format!("{field} must be a relative ./ module path ending in .js without traversal"),
        ));
    }
    Ok(())
}

fn required_string_field(
    value: Option<&Value>,
    field: &str,
    context: &DiagnosticContext,
) -> Result<String, PackageDiagnostic> {
    match value {
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(text.clone()),
        _ => Err(context.diagnostic(
            PackageValidationRule::MissingField,
            format!("{field} must be a non-empty string"),
        )),
    }
}

fn read_string(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(ToOwned::to_owned)
}

fn optional_non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn payload_size(value: &Value) -> usize {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX)
}

fn is_semver_like(value: &str) -> bool {
    let core = value.split_once('-').map_or(value, |(core, _)| core);
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDiagnostic {
    pub package_name: Option<String>,
    pub package_version: Option<String>,
    pub api_prefix: Option<String>,
    pub rule: PackageValidationRule,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageValidationRule {
    MissingField,
    InvalidVersion,
    InvalidPrefix,
    DuplicatePrefix,
    PayloadTooLarge,
    UnknownPermission,
    InvalidPermission,
    DuplicatePermission,
    ProhibitedAuthority,
    ReservedClayId,
    InvalidModeId,
    DuplicateModeId,
    InvalidEntry,
    InvalidPackageGraph,
    RawDenoOpsExposure,
    ClientJavaScriptHook,
    UnknownPreset,
}

pub(crate) struct DiagnosticContext {
    package_name: Option<String>,
    package_version: Option<String>,
    api_prefix: Option<String>,
}

impl DiagnosticContext {
    pub(crate) fn new(
        package_name: Option<String>,
        package_version: Option<String>,
        api_prefix: Option<String>,
    ) -> Self {
        Self {
            package_name,
            package_version,
            api_prefix,
        }
    }

    pub(crate) fn diagnostic(
        &self,
        rule: PackageValidationRule,
        message: impl Into<String>,
    ) -> PackageDiagnostic {
        PackageDiagnostic {
            package_name: self.package_name.clone(),
            package_version: self.package_version.clone(),
            api_prefix: self.api_prefix.clone(),
            rule,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> DiagnosticContext {
        DiagnosticContext::new(
            Some("@clay/demo".to_string()),
            Some("0.1.0".to_string()),
            Some("demo".to_string()),
        )
    }

    fn code_mode_package() -> Value {
        json!({
            "name": "@clay/demo",
            "version": "0.1.0",
            "clay": {
                "apiPrefix": "demo",
                "preset": "code-mode",
                "entry": "./dist/index.js",
                "modes": ["demo"]
            }
        })
    }

    #[test]
    fn code_mode_preset_expands_permissions_and_extension_points() {
        let manifest = validate_manifest_value(&code_mode_package()).unwrap();
        assert_eq!(manifest.clay.preset.as_deref(), Some("code-mode"));
        assert_eq!(
            manifest
                .clay
                .permissions
                .iter()
                .map(|permission| permission.as_str())
                .collect::<Vec<_>>(),
            vec![
                "mode-registration",
                "mode-activation",
                "command-registration",
                "completion-provider",
                "parse-document",
                "render-decorations",
            ]
        );
        assert!(
            manifest
                .clay
                .permissions
                .iter()
                .all(|permission| { *permission != PackagePermission::LanguageServer })
        );
        assert!(
            manifest
                .clay
                .extension_points
                .iter()
                .any(|point| point.id == "demo.grammar")
        );
    }

    #[test]
    fn explicit_permissions_replace_preset_defaults() {
        let mut value = code_mode_package();
        value["clay"]["permissions"] = json!(["mode-registration", "package-configuration"]);
        let manifest = validate_manifest_value(&value).unwrap();
        assert_eq!(
            manifest
                .clay
                .permissions
                .iter()
                .map(|permission| permission.as_str())
                .collect::<Vec<_>>(),
            vec!["mode-registration", "package-configuration"]
        );
    }

    #[test]
    fn unknown_preset_fails_validation() {
        let mut value = code_mode_package();
        value["clay"]["preset"] = json!("theme-pack");
        let err = validate_manifest_value(&value).unwrap_err();
        assert_eq!(err.rule, PackageValidationRule::UnknownPreset);
        assert!(err.message.contains("theme-pack"));
    }

    #[test]
    fn lsp_bridge_preset_does_not_request_language_server() {
        let value = json!({
            "name": "@clay/lsp-demo",
            "version": "0.1.0",
            "clay": {
                "apiPrefix": "lsp-demo",
                "preset": "lsp-bridge",
                "entry": "./dist/index.js",
                "modes": []
            }
        });
        let manifest = validate_manifest_value(&value).unwrap();
        assert_eq!(manifest.clay.preset.as_deref(), Some("lsp-bridge"));
        assert!(
            !manifest
                .clay
                .permissions
                .contains(&PackagePermission::LanguageServer)
        );
        assert_eq!(
            manifest
                .clay
                .permissions
                .iter()
                .map(|permission| permission.as_str())
                .collect::<Vec<_>>(),
            vec![
                "parse-document",
                "completion-provider",
                "render-decorations"
            ]
        );
    }

    #[test]
    fn preset_expansion_counts_toward_payload_budget() {
        let value = code_mode_package();
        let expanded = expand_capability_preset(&value, &ctx()).unwrap();
        let unexpanded_len = payload_size(&value);
        let expanded_len = payload_size(&expanded);
        assert!(expanded_len > unexpanded_len);

        let pad = BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES - expanded_len + 1;
        let mut padded = value;
        padded["padding"] = json!("x".repeat(pad));
        assert!(payload_size(&padded) <= BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES);
        let err = validate_manifest_value(&padded).unwrap_err();
        assert_eq!(err.rule, PackageValidationRule::PayloadTooLarge);
    }
}
