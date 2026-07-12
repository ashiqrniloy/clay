//! Atomic `setTypography` Clay JS op for user-owned font profiles.

use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::{
    perf::budgets::TYPOGRAPHY_PAYLOAD_BUDGET_BYTES,
    protocol::{ActiveTypography, FontProfile},
};

use super::ClayOpState;

/// Validate and replace all three user-owned profiles as one transaction.
#[op2]
#[string]
pub(super) fn op_clay_theme_set_typography(
    state: &mut OpState,
    #[string] request_json: String,
) -> Result<String, JsErrorBox> {
    if request_json.len() > TYPOGRAPHY_PAYLOAD_BUDGET_BYTES {
        return Err(invalid_typography());
    }
    let request: Value = serde_json::from_str(&request_json).map_err(|_| invalid_typography())?;
    let typography = parse_typography(&request)?;
    let typography = state
        .borrow::<Arc<ClayOpState>>()
        .set_active_typography(typography)
        .map_err(|_| invalid_typography())?;

    serde_json::to_string(&json!({ "revision": typography.revision })).map_err(|_| {
        JsErrorBox::generic("clay.theme.invalid_typography: failed to serialize result")
    })
}

fn parse_typography(value: &Value) -> Result<ActiveTypography, JsErrorBox> {
    let object = value.as_object().ok_or_else(invalid_typography)?;
    require_only_keys(object, &["monospace", "proportional", "ui"])?;
    Ok(ActiveTypography {
        revision: 0,
        monospace: parse_profile(object, "monospace")?,
        proportional: parse_profile(object, "proportional")?,
        ui: parse_profile(object, "ui")?,
    })
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
