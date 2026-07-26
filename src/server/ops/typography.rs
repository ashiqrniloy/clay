//! Atomic `setTypography` Clay JS op for user-owned font profiles.

use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::{
    perf::budgets::TYPOGRAPHY_PAYLOAD_BUDGET_BYTES,
    protocol::{ActiveTypography, FontProfile, UiTypographyHierarchy},
};

use super::ClayOpState;

/// Validate and apply a complete typography replacement (three font profiles
/// plus optional hierarchy) as one transaction. Shared by the `setTypography`
/// op and the persisted-preference apply path so both enforce identical bounds.
/// The returned snapshot carries the authoritative revision assigned by
/// [`ClayOpState::set_active_typography`].
pub(crate) fn apply_typography(
    clay_state: &Arc<ClayOpState>,
    request_json: &str,
) -> Result<ActiveTypography, JsErrorBox> {
    if request_json.len() > TYPOGRAPHY_PAYLOAD_BUDGET_BYTES {
        return Err(invalid_typography());
    }
    let request: Value = serde_json::from_str(request_json).map_err(|_| invalid_typography())?;
    let typography = parse_typography(&request)?;
    clay_state
        .set_active_typography(typography)
        .map_err(|_| invalid_typography())
}

/// Validate and replace all three user-owned profiles as one transaction.
#[op2]
#[string]
pub(super) fn op_clay_theme_set_typography(
    state: &mut OpState,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    let clay_state = state.borrow::<Arc<ClayOpState>>();
    let typography = apply_typography(clay_state, &request_json)?;
    serde_json::to_string(&json!({ "revision": typography.revision })).map_err(|_| {
        JsErrorBox::generic("clay.theme.invalid_typography: failed to serialize result")
    })
}

fn parse_typography(value: &Value) -> Result<ActiveTypography, JsErrorBox> {
    let object = value.as_object().ok_or_else(invalid_typography)?;
    // The three profiles are always required; `hierarchy` is optional. Reject any
    // other key so a future field never silently round-trips as typography.
    require_only_keys(object, &["monospace", "proportional", "ui"])?;
    if object.contains_key("hierarchy") && object.len() != 4 {
        return Err(invalid_typography());
    }
    Ok(ActiveTypography {
        revision: 0,
        monospace: parse_profile(object, "monospace")?,
        proportional: parse_profile(object, "proportional")?,
        ui: parse_profile(object, "ui")?,
        hierarchy: parse_hierarchy(object.get("hierarchy"))?,
    })
}

/// Parse the optional complete hierarchy. Omission yields defaults (backward
/// compatibility). When present it must carry exactly the seven named scale
/// fields, each a finite bounded number; partial hierarchies are rejected
/// atomically so half-installed scales never reach layout.
fn parse_hierarchy(value: Option<&Value>) -> Result<UiTypographyHierarchy, JsErrorBox> {
    const NAMES: [&str; 7] = [
        "display", "title", "section", "body", "status", "detail", "caption",
    ];
    let Some(object) = value else {
        return Ok(UiTypographyHierarchy::DEFAULT);
    };
    let object = object.as_object().ok_or_else(invalid_typography)?;
    if object.len() != NAMES.len() || !object.keys().all(|key| NAMES.contains(&key.as_str())) {
        return Err(invalid_typography());
    }
    let mut hierarchy = UiTypographyHierarchy::DEFAULT;
    for name in NAMES {
        let scale = object
            .get(name)
            .and_then(Value::as_f64)
            .ok_or_else(invalid_typography)? as f32;
        if !scale.is_finite() || scale <= 0.0 || scale > crate::protocol::HIERARCHY_SCALE_MAX {
            return Err(invalid_typography());
        }
        match name {
            "display" => hierarchy.display = scale,
            "title" => hierarchy.title = scale,
            "section" => hierarchy.section = scale,
            "body" => hierarchy.body = scale,
            "status" => hierarchy.status = scale,
            "detail" => hierarchy.detail = scale,
            "caption" => hierarchy.caption = scale,
            _ => unreachable!(),
        }
    }
    Ok(hierarchy)
}

fn parse_profile(object: &Map<String, Value>, name: &str) -> Result<FontProfile, JsErrorBox> {
    let profile = object
        .get(name)
        .and_then(Value::as_object)
        .ok_or_else(invalid_typography)?;
    require_only_keys(profile, &["families", "size"])?;
    let families = profile
        .get("families")
        .and_then(Value::as_array)
        .ok_or_else(invalid_typography)?
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid_typography)?
        .into_iter()
        .map(str::to_string)
        .collect();
    let size = profile
        .get("size")
        .and_then(Value::as_f64)
        .ok_or_else(invalid_typography)? as f32;
    Ok(FontProfile { families, size })
}

fn require_only_keys(object: &Map<String, Value>, keys: &[&str]) -> Result<(), JsErrorBox> {
    if object.len() == keys.len() && object.keys().all(|key| keys.contains(&key.as_str())) {
        Ok(())
    } else {
        Err(invalid_typography())
    }
}

fn invalid_typography() -> JsErrorBox {
    JsErrorBox::generic(
        "clay.theme.invalid_typography: setTypography requires complete monospace, proportional, and ui profiles with only families and size",
    )
}
