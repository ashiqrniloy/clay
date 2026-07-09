use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::{
    packages::record::{PackageRecord, assemble_package_record},
    protocol::{
        DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, DocumentId,
        DocumentVersion,
    },
    server::decorations::{DecorationValidationError, validate_decoration_publication},
};

use super::ClayOpState;

#[op2]
#[string]
pub(super) fn op_clay_decorations_publish_decorations(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options_value = parse_json(&options_json, "clay.decorations.invalid_publication")?;
    let options = options_value.as_object().ok_or_else(|| {
        clay_error("clay.decorations.invalid_publication: options must be an object")
    })?;
    let package = package_from_options(options, "render-decorations")?;
    let document_id = required_u64(
        options,
        "documentId",
        "clay.decorations.invalid_publication",
    )?;
    let document_version = required_u64(
        options,
        "documentVersion",
        "clay.decorations.invalid_publication",
    )?;
    let current_document_version =
        optional_u64(options.get("currentDocumentVersion"))?.unwrap_or(document_version);
    let viewport = required_object(options, "viewport", "clay.decorations.invalid_publication")?;
    let viewport_byte_start = required_u64(
        viewport,
        "byteStart",
        "clay.decorations.invalid_publication",
    )?;
    let viewport_byte_end =
        required_u64(viewport, "byteEnd", "clay.decorations.invalid_publication")?;
    let spans = options
        .get("spans")
        .and_then(Value::as_array)
        .ok_or_else(|| clay_error("clay.decorations.invalid_publication: spans must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, span)| span_from_value(index, span, &package))
        .collect::<Result<Vec<_>, _>>()?;

    let set = DecorationSet {
        document_id: document_id as DocumentId,
        document_version: document_version as DocumentVersion,
        viewport_byte_start,
        viewport_byte_end,
        spans,
    };
    let set = validate_decoration_publication(&package, current_document_version, set)
        .map_err(decoration_error)?;
    let span_count = set.spans.len();
    state
        .borrow::<Arc<ClayOpState>>()
        .publish_decoration_set(set);

    serde_json::to_string(&json!({
        "documentId": document_id,
        "documentVersion": document_version,
        "packagePrefix": package.manifest.clay.api_prefix,
        "publishedSpanCount": span_count,
    }))
    .map_err(|error| {
        clay_error(format!(
            "clay.decorations.publish_failed: failed to serialize result ({error})"
        ))
    })
}

fn span_from_value(
    index: usize,
    value: &Value,
    package: &PackageRecord,
) -> Result<DecorationSpan, JsErrorBox> {
    let object = value.as_object().ok_or_else(|| {
        clay_error(format!(
            "clay.decorations.invalid_span: span {index} must be an object"
        ))
    })?;
    let kind = match required_str(object, "kind", "clay.decorations.invalid_span")? {
        "syntax" | "Syntax" => DecorationKind::Syntax,
        "semantic" | "Semantic" => DecorationKind::Semantic,
        "diagnostic" | "Diagnostic" => DecorationKind::Diagnostic,
        "search-match" | "searchMatch" | "SearchMatch" => DecorationKind::SearchMatch,
        other => {
            return Err(clay_error(format!(
                "clay.decorations.invalid_span: unsupported decoration kind `{other}`"
            )));
        }
    };
    Ok(DecorationSpan::from_style_token(
        required_u64(object, "byteStart", "clay.decorations.invalid_span")?,
        required_u64(object, "byteEnd", "clay.decorations.invalid_span")?,
        kind,
        required_str(object, "styleToken", "clay.decorations.invalid_span")?,
        optional_u64(object.get("priority"))?.unwrap_or(0) as u16,
        DecorationProvenance {
            package_name: package.manifest.name.clone(),
            package_version: package.manifest.version.clone(),
            package_prefix: package.manifest.clay.api_prefix.clone(),
        },
    ))
}

pub(super) fn package_from_options(
    options: &Map<String, Value>,
    required_permission: &str,
) -> Result<PackageRecord, JsErrorBox> {
    if let Some(manifest) = options.get("packageManifest") {
        let package = assemble_package_record(manifest).map_err(|error| {
            clay_error(format!(
                "clay.packages.invalid_manifest: {:?}: {}",
                error.rule, error.message
            ))
        })?;
        if !package
            .manifest
            .clay
            .permissions
            .iter()
            .any(|permission| permission.as_str() == required_permission)
        {
            return Err(clay_error(format!(
                "clay.packages.missing_permission: package `{}` must declare `{required_permission}`",
                package.manifest.name
            )));
        }
        return Ok(package);
    }

    let package_name = required_str(options, "packageName", "clay.packages.invalid_manifest")?;
    let package_version = options
        .get("packageVersion")
        .and_then(Value::as_str)
        .unwrap_or("0.0.0");
    let api_prefix = required_str(options, "packagePrefix", "clay.packages.invalid_manifest")
        .or_else(|_| required_str(options, "apiPrefix", "clay.packages.invalid_manifest"))?;
    let permissions = options
        .get("permissions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            clay_error(format!(
                "clay.packages.missing_permission: `{required_permission}` permission is required"
            ))
        })?;
    let permission_strings = permissions
        .iter()
        .map(|value| {
            value.as_str().ok_or_else(|| {
                clay_error("clay.packages.invalid_permissions: permissions must be strings")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !permission_strings.contains(&required_permission) {
        return Err(clay_error(format!(
            "clay.packages.missing_permission: `{required_permission}` permission is required"
        )));
    }
    assemble_package_record(&json!({
        "name": package_name,
        "version": package_version,
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": api_prefix,
            "entry": "./dist/index.js",
            "permissions": permission_strings,
            "modes": [options.get("mode").and_then(Value::as_str).unwrap_or(api_prefix)],
            "docs": "./docs/index.md"
        }
    }))
    .map_err(|error| {
        clay_error(format!(
            "clay.packages.invalid_manifest: {:?}: {}",
            error.rule, error.message
        ))
    })
}

pub(super) fn parse_json(json_text: &str, code: &str) -> Result<Value, JsErrorBox> {
    serde_json::from_str(json_text)
        .map_err(|error| clay_error(format!("{code}: input must be valid JSON ({error})")))
}

pub(super) fn required_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    code: &str,
) -> Result<&'a Map<String, Value>, JsErrorBox> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| clay_error(format!("{code}: {key} must be an object")))
}

pub(super) fn required_str<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    code: &str,
) -> Result<&'a str, JsErrorBox> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| clay_error(format!("{code}: {key} must be a non-empty string")))
}

pub(super) fn required_u64(
    object: &Map<String, Value>,
    key: &str,
    code: &str,
) -> Result<u64, JsErrorBox> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| clay_error(format!("{code}: {key} must be an unsigned integer")))
}

pub(super) fn optional_u64(value: Option<&Value>) -> Result<Option<u64>, JsErrorBox> {
    match value {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            clay_error("clay.runtime.invalid_options: optional integer must be unsigned")
        }),
    }
}

pub(super) fn clay_error(message: impl Into<String>) -> JsErrorBox {
    JsErrorBox::generic(message.into())
}

fn decoration_error(error: DecorationValidationError) -> JsErrorBox {
    clay_error(format!("clay.decorations.publish_failed: {error:?}"))
}
