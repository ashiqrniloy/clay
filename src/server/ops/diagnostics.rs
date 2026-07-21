use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::{
    protocol::{
        DecorationProvenance, DiagnosticSet, DiagnosticSeverity, DiagnosticSpan, DocumentId,
        DocumentVersion,
    },
    server::diagnostics::{DiagnosticValidationError, validate_diagnostic_publication},
};

use super::{
    ClayOpState,
    decorations::{
        clay_error, optional_u64, parse_json, required_object, required_str, required_u64,
    },
};

#[op2]
#[string]
pub(super) fn op_clay_diagnostics_publish_diagnostics(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options_value = parse_json(&options_json, "clay.diagnostics.invalid_publication")?;
    let options = options_value.as_object().ok_or_else(|| {
        clay_error("clay.diagnostics.invalid_publication: options must be an object")
    })?;
    reject_forbidden_keys(options)?;
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .require_current_package_capability(
            crate::packages::permissions::PackagePermission::RenderDecorations,
        )?;
    let document_id = required_u64(
        options,
        "documentId",
        "clay.diagnostics.invalid_publication",
    )?;
    let document_version = required_u64(
        options,
        "documentVersion",
        "clay.diagnostics.invalid_publication",
    )?;
    let current_document_version =
        optional_u64(options.get("currentDocumentVersion"))?.unwrap_or(document_version);
    let viewport = required_object(options, "viewport", "clay.diagnostics.invalid_publication")?;
    let viewport_byte_start = required_u64(
        viewport,
        "byteStart",
        "clay.diagnostics.invalid_publication",
    )?;
    let viewport_byte_end =
        required_u64(viewport, "byteEnd", "clay.diagnostics.invalid_publication")?;
    let source =
        required_str(options, "source", "clay.diagnostics.invalid_publication")?.to_string();
    let provenance = DecorationProvenance {
        package_name: package.manifest.name.clone(),
        package_version: package.manifest.version.clone(),
        package_prefix: package.manifest.clay.api_prefix.clone(),
    };
    let spans = options
        .get("spans")
        .and_then(Value::as_array)
        .ok_or_else(|| clay_error("clay.diagnostics.invalid_publication: spans must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, span)| span_from_value(index, span, &source, &provenance))
        .collect::<Result<Vec<_>, _>>()?;

    let set = DiagnosticSet {
        document_id: document_id as DocumentId,
        document_version: document_version as DocumentVersion,
        viewport_byte_start,
        viewport_byte_end,
        source: source.clone(),
        provenance,
        spans,
    };
    let set = validate_diagnostic_publication(&package, current_document_version, set)
        .map_err(diagnostic_error)?;
    let span_count = set.spans.len();
    state
        .borrow::<Arc<ClayOpState>>()
        .publish_diagnostic_set(set);

    serde_json::to_string(&json!({
        "documentId": document_id,
        "documentVersion": document_version,
        "packagePrefix": package.manifest.clay.api_prefix,
        "source": source,
        "publishedSpanCount": span_count,
    }))
    .map_err(|error| {
        clay_error(format!(
            "clay.diagnostics.publish_failed: failed to serialize result ({error})"
        ))
    })
}

fn span_from_value(
    index: usize,
    value: &Value,
    set_source: &str,
    provenance: &DecorationProvenance,
) -> Result<DiagnosticSpan, JsErrorBox> {
    let object = value.as_object().ok_or_else(|| {
        clay_error(format!(
            "clay.diagnostics.invalid_span: span {index} must be an object"
        ))
    })?;
    reject_forbidden_keys(object)?;
    let severity = match required_str(object, "severity", "clay.diagnostics.invalid_span")? {
        "error" | "Error" => DiagnosticSeverity::Error,
        "warning" | "Warning" => DiagnosticSeverity::Warning,
        "info" | "Info" => DiagnosticSeverity::Info,
        other => {
            return Err(clay_error(format!(
                "clay.diagnostics.invalid_span: unsupported severity `{other}`"
            )));
        }
    };
    let source = match object.get("source") {
        None | Some(Value::Null) => set_source.to_string(),
        Some(Value::String(value)) if !value.trim().is_empty() => value.clone(),
        _ => {
            return Err(clay_error(format!(
                "clay.diagnostics.invalid_span: span {index} source must be a non-empty string"
            )));
        }
    };
    Ok(DiagnosticSpan {
        byte_start: required_u64(object, "byteStart", "clay.diagnostics.invalid_span")?,
        byte_end: required_u64(object, "byteEnd", "clay.diagnostics.invalid_span")?,
        severity,
        code: required_str(object, "code", "clay.diagnostics.invalid_span")?.to_string(),
        message: required_str(object, "message", "clay.diagnostics.invalid_span")?.to_string(),
        source,
        provenance: provenance.clone(),
    })
}

fn reject_forbidden_keys(object: &Map<String, Value>) -> Result<(), JsErrorBox> {
    for key in [
        "handler",
        "callback",
        "onDiagnostic",
        "function",
        "clientJavaScript",
        "nativeHandle",
        "rawOps",
        "draw",
        "css",
        "render",
    ] {
        if object.contains_key(key) {
            return Err(clay_error(format!(
                "clay.diagnostics.invalid_publication: executable or raw authority field {key} is not accepted"
            )));
        }
    }
    Ok(())
}

fn diagnostic_error(error: DiagnosticValidationError) -> JsErrorBox {
    clay_error(format!("clay.diagnostics.publish_failed: {error:?}"))
}
