/// Package enable/load contract assembler for Phase 17.
///
/// `PackageRecord` is the full typed representation of a package that has passed
/// Clay-owned enable/load validation.  It is built by [`assemble_package_record`]
/// from a raw `package.json`-shaped [`serde_json::Value`], reusing the Phase 16.5
/// validators in [`crate::packages::manifest`], [`crate::packages::permissions`],
/// and [`crate::packages::commands`] rather than duplicating them.
///
/// Enable/load validation runs only at install/enable/reload time and is never
/// called from typing, paint, layout, scroll, or text-event handlers.
use std::collections::HashSet;

use serde_json::Value;

use crate::packages::manifest::{ClayPackageManifest, PackageDiagnostic, validate_manifest_value};
use crate::packages::permissions::PackagePermission;
use crate::perf::budgets::{
    BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES,
    SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
};

// ── Contribution descriptors ─────────────────────────────────────────────────

/// Inert descriptor for a command contribution declared by a package.
///
/// This is manifest-level metadata only; it does not grant handler authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandContributionDescriptor {
    /// Package-prefixed command ID (e.g. `markdown.togglePreview`).
    pub id: String,
    /// Human-readable label for the command palette, help, and AI-agent lookup.
    pub display_name: String,
    /// Routing policy declared by the package for this command.
    pub routing_policy: String,
}

/// Inert descriptor for a configuration key contribution declared by a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigurationContributionDescriptor {
    /// Package-prefixed configuration key (e.g. `markdown.preview.enabled`).
    pub key: String,
    /// JSON schema type: `"boolean"`, `"string"`, `"number"`, `"integer"`.
    pub value_type: String,
    /// Serialized JSON default value.
    pub default_value: Option<String>,
}

/// Inert descriptor for a key-routing override declared by a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRoutingContributionDescriptor {
    /// Package-prefixed command ID that this key binding targets.
    pub command_id: String,
    /// Optional deterministic key binding token, e.g. `Ctrl+Shift+P`.
    pub key_binding: Option<String>,
    /// Optional routing policy used for ambiguity checks when a key is declared.
    pub routing_policy: Option<String>,
    /// Optional explicit priority. Equal key/routing/priority entries conflict.
    pub priority: Option<i32>,
}

/// Inert descriptor for a text-transform rule declared by a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextTransformContributionDescriptor {
    /// Package-prefixed transform ID (e.g. `markdown.list-continuation`).
    pub transform_id: String,
    /// Known transform kind: `"enter-rule"`, `"tab-rule"`, `"pair-rule"`,
    /// `"comment-continuation"`, or `"autocomplete-trigger"`.
    pub kind: String,
}

/// Inert descriptor for an SDUI/status-bar contribution declared by a package.
///
/// SDUI actions embedded in the contribution must target declared commands;
/// they inherit the command permissions at execution time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SduiContributionDescriptor {
    /// Package-prefixed region/slot identifier.
    pub region_id: String,
    /// Display label for conflict diagnostics and AI-agent lookup.
    pub display_name: String,
    /// Estimated full snapshot payload for this inert package SDUI contribution.
    pub estimated_snapshot_bytes: usize,
    /// Estimated update payload for this inert package SDUI contribution.
    pub estimated_update_bytes: usize,
}

/// Inert descriptor for a decoration/render primitive declared by a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorationContributionDescriptor {
    /// Package-prefixed decoration/render primitive ID.
    pub primitive_id: String,
    /// Known decoration kind or style token namespace.
    pub kind: String,
}

/// All inert primitive contribution descriptors declared by a package.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageContributions {
    pub commands: Vec<CommandContributionDescriptor>,
    pub configuration: Vec<ConfigurationContributionDescriptor>,
    pub key_routing: Vec<KeyRoutingContributionDescriptor>,
    pub text_transforms: Vec<TextTransformContributionDescriptor>,
    pub sdui: Vec<SduiContributionDescriptor>,
    pub decorations: Vec<DecorationContributionDescriptor>,
}

