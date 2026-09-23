//! Atomic `setTypography` Clay JS op for user-owned font profiles.

use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Map, Value, json};

use crate::{
    perf::budgets::TYPOGRAPHY_PAYLOAD_BUDGET_BYTES,
    protocol::{ActiveTypography, FontProfile, LigaturePolicy, UiTypographyHierarchy},
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
    serde_json::to_string(&json!({ "revision": typography.revision }))
        .map_err(|_| JsErrorBox::generic("theme.invalid_typography: failed to serialize result"))
}

pub(crate) fn validate_typography_request(value: &Value) -> Result<(), String> {
    parse_typography(value)
        .map(|_| ())
        .map_err(|_| "theme.invalid_typography: invalid complete typography request".to_string())
}

fn parse_typography(value: &Value) -> Result<ActiveTypography, JsErrorBox> {
    let object = value.as_object().ok_or_else(invalid_typography)?;
    // The three profiles are always required; `hierarchy` is optional. Reject any
    // other key so a future field never silently round-trips as typography.
    require_keys(object, &["monospace", "proportional", "ui"])?;
    reject_unknown_keys(object, &["monospace", "proportional", "ui", "hierarchy"])?;
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
    require_keys(profile, &["families", "size"])?;
    reject_unknown_keys(profile, &["families", "size", "ligatures"])?;
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
    let ligatures = profile
        .get("ligatures")
        .map(parse_ligature_policy)
        .transpose()?;
    Ok(FontProfile {
        families,
        size,
        ligatures: Box::new(ligatures.unwrap_or_default()),
    })
}

/// Parse an optional `ligatures` policy object. Fields default when absent so a
/// `setTypography` call without `ligatures` keeps the historical ligature-on
/// shaping; deny-by-default rejects unknown ligature keys.
fn parse_ligature_policy(value: &Value) -> Result<LigaturePolicy, JsErrorBox> {
    let object = value.as_object().ok_or_else(invalid_typography)?;
    reject_unknown_keys(
        object,
        &[
            "enableStandard",
            "enableContextual",
            "discretionaryFeatures",
            "rawFeatures",
            "disableFeatures",
        ],
    )?;
    let enable_standard = object
        .get("enableStandard")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let enable_contextual = object
        .get("enableContextual")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let discretionary_features = parse_feature_list(object, "discretionaryFeatures")?;
    let raw_features = object
        .get("rawFeatures")
        .and_then(Value::as_str)
        .map(str::to_string);
    let disable_features = parse_feature_list(object, "disableFeatures")?;
    Ok(LigaturePolicy {
        enable_standard,
        enable_contextual,
        discretionary_features,
        raw_features,
        disable_features,
    })
}

fn parse_feature_list(object: &Map<String, Value>, key: &str) -> Result<Vec<String>, JsErrorBox> {
    match object.get(key).and_then(Value::as_array) {
        Some(array) => array
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(invalid_typography)
            .map(|items| items.into_iter().map(str::to_string).collect()),
        None => Ok(Vec::new()),
    }
}

fn require_keys(object: &Map<String, Value>, keys: &[&str]) -> Result<(), JsErrorBox> {
    if keys.iter().all(|key| object.contains_key(*key)) {
        Ok(())
    } else {
        Err(invalid_typography())
    }
}

fn reject_unknown_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), JsErrorBox> {
    if object.keys().all(|key| allowed.contains(&key.as_str())) {
        Ok(())
    } else {
        Err(invalid_typography())
    }
}

fn invalid_typography() -> JsErrorBox {
    JsErrorBox::generic(
        "theme.invalid_typography: setTypography requires complete monospace, proportional, and ui profiles with only families and size",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base_profile() -> serde_json::Value {
        json!({ "families": ["monospace"], "size": 16.0 })
    }

    #[test]
    fn parse_typography_accepts_profile_without_ligatures() {
        let value = json!({
            "monospace": base_profile(),
            "proportional": { "families": ["sans-serif"], "size": 16.0 },
            "ui": { "families": ["system-ui"], "size": 12.0 },
        });
        let typography = parse_typography(&value).expect("ligatures optional");
        assert!(typography.monospace.ligatures.enable_standard);
        assert!(typography.monospace.ligatures.enable_contextual);
    }

    #[test]
    fn parse_typography_parses_ligature_policy_fields() {
        let value = json!({
            "monospace": {
                "families": ["monospace"],
                "size": 16.0,
                "ligatures": {
                    "enableStandard": false,
                    "enableContextual": true,
                    "discretionaryFeatures": ["ss01"],
                    "rawFeatures": "'calt' 1, 'liga' 0",
                    "disableFeatures": ["liga"],
                },
            },
            "proportional": { "families": ["sans-serif"], "size": 16.0 },
            "ui": { "families": ["system-ui"], "size": 12.0 },
        });
        let typography = parse_typography(&value).expect("valid ligatures");
        let policy = &typography.monospace.ligatures;
        assert!(!policy.enable_standard);
        assert!(policy.enable_contextual);
        assert_eq!(policy.discretionary_features, vec!["ss01".to_string()]);
        assert_eq!(policy.disable_features, vec!["liga".to_string()]);
        assert_eq!(policy.raw_features.as_deref(), Some("'calt' 1, 'liga' 0"));
    }

    #[test]
    fn parse_typography_rejects_unknown_ligature_key() {
        let value = json!({
            "monospace": {
                "families": ["monospace"],
                "size": 16.0,
                "ligatures": { "enableStandard": true, "futureField": 1 },
            },
            "proportional": { "families": ["sans-serif"], "size": 16.0 },
            "ui": { "families": ["system-ui"], "size": 12.0 },
        });
        assert!(parse_typography(&value).is_err());
    }

    #[test]
    fn parse_typography_rejects_unknown_top_level_key() {
        let value = json!({
            "monospace": base_profile(),
            "proportional": { "families": ["sans-serif"], "size": 16.0 },
            "ui": { "families": ["system-ui"], "size": 12.0 },
            "futureTopLevel": 1,
        });
        assert!(parse_typography(&value).is_err());
    }
}
