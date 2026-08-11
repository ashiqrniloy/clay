use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::{
    packages::record::PackageRecord,
    protocol::{
        DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, DocumentFontRole,
        DocumentId, DocumentVersion, Modifiers, TokenType,
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
    let options_value = parse_json(&options_json, "decorations.invalid_publication")?;
    let options = options_value
        .as_object()
        .ok_or_else(|| clay_error("decorations.invalid_publication: options must be an object"))?;
    // Provenance comes from the host-owned executing-package context,
    // resolved against the enabled set with an approved capability check;
    // caller-supplied manifests are never consulted.
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .require_current_package_capability(
            crate::packages::permissions::PackagePermission::RenderDecorations,
        )?;
    let document_id = required_u64(options, "documentId", "decorations.invalid_publication")?;
    let document_version = required_u64(
        options,
        "documentVersion",
        "decorations.invalid_publication",
    )?;
    let current_document_version =
        optional_u64(options.get("currentDocumentVersion"))?.unwrap_or(document_version);
    let viewport = required_object(options, "viewport", "decorations.invalid_publication")?;
    let viewport_byte_start =
        required_u64(viewport, "byteStart", "decorations.invalid_publication")?;
    let viewport_byte_end = required_u64(viewport, "byteEnd", "decorations.invalid_publication")?;
    let spans = options
        .get("spans")
        .and_then(Value::as_array)
        .ok_or_else(|| clay_error("decorations.invalid_publication: spans must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, span)| span_from_value(index, span, &package))
        .collect::<Result<Vec<_>, _>>()?;

    let set = DecorationSet {
        document_id: document_id as DocumentId,
        document_version: document_version as DocumentVersion,
        package_prefix: package.manifest.clay.api_prefix.clone(),
        kind: spans
            .first()
            .map_or(crate::protocol::DecorationKind::Syntax, |span| span.kind),
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
            "decorations.publish_failed: failed to serialize result ({error})"
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
            "decorations.invalid_span: span {index} must be an object"
        ))
    })?;
    let kind = match required_str(object, "kind", "decorations.invalid_span")? {
        "syntax" | "Syntax" => DecorationKind::Syntax,
        "semantic" | "Semantic" => DecorationKind::Semantic,
        "diagnostic" | "Diagnostic" => DecorationKind::Diagnostic,
        "search-match" | "searchMatch" | "SearchMatch" => DecorationKind::SearchMatch,
        other => {
            return Err(clay_error(format!(
                "decorations.invalid_span: unsupported decoration kind `{other}`"
            )));
        }
    };
    let font_role = match object.get("fontRole") {
        None | Some(Value::Null) => None,
        Some(Value::String(role)) => match DocumentFontRole::from_name(role) {
            Some(role @ (DocumentFontRole::Monospace | DocumentFontRole::Proportional)) => {
                Some(role)
            }
            _ => {
                return Err(clay_error(
                    "decorations.invalid_span: fontRole must be `monospace` or `proportional`",
                ));
            }
        },
        Some(_) => {
            return Err(clay_error(
                "decorations.invalid_span: fontRole must be a semantic role string",
            ));
        }
    };
    let byte_start = required_u64(object, "byteStart", "decorations.invalid_span")?;
    let byte_end = required_u64(object, "byteEnd", "decorations.invalid_span")?;
    let priority = optional_u64(object.get("priority"))?.unwrap_or(0) as u16;
    let provenance = DecorationProvenance {
        package_name: package.manifest.name.clone(),
        package_version: package.manifest.version.clone(),
        package_prefix: package.manifest.clay.api_prefix.clone(),
    };

    let token_type_raw = object
        .get("tokenType")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let style_token_raw = object
        .get("styleToken")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let mut span = match (token_type_raw, style_token_raw) {
        (Some(token_type_name), _) => {
            let token_type = TokenType::from_name(token_type_name).ok_or_else(|| {
                clay_error(format!(
                    "decorations.invalid_span: unknown tokenType `{token_type_name}`"
                ))
            })?;
            let modifiers = modifiers_from_value(object.get("modifiers"))?;
            DecorationSpan::from_vocabulary(
                byte_start, byte_end, kind, token_type, modifiers, priority, provenance,
            )
        }
        (None, Some(style_token)) => DecorationSpan::from_style_token(
            byte_start,
            byte_end,
            kind,
            style_token,
            priority,
            provenance,
        ),
        (None, None) => {
            return Err(clay_error(
                "decorations.invalid_span: span must provide tokenType or styleToken",
            ));
        }
    };
    span.font_role = font_role;
    Ok(span)
}

fn modifiers_from_value(value: Option<&Value>) -> Result<Modifiers, JsErrorBox> {
    match value {
        None | Some(Value::Null) => Ok(Modifiers::NONE),
        Some(Value::Array(names)) => {
            let mut owned = Vec::with_capacity(names.len());
            for entry in names {
                let name = entry.as_str().ok_or_else(|| {
                    clay_error("decorations.invalid_span: modifiers must be an array of strings")
                })?;
                owned.push(name);
            }
            let borrowed = owned.to_vec();
            Modifiers::from_names(&borrowed)
                .ok_or_else(|| clay_error("decorations.invalid_span: unknown modifiers entry"))
        }
        Some(_) => Err(clay_error(
            "decorations.invalid_span: modifiers must be an array of strings",
        )),
    }
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
            clay_error("runtime.invalid_options: optional integer must be unsigned")
        }),
    }
}

pub(super) fn clay_error(message: impl Into<String>) -> JsErrorBox {
    JsErrorBox::generic(message.into())
}

fn decoration_error(error: DecorationValidationError) -> JsErrorBox {
    clay_error(format!("decorations.publish_failed: {error:?}"))
}