// ── Documentation and performance metadata ───────────────────────────────────

/// Path to the package's Clay JS API documentation index, declared in the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDocsMetadata {
    /// Relative path to the docs entry point (e.g. `./docs/index.md`).
    pub docs_path: String,
}

/// Performance metadata for the package's contribution to static payload budgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePerformanceMetadata {
    /// Estimated static manifest payload size in bytes, checked against
    /// [`crate::perf::budgets::BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES`].
    pub estimated_manifest_bytes: usize,
}

/// A declared Clay JS API dependency of the package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageApiDependency {
    /// Stable Clay JS API ID (e.g. `clay.modes.serverRegisterModePattern`).
    pub api_id: String,
}

// ── Package record ───────────────────────────────────────────────────────────

/// Full enable/load record for a validated Clay package.
///
/// A `PackageRecord` is produced by [`assemble_package_record`] only after all
/// Clay-owned validation rules pass.  It retains provenance on every accepted
/// contribution descriptor so later conflict handling, diagnostics, generated
/// documentation, and AI-agent discovery can identify the owning package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRecord {
    /// The validated Phase 16.5 manifest (identity, prefix, permissions, modes, entries).
    pub manifest: ClayPackageManifest,
    /// Declared inert primitive contributions.
    pub contributions: PackageContributions,
    /// Documentation metadata path, required for every enabled package.
    pub docs: PackageDocsMetadata,
    /// Static performance metadata, checked against payload budgets at enable time.
    pub performance: PackagePerformanceMetadata,
    /// Declared Clay JS API dependencies.
    pub api_dependencies: Vec<PackageApiDependency>,
}

// ── Error types ──────────────────────────────────────────────────────────────

/// Structured enable/load diagnostic produced when a package fails contract
/// validation.  Every field is optional because some errors occur before the
/// package identity is fully parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRecordError {
    pub package_name: Option<String>,
    pub package_version: Option<String>,
    pub api_prefix: Option<String>,
    /// The contribution ID that failed (command, config key, transform, …) when applicable.
    pub contribution_id: Option<String>,
    pub rule: PackageRecordRule,
    pub message: String,
}

/// Validation rule that caused a [`PackageRecordError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageRecordRule {
    /// A required field (entry, docs, performance, contributions block) is absent.
    MissingRequiredField,
    /// The static payload estimate exceeds an advisory budget constant.
    PayloadBudgetExceeded,
    /// A contribution descriptor is malformed or references an undeclared ID.
    InvalidContributionDescriptor,
    /// A contribution requires a permission not declared in `clay.permissions`.
    UndeclaredPermissionForContribution,
    /// A contribution ID claims the reserved `clay.*` namespace.
    ReservedClayIdInContribution,
    /// A duplicate contribution ID (command, config key, region …) within the package.
    DuplicateContributionId,
    /// A declared API dependency ID is empty or malformed.
    InvalidApiDependency,
    /// Re-exported from the manifest layer.
    ManifestValidationFailed,
}

impl PackageRecordError {
    fn from_manifest_diagnostic(d: PackageDiagnostic) -> Self {
        Self {
            package_name: d.package_name,
            package_version: d.package_version,
            api_prefix: d.api_prefix,
            contribution_id: None,
            rule: PackageRecordRule::ManifestValidationFailed,
            message: d.message,
        }
    }
}

// ── Assembler ────────────────────────────────────────────────────────────────

