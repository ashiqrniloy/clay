use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::packages::{
    manifest::{PackageDiagnostic, validate_manifest_value},
    permissions::{PermissionValidationError, parse_permission},
    record::{PackageRecordError, assemble_package_record},
};

#[op2]
#[string]
pub(super) fn op_clay_packages_validate_manifest(
    _state: &mut OpState,
    #[string] manifest_json: String,
) -> Result<String, JsErrorBox> {
    let value = parse_json(&manifest_json, "clay.packages.invalid_manifest")?;
    let manifest =
        validate_manifest_value(&value).map_err(package_error("clay.packages.invalid_manifest"))?;
    serde_json::to_string(&json!({
        "name": manifest.name,
        "version": manifest.version,
        "apiPrefix": manifest.clay.api_prefix,
        "permissions": manifest.clay.permissions.iter().map(|permission| permission.as_str()).collect::<Vec<_>>(),
        "modes": manifest.clay.modes,
        "entry": manifest.clay.entry,
        "loadEntry": manifest.clay.load_entry,
    }))
    .map_err(serialize_error("clay.packages.validation_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_packages_load_package(
    _state: &mut OpState,
    #[string] package_json: String,
) -> Result<String, JsErrorBox> {
    let value = parse_json(&package_json, "clay.packages.invalid_package")?;
    let record =
        assemble_package_record(&value).map_err(record_error("clay.packages.load_failed"))?;
    serde_json::to_string(&json!({
        "name": record.manifest.name,
        "version": record.manifest.version,
        "apiPrefix": record.manifest.clay.api_prefix,
        "entry": record.manifest.clay.entry,
        "loadEntry": record.manifest.clay.load_entry,
        "docs": record.docs.docs_path,
        "estimatedManifestBytes": record.performance.estimated_manifest_bytes,
        "apiDependencies": record.api_dependencies.iter().map(|dependency| dependency.api_id.as_str()).collect::<Vec<_>>(),
        "contributions": {
            "commands": record.contributions.commands.len(),
            "configuration": record.contributions.configuration.len(),
            "keyRouting": record.contributions.key_routing.len(),
            "textTransforms": record.contributions.text_transforms.len(),
            "sdui": record.contributions.sdui.len(),
            "decorations": record.contributions.decorations.len(),
            "uiPanels": record.contributions.ui_panels.len(),
            "uiComponents": record.contributions.ui_components.len(),
            "uiOverlays": record.contributions.ui_overlays.len(),
            "themeTokens": record.contributions.theme_tokens.len(),
            "input": record.contributions.input_contributions.len(),
            "uiStateScopes": record.contributions.ui_state_scopes.len(),
            "layoutOverrides": record.contributions.layout_overrides.len(),
            "packageOptions": record.contributions.package_options.len(),
        }
    }))
    .map_err(serialize_error("clay.packages.load_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_packages_validate_permissions(
    _state: &mut OpState,
    #[string] permissions_json: String,
) -> Result<String, JsErrorBox> {
    let value = parse_json(&permissions_json, "clay.packages.invalid_permissions")?;
    let Some(values) = value.as_array() else {
        return Err(JsErrorBox::generic(
            "clay.packages.invalid_permissions: permissions must be an array of strings",
        ));
    };
    let mut permissions = Vec::new();
    for value in values {
        let Some(permission) = value.as_str() else {
            return Err(JsErrorBox::generic(
                "clay.packages.invalid_permissions: permissions must be an array of strings",
            ));
        };
        match parse_permission(permission) {
            Ok(permission) => permissions.push(permission.as_str()),
            Err(PermissionValidationError::UnknownPermission { .. }) => {
                return Err(JsErrorBox::generic(format!(
                    "clay.packages.unknown_permission: unknown Clay package permission `{permission}`"
                )));
            }
            Err(PermissionValidationError::ProhibitedAuthority { .. }) => {
                return Err(JsErrorBox::generic(format!(
                    "clay.packages.prohibited_authority: prohibited authority `{permission}` cannot be requested by default"
                )));
            }
        }
    }

    serde_json::to_string(&json!({ "permissions": permissions }))
        .map_err(serialize_error("clay.packages.validation_failed"))
}

fn parse_json(json_text: &str, code: &str) -> Result<Value, JsErrorBox> {
    serde_json::from_str(json_text)
        .map_err(|error| JsErrorBox::generic(format!("{code}: input must be valid JSON ({error})")))
}

fn package_error(code: &'static str) -> impl Fn(PackageDiagnostic) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: {:?}: {}", error.rule, error.message))
}

fn record_error(code: &'static str) -> impl Fn(PackageRecordError) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: {:?}: {}", error.rule, error.message))
}

fn serialize_error(code: &'static str) -> impl Fn(serde_json::Error) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: failed to serialize result ({error})"))
}
