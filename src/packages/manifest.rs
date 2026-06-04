use std::collections::HashSet;

use serde_json::Value;

use crate::packages::permissions::{
    PackagePermission, PermissionValidationError, parse_permission,
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
    pub permissions: Vec<PackagePermission>,
    pub modes: Vec<String>,
    pub entry: String,
    pub load_entry: Option<String>,
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

    if payload_size(value) > BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES {
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

    reject_forbidden_runtime_metadata(value, &context)?;

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

    let permissions = parse_permissions(clay.get("permissions"), &context)?;
    let modes = parse_modes(clay.get("modes"), &api_prefix, &context)?;

    Ok(ClayPackageManifest {
        name: package_name,
        version: package_version,
        clay: ClayPackageMetadata {
            api_prefix,
            permissions,
            modes,
            entry,
            load_entry,
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

fn parse_permissions(
    value: Option<&Value>,
    context: &DiagnosticContext,
) -> Result<Vec<PackagePermission>, PackageDiagnostic> {
    let Some(Value::Array(values)) = value else {
        return Err(context.diagnostic(
            PackageValidationRule::MissingField,
            "clay.permissions must be an array of known permission strings",
        ));
    };

    let mut seen = HashSet::new();
    let mut permissions = Vec::new();
    for value in values {
        let Some(permission) = value.as_str() else {
            return Err(context.diagnostic(
                PackageValidationRule::InvalidPermission,
                "clay.permissions entries must be strings",
            ));
        };
        if !seen.insert(permission.to_string()) {
            return Err(context.diagnostic(
                PackageValidationRule::DuplicatePermission,
                "clay.permissions entries must be unique",
            ));
        }
        match parse_permission(permission) {
            Ok(permission) => permissions.push(permission),
            Err(PermissionValidationError::UnknownPermission { .. }) => {
                return Err(context.diagnostic(
                    PackageValidationRule::UnknownPermission,
                    format!("unknown Clay package permission `{permission}`"),
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

    Ok(permissions)
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
    if entry.trim().is_empty()
        || entry.starts_with("http://")
        || entry.starts_with("https://")
        || entry.contains("Deno.core.ops")
    {
        return Err(context.diagnostic(
            PackageValidationRule::InvalidEntry,
            format!("{field} must be a non-empty local module path"),
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
    RawDenoOpsExposure,
    ClientJavaScriptHook,
}

struct DiagnosticContext {
    package_name: Option<String>,
    package_version: Option<String>,
    api_prefix: Option<String>,
}

impl DiagnosticContext {
    fn new(
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

    fn diagnostic(
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