/// Assemble a [`PackageRecord`] from a raw `package.json`-shaped JSON value.
///
/// This is the Clay-owned enable/load contract validator.  It runs at
/// install/enable/reload time and must never be called from typing, paint,
/// layout, scroll, or text-event handlers.
///
/// Steps:
/// 1. Validate the Phase 16.5 manifest (identity, prefix, permissions, modes).
/// 2. Parse and validate contribution descriptors from `clay.contributions`.
/// 3. Validate the required `clay.docs` path.
/// 4. Validate `clay.performance` metadata against advisory budgets.
/// 5. Parse `clay.apiDependencies` stubs.
pub fn assemble_package_record(value: &Value) -> Result<PackageRecord, PackageRecordError> {
    // Step 1: reuse Phase 16.5 manifest validator.
    let manifest =
        validate_manifest_value(value).map_err(PackageRecordError::from_manifest_diagnostic)?;

    let clay = value
        .get("clay")
        .and_then(Value::as_object)
        .expect("clay block already validated by validate_manifest_value");

    let ctx = ErrorContext {
        package_name: Some(manifest.name.clone()),
        package_version: Some(manifest.version.clone()),
        api_prefix: Some(manifest.clay.api_prefix.clone()),
    };

    // Step 2: contribution descriptors (optional block; empty = no contributions).
    let contributions = match clay.get("contributions") {
        Some(contrib_value) => parse_contributions(
            contrib_value,
            &manifest.clay.api_prefix,
            &manifest.clay.permissions,
            &ctx,
        )?,
        None => PackageContributions::default(),
    };

    // Step 3: required docs path.
    let docs = parse_docs_metadata(clay.get("docs"), &ctx)?;

    // Step 4: performance metadata.
    let performance = parse_performance_metadata(clay.get("performance"), value, &ctx)?;

    // Step 5: API dependencies (optional).
    let api_dependencies = parse_api_dependencies(clay.get("apiDependencies"), &ctx)?;
    validate_api_dependency_permissions(&api_dependencies, &manifest.clay.permissions, &ctx)?;

    Ok(PackageRecord {
        manifest,
        contributions,
        docs,
        performance,
        api_dependencies,
    })
}

// ── Internal parsers ─────────────────────────────────────────────────────────

fn parse_docs_metadata(
    value: Option<&Value>,
    ctx: &ErrorContext,
) -> Result<PackageDocsMetadata, PackageRecordError> {
    match value {
        Some(Value::String(path)) if !path.trim().is_empty() => Ok(PackageDocsMetadata {
            docs_path: path.clone(),
        }),
        _ => Err(ctx.error(
            PackageRecordRule::MissingRequiredField,
            None,
            "clay.docs must be a non-empty path to the package documentation index",
        )),
    }
}

fn parse_performance_metadata(
    value: Option<&Value>,
    raw_manifest: &Value,
    ctx: &ErrorContext,
) -> Result<PackagePerformanceMetadata, PackageRecordError> {
    let estimated_manifest_bytes = match value {
        Some(Value::Object(perf)) => {
            match perf.get("estimatedManifestBytes") {
                Some(Value::Number(n)) => n
                    .as_u64()
                    .map(|v| v as usize)
                    .ok_or_else(|| {
                        ctx.error(
                            PackageRecordRule::MissingRequiredField,
                            None,
                            "clay.performance.estimatedManifestBytes must be a non-negative integer",
                        )
                    })?,
                _ => return Err(ctx.error(
                    PackageRecordRule::MissingRequiredField,
                    None,
                    "clay.performance.estimatedManifestBytes must be declared as a non-negative integer",
                )),
            }
        }
        // When no performance block is present, derive the estimate from the raw payload size.
        None => {
            serde_json::to_vec(raw_manifest)
                .map(|bytes| bytes.len())
                .unwrap_or(usize::MAX)
        }
        _ => return Err(ctx.error(
            PackageRecordRule::MissingRequiredField,
            None,
            "clay.performance must be an object when present",
        )),
    };

    if estimated_manifest_bytes > BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES {
        return Err(ctx.error(
            PackageRecordRule::PayloadBudgetExceeded,
            None,
            format!(
                "package estimated manifest payload ({estimated_manifest_bytes} bytes) exceeds \
                 BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES ({BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES} bytes)"
            ),
        ));
    }

    Ok(PackagePerformanceMetadata {
        estimated_manifest_bytes,
    })
}

