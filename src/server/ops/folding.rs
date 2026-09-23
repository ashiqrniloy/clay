use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::{
    packages::record::PackageRecord,
    protocol::{DocumentId, DocumentVersion, FoldingProvenance, FoldingRange, FoldingRangeSet},
    server::folding::{FoldingValidationError, validate_folding_publication},
};

use super::{
    ClayOpState,
    decorations::{clay_error, optional_u64, parse_json, required_u64},
};

#[op2]
#[string]
pub(super) fn op_clay_folding_publish_ranges(
    state: &mut OpState,
    #[string] options_json: String,
) -> Result<String, JsErrorBox> {
    let options_value = parse_json(&options_json, "folding.invalid_publication")?;
    let options = options_value
        .as_object()
        .ok_or_else(|| clay_error("folding.invalid_publication: options must be an object"))?;
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .require_current_package_capability(
            crate::packages::permissions::PackagePermission::RenderFolding,
        )?;
    let document_id = required_u64(options, "documentId", "folding.invalid_publication")?;
    let document_version = required_u64(options, "documentVersion", "folding.invalid_publication")?;
    let current_document_version =
        optional_u64(options.get("currentDocumentVersion"))?.unwrap_or(document_version);
    if document_version != current_document_version {
        return serde_json::to_string(&json!({
            "documentId": document_id,
            "documentVersion": document_version,
            "dropped": true,
        }))
        .map_err(|error| clay_error(format!("folding.publish_failed: {error}")));
    }
    let ranges = options
        .get("ranges")
        .and_then(Value::as_array)
        .ok_or_else(|| clay_error("folding.invalid_publication: ranges must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, value)| range_from_value(index, value, &package))
        .collect::<Result<Vec<_>, _>>()?;
    let set = FoldingRangeSet {
        document_id: document_id as DocumentId,
        document_version: document_version as DocumentVersion,
        package_prefix: package.manifest.clay.api_prefix.clone(),
        ranges,
    };
    let set = validate_folding_publication(&package, current_document_version, set)
        .map_err(folding_error)?;
    let range_count = set.ranges.len();
    state.borrow::<Arc<ClayOpState>>().publish_folding_set(set);
    serde_json::to_string(&json!({
        "documentId": document_id,
        "documentVersion": document_version,
        "packagePrefix": package.manifest.clay.api_prefix,
        "publishedRangeCount": range_count,
        "dropped": false,
    }))
    .map_err(|error| clay_error(format!("folding.publish_failed: {error}")))
}

fn range_from_value(
    index: usize,
    value: &Value,
    package: &PackageRecord,
) -> Result<FoldingRange, JsErrorBox> {
    let object = value.as_object().ok_or_else(|| {
        clay_error(format!(
            "folding.invalid_range: range {index} must be an object"
        ))
    })?;
    let byte_start = required_u64(object, "byteStart", "folding.invalid_range")?;
    let byte_end = required_u64(object, "byteEnd", "folding.invalid_range")?;
    let label = match object.get("label") {
        None | Some(Value::Null) => None,
        Some(Value::String(label)) if !label.is_empty() => Some(label.clone()),
        _ => {
            return Err(clay_error(
                "folding.invalid_range: label must be a non-empty string",
            ));
        }
    };
    Ok(FoldingRange {
        byte_start,
        byte_end,
        label,
        provenance: FoldingProvenance {
            package_name: package.manifest.name.clone(),
            package_version: package.manifest.version.clone(),
            package_prefix: package.manifest.clay.api_prefix.clone(),
        },
    })
}

fn folding_error(error: FoldingValidationError) -> JsErrorBox {
    clay_error(format!(
        "folding.publish_failed: {error:?} (FOLDING_RANGE_PAYLOAD_BUDGET_BYTES)"
    ))
}