fn parse_api_dependencies(
    value: Option<&Value>,
    ctx: &ErrorContext,
) -> Result<Vec<PackageApiDependency>, PackageRecordError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidApiDependency,
            None,
            "clay.apiDependencies must be an array of Clay JS API ID strings when present",
        ));
    };
    let mut deps = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some(api_id) = entry.as_str() else {
            return Err(ctx.error(
                PackageRecordRule::InvalidApiDependency,
                None,
                "clay.apiDependencies entries must be strings",
            ));
        };
        if api_id.trim().is_empty() {
            return Err(ctx.error(
                PackageRecordRule::InvalidApiDependency,
                None,
                "clay.apiDependencies entries must be non-empty strings",
            ));
        }
        deps.push(PackageApiDependency {
            api_id: api_id.to_string(),
        });
    }
    Ok(deps)
}

fn validate_api_dependency_permissions(
    dependencies: &[PackageApiDependency],
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    for dependency in dependencies {
        let required = match dependency.api_id.as_str() {
            "clay.modes.serverRegisterModePattern" => Some(PackagePermission::ModeRegistration),
            "clay.modes.serverActivateMajorMode" => Some(PackagePermission::ModeActivation),
            "clay.commands.serverRegisterCommand" => Some(PackagePermission::CommandRegistration),
            "clay.parse.serverRegisterParseHandler" => Some(PackagePermission::ParseDocument),
            "clay.decorations.serverPublishDecorations" => {
                Some(PackagePermission::RenderDecorations)
            }
            _ => None,
        };

        if let Some(required) = required {
            if !permissions.contains(&required) {
                return Err(ctx.error(
                    PackageRecordRule::UndeclaredPermissionForContribution,
                    Some(&dependency.api_id),
                    format!(
                        "API dependency `{}` requires the `{}` permission to be declared in clay.permissions",
                        dependency.api_id,
                        required.as_str()
                    ),
                ));
            }
        }
    }

    Ok(())
}

fn parse_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<PackageContributions, PackageRecordError> {
    let Value::Object(map) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions must be an object when present",
        ));
    };

    let commands = match map.get("commands") {
        Some(v) => parse_command_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let configuration = match map.get("configuration") {
        Some(v) => parse_configuration_contributions(v, api_prefix, permissions, ctx)?,
        None => Vec::new(),
    };
    let key_routing = match map.get("keyRouting") {
        Some(v) => parse_key_routing_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let text_transforms = match map.get("textTransforms") {
        Some(v) => parse_text_transform_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let sdui = match map.get("sdui") {
        Some(v) => parse_sdui_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };
    let decorations = match map.get("decorations") {
        Some(v) => parse_decoration_contributions(v, api_prefix, ctx)?,
        None => Vec::new(),
    };

    Ok(PackageContributions {
        commands,
        configuration,
        key_routing,
        text_transforms,
        sdui,
        decorations,
    })
}

fn parse_command_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<CommandContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.commands must be an array",
        ));
    };

    // Commands require `command-registration` permission.
    if !entries.is_empty() && !permissions.contains(&PackagePermission::CommandRegistration) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "command contributions require the `command-registration` permission to be declared in clay.permissions",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "command contribution entries must be objects",
            )
        })?;

        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "command contribution must include a non-empty `id` field",
                )
            })?;

        if id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(id),
                "command contribution IDs cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "command contribution IDs must use the package apiPrefix or apiPrefix.* namespace",
            ));
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "command contribution IDs must be unique within a package",
            ));
        }

        let display_name = obj
            .get("displayName")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "command contribution must include a non-empty `displayName` field",
                )
            })?;

        let routing_policy = obj
            .get("routingPolicy")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "command contribution must include a non-empty `routingPolicy` field",
                )
            })?;

        // Reject routing policies that would grant built-in client-edit authority.
        if matches!(
            routing_policy,
            "client-first-predictable" | "client-first-requires-ack"
        ) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "package command contributions cannot declare built-in client-edit routing policies",
            ));
        }

        descriptors.push(CommandContributionDescriptor {
            id: id.to_string(),
            display_name: display_name.to_string(),
            routing_policy: routing_policy.to_string(),
        });
    }

    Ok(descriptors)
}

fn parse_configuration_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<ConfigurationContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.configuration must be an array",
        ));
    };

    // Behavior-changing configuration requires `package-configuration`.
    if !entries.is_empty() && !permissions.contains(&PackagePermission::PackageConfiguration) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "configuration contributions require the `package-configuration` permission to be declared in clay.permissions",
        ));
    }

    let mut seen_keys = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "configuration contribution entries must be objects",
            )
        })?;

        let key = obj
            .get("key")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "configuration contribution must include a non-empty `key` field",
                )
            })?;

        if key.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(key),
                "configuration contribution keys cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(key, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(key),
                "configuration keys must use the package apiPrefix or apiPrefix.* namespace",
            ));
        }
        if !seen_keys.insert(key.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(key),
                "configuration contribution keys must be unique within a package",
            ));
        }

        let value_type = obj
            .get("type")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(key),
                    "configuration contribution must include a `type` field",
                )
            })?;

        if !matches!(value_type, "boolean" | "string" | "number" | "integer") {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(key),
                "configuration contribution `type` must be one of: boolean, string, number, integer",
            ));
        }

        let default_value = obj
            .get("default")
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        descriptors.push(ConfigurationContributionDescriptor {
            key: key.to_string(),
            value_type: value_type.to_string(),
            default_value,
        });
    }

    Ok(descriptors)
}

fn parse_key_routing_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<KeyRoutingContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.keyRouting must be an array",
        ));
    };

    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "key routing contribution entries must be objects",
            )
        })?;

        let command_id = obj
            .get("commandId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "key routing contribution must include a non-empty `commandId` field",
                )
            })?;

        if command_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(command_id),
                "key routing contributions cannot target reserved clay.* command IDs",
            ));
        }
        if !is_package_owned_id(command_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(command_id),
                "key routing contribution commandId must use the package apiPrefix namespace",
            ));
        }

        let key_binding = obj
            .get("key")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(ToOwned::to_owned);
        let routing_policy = obj
            .get("routingPolicy")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(ToOwned::to_owned);
        let priority = match obj.get("priority") {
            Some(Value::Number(n)) => n.as_i64().map(|v| v as i32),
            Some(_) => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(command_id),
                    "key routing contribution priority must be an integer when present",
                ));
            }
            None => None,
        };

        descriptors.push(KeyRoutingContributionDescriptor {
            command_id: command_id.to_string(),
            key_binding,
            routing_policy,
            priority,
        });
    }

    Ok(descriptors)
}

fn parse_text_transform_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<TextTransformContributionDescriptor>, PackageRecordError> {
    const VALID_KINDS: &[&str] = &[
        "enter-rule",
        "tab-rule",
        "pair-rule",
        "comment-continuation",
        "autocomplete-trigger",
    ];

    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.textTransforms must be an array",
        ));
    };

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "text transform contribution entries must be objects",
            )
        })?;

        // Reject any executable fields.
        for forbidden in &["javascriptCallback", "code", "clientHook", "drawCallback"] {
            if obj.contains_key(*forbidden) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    format!(
                        "text transform contributions are inert manifest data and must not include `{forbidden}`"
                    ),
                ));
            }
        }

        let transform_id = obj
            .get("transformId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "text transform contribution must include a non-empty `transformId` field",
                )
            })?;

        if transform_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(transform_id),
                "text transform IDs cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(transform_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(transform_id),
                "text transform IDs must use the package apiPrefix namespace",
            ));
        }
        if !seen_ids.insert(transform_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(transform_id),
                "text transform IDs must be unique within a package",
            ));
        }

        let kind = obj
            .get("kind")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(transform_id),
                    "text transform contribution must include a `kind` field",
                )
            })?;

        if !VALID_KINDS.contains(&kind) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(transform_id),
                format!(
                    "text transform `kind` must be one of: {}",
                    VALID_KINDS.join(", ")
                ),
            ));
        }

        descriptors.push(TextTransformContributionDescriptor {
            transform_id: transform_id.to_string(),
            kind: kind.to_string(),
        });
    }

    Ok(descriptors)
}

fn parse_sdui_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<SduiContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.sdui must be an array",
        ));
    };

    let mut seen_regions = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "SDUI contribution entries must be objects",
            )
        })?;

        // Reject executable widget fields.
        for forbidden in &[
            "widgetCallback",
            "clientJavaScript",
            "drawCallback",
            "nativeHandle",
        ] {
            if obj.contains_key(*forbidden) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    format!(
                        "SDUI contributions must not include client-side or native-widget fields (`{forbidden}`)"
                    ),
                ));
            }
        }

        let region_id = obj
            .get("regionId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "SDUI contribution must include a non-empty `regionId` field",
                )
            })?;

        if region_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(region_id),
                "SDUI contribution regionIds cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(region_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(region_id),
                "SDUI contribution regionIds must use the package apiPrefix namespace",
            ));
        }
        if !seen_regions.insert(region_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(region_id),
                "SDUI contribution regionIds must be unique within a package",
            ));
        }

        let display_name = obj
            .get("displayName")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(region_id),
                    "SDUI contribution must include a non-empty `displayName` field",
                )
            })?;

        let estimated_snapshot_bytes = obj
            .get("estimatedSnapshotBytes")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or_else(|| {
                serde_json::to_vec(entry)
                    .map(|bytes| bytes.len())
                    .unwrap_or(usize::MAX)
            });
        if estimated_snapshot_bytes > SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                Some(region_id),
                format!(
                    "SDUI snapshot payload estimate ({estimated_snapshot_bytes} bytes) exceeds SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES ({SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }

        let estimated_update_bytes = obj
            .get("estimatedUpdateBytes")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(estimated_snapshot_bytes);
        if estimated_update_bytes > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                Some(region_id),
                format!(
                    "SDUI update payload estimate ({estimated_update_bytes} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }

        descriptors.push(SduiContributionDescriptor {
            region_id: region_id.to_string(),
            display_name: display_name.to_string(),
            estimated_snapshot_bytes,
            estimated_update_bytes,
        });
    }

    Ok(descriptors)
}

fn parse_decoration_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<DecorationContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.decorations must be an array",
        ));
    };

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "decoration contribution entries must be objects",
            )
        })?;
        for forbidden in &["drawCallback", "clientJavaScript", "nativeHandle"] {
            if obj.contains_key(*forbidden) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    format!(
                        "decoration contributions are inert and must not include `{forbidden}`"
                    ),
                ));
            }
        }
        let primitive_id = obj
            .get("primitiveId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "decoration contribution must include a non-empty `primitiveId` field",
                )
            })?;
        if primitive_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(primitive_id),
                "decoration primitive IDs cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(primitive_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(primitive_id),
                "decoration primitive IDs must use the package apiPrefix namespace",
            ));
        }
        if !seen_ids.insert(primitive_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(primitive_id),
                "decoration primitive IDs must be unique within a package",
            ));
        }
        let kind = obj
            .get("kind")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(primitive_id),
                    "decoration contribution must include a non-empty `kind` field",
                )
            })?;
        descriptors.push(DecorationContributionDescriptor {
            primitive_id: primitive_id.to_string(),
            kind: kind.to_string(),
        });
    }

    Ok(descriptors)
}

// ── Utility ──────────────────────────────────────────────────────────────────

fn is_package_owned_id(value: &str, api_prefix: &str) -> bool {
    value == api_prefix
        || value
            .strip_prefix(api_prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

struct ErrorContext {
    package_name: Option<String>,
    package_version: Option<String>,
    api_prefix: Option<String>,
}

impl ErrorContext {
    fn error(
        &self,
        rule: PackageRecordRule,
        contribution_id: Option<&str>,
        message: impl Into<String>,
    ) -> PackageRecordError {
        PackageRecordError {
            package_name: self.package_name.clone(),
            package_version: self.package_version.clone(),
            api_prefix: self.api_prefix.clone(),
            contribution_id: contribution_id.map(ToOwned::to_owned),
            rule,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn full_markdown_fixture() -> Value {
        json!({
            "name": "@clay/markdown",
            "version": "0.1.0",
            "type": "module",
            "exports": { ".": "./dist/index.js" },
            "clay": {
                "apiPrefix": "markdown",
                "entry": "./dist/index.js",
                "loadEntry": "./dist/load.js",
                "permissions": [
                    "mode-registration",
                    "mode-activation",
                    "command-registration",
                    "package-configuration"
                ],
                "modes": ["markdown"],
                "docs": "./docs/index.md",
                "apiDependencies": [
                    "clay.modes.serverRegisterModePattern",
                    "clay.commands.serverRegisterCommand"
                ],
                "contributions": {
                    "commands": [{
                        "id": "markdown.togglePreview",
                        "displayName": "Toggle Markdown Preview",
                        "routingPolicy": "server-first"
                    }],
                    "configuration": [{
                        "key": "markdown.preview.enabled",
                        "type": "boolean",
                        "default": false
                    }]
                }
            }
        })
    }

    #[test]
    fn package_record_accepts_full_markdown_contract() {
        let record = assemble_package_record(&full_markdown_fixture())
            .expect("full markdown contract must validate");

        assert_eq!(record.manifest.name, "@clay/markdown");
        assert_eq!(record.manifest.version, "0.1.0");
        assert_eq!(record.manifest.clay.api_prefix, "markdown");
        assert_eq!(record.docs.docs_path, "./docs/index.md");
        assert_eq!(record.api_dependencies.len(), 2);
        assert_eq!(
            record.api_dependencies[0].api_id,
            "clay.modes.serverRegisterModePattern"
        );
        assert_eq!(record.contributions.commands.len(), 1);
        assert_eq!(
            record.contributions.commands[0].id,
            "markdown.togglePreview"
        );
        assert_eq!(record.contributions.configuration.len(), 1);
        assert_eq!(
            record.contributions.configuration[0].key,
            "markdown.preview.enabled"
        );
    }

    #[test]
    fn package_record_rejects_missing_docs_field() {
        let mut fixture = full_markdown_fixture();
        // Remove the docs field.
        fixture["clay"].as_object_mut().unwrap().remove("docs");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::MissingRequiredField);
        assert!(err.message.contains("clay.docs"));
        assert_eq!(err.package_name.as_deref(), Some("@clay/markdown"));
    }

    #[test]
    fn package_record_rejects_contribution_claiming_clay_reserved_id() {
        let mut fixture = full_markdown_fixture();
        fixture["clay"]["contributions"]["commands"][0]["id"] = json!("clay.badCommand");
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(err.rule, PackageRecordRule::ReservedClayIdInContribution);
        assert!(err.message.contains("clay.*"));
    }

    #[test]
    fn package_record_rejects_undeclared_permission_for_contribution() {
        let mut fixture = full_markdown_fixture();
        // Strip `command-registration` so commands are undeclared.
        fixture["clay"]["permissions"] = json!([
            "mode-registration",
            "mode-activation",
            "package-configuration"
        ]);
        let err = assemble_package_record(&fixture).unwrap_err();
        assert_eq!(
            err.rule,
            PackageRecordRule::UndeclaredPermissionForContribution
        );
        assert!(err.message.contains("command-registration"));
    }
}
