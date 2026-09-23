// Auto-extracted icon-pack contribution parsing (Plan 112 task 3). Private submodule: icon family.
use super::*;

use std::collections::HashSet;

use serde_json::Value;

use crate::perf::budgets::{ICON_GEOMETRY_PAYLOAD_BUDGET_BYTES, ICON_PACK_PAYLOAD_BUDGET_BYTES};
use crate::shell::icons::{
    ICON_SCHEMA_VERSION, IconGeometry, IconPath, MAX_ICON_PATHS, MAX_ICONS_PER_PACK,
    parse_icon_path, validate_icon_geometry, validate_icon_pack_key,
};

/// Fields that would smuggle raw SVG/XML, CSS, URLs, or executable hooks into
/// what must be bounded normalized geometry. Rejected structurally before
/// parsing so no raw markup is ever interpreted.
const FORBIDDEN_ICON_FIELDS: &[&str] = &[
    "svg",
    "xml",
    "url",
    "href",
    "src",
    "style",
    "class",
    "className",
    "fill",
    "stroke",
    "strokeWidth",
    "transform",
    "filter",
    "mask",
    "use",
    "image",
    "foreignObject",
    "script",
    "onload",
    "onclick",
    "handler",
    "callback",
    "font",
    "fontFamily",
];

pub(super) fn parse_icon_pack_contribution(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Option<IconPackContributionDescriptor>, PackageRecordError> {
    if value.is_null() {
        return Ok(None);
    }
    let size = contribution_payload_size(value);
    if size > ICON_PACK_PAYLOAD_BUDGET_BYTES {
        return Err(ctx.error(
            PackageRecordRule::PayloadBudgetExceeded,
            None,
            format!(
                "iconPack contribution payload ({size} bytes) exceeds ICON_PACK_PAYLOAD_BUDGET_BYTES ({ICON_PACK_PAYLOAD_BUDGET_BYTES} bytes)"
            ),
        ));
    }

    reject_ui_prohibited_authority(value, ctx)?;

    let obj = value.as_object().ok_or_else(|| {
        ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.iconPack must be an object",
        )
    })?;

    for forbidden in FORBIDDEN_ICON_FIELDS {
        if obj.contains_key(*forbidden) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(forbidden),
                "iconPack must declare bounded normalized geometry, not raw SVG/XML, CSS, URLs, or executable hooks",
            ));
        }
    }

    let schema_version = obj
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if schema_version != u64::from(ICON_SCHEMA_VERSION) {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("iconPack schemaVersion must be {ICON_SCHEMA_VERSION} (got {schema_version})"),
        ));
    }

    let display_name = required_str_field(obj, "displayName", ctx)?.to_string();

    // First-party status gates bare core keys at declaration time. Activation
    // additionally requires compiled-inventory provenance (task 5); this is
    // namespace hygiene, not a trust decision.
    let first_party = ctx
        .package_name
        .as_deref()
        .is_some_and(|name| name.starts_with("@clay/"));

    let entries = array_field(
        obj.get("icons").unwrap_or(&Value::Null),
        "iconPack.icons",
        ctx,
    )?;
    if entries.is_empty() {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "iconPack must declare at least one icon",
        ));
    }
    if entries.len() > MAX_ICONS_PER_PACK {
        return Err(ctx.error(
            PackageRecordRule::PayloadBudgetExceeded,
            None,
            format!(
                "iconPack declares {} icons; the limit is {MAX_ICONS_PER_PACK}",
                entries.len()
            ),
        ));
    }

    let mut seen_keys: HashSet<String> = HashSet::new();
    let mut icons = Vec::with_capacity(entries.len());
    for entry in entries {
        let icon_obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "iconPack icon entries must be objects",
            )
        })?;
        for forbidden in FORBIDDEN_ICON_FIELDS {
            if icon_obj.contains_key(*forbidden) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(forbidden),
                    "icon entries must declare bounded normalized geometry, not raw SVG/XML, CSS, URLs, or executable hooks",
                ));
            }
        }
        let key = required_str_field(icon_obj, "key", ctx)?;
        validate_icon_pack_key(key, api_prefix, first_party).map_err(|error| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(key),
                error.message,
            )
        })?;
        if !seen_keys.insert(key.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(key),
                "iconPack icon keys must be unique within a pack",
            ));
        }

        let view_box = parse_view_box(icon_obj.get("viewBox"), ctx)?;
        let path_values = array_field(
            icon_obj.get("paths").unwrap_or(&Value::Null),
            "icon paths",
            ctx,
        )?;
        if path_values.is_empty() || path_values.len() > MAX_ICON_PATHS {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(key),
                format!("icon paths require 1..={MAX_ICON_PATHS} entries"),
            ));
        }
        let mut paths = Vec::with_capacity(path_values.len());
        for path_value in path_values {
            let path_obj = path_value.as_object().ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(key),
                    "icon path entries must be objects with bounded `d` path data",
                )
            })?;
            for forbidden in FORBIDDEN_ICON_FIELDS {
                if path_obj.contains_key(*forbidden) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(forbidden),
                        "icon paths must declare bounded `d` path data, not raw SVG/XML, CSS, URLs, or executable hooks",
                    ));
                }
            }
            let d = required_str_field(path_obj, "d", ctx)?;
            if d.len() > ICON_GEOMETRY_PAYLOAD_BUDGET_BYTES {
                return Err(ctx.error(
                    PackageRecordRule::PayloadBudgetExceeded,
                    Some(key),
                    format!(
                        "icon path payload ({} bytes) exceeds ICON_GEOMETRY_PAYLOAD_BUDGET_BYTES ({ICON_GEOMETRY_PAYLOAD_BUDGET_BYTES} bytes)",
                        d.len()
                    ),
                ));
            }
            let commands = parse_icon_path(d).map_err(|error| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(key),
                    error.message,
                )
            })?;
            let opacity = match path_obj.get("opacity") {
                None | Some(Value::Null) => None,
                Some(Value::Number(number)) => {
                    let opacity = number.as_f64().unwrap_or(f64::NAN);
                    if !(0.0..=1.0).contains(&opacity) {
                        return Err(ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(key),
                            "icon path opacity must be within [0, 1]",
                        ));
                    }
                    Some(opacity as f32)
                }
                Some(_) => {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(key),
                        "icon path opacity must be a number",
                    ));
                }
            };
            paths.push(IconPath { commands, opacity });
        }

        let geometry = IconGeometry { view_box, paths };
        validate_icon_geometry(&geometry).map_err(|error| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(key),
                error.message,
            )
        })?;
        icons.push(IconDescriptor {
            key: key.to_string(),
            geometry,
        });
    }

    Ok(Some(IconPackContributionDescriptor {
        display_name,
        schema_version: ICON_SCHEMA_VERSION,
        icons,
        estimated_payload_bytes: size,
    }))
}

fn parse_view_box(
    value: Option<&Value>,
    ctx: &ErrorContext,
) -> Result<[f32; 4], PackageRecordError> {
    let values = value.and_then(Value::as_array).ok_or_else(|| {
        ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "icon viewBox must be an array of four numbers",
        )
    })?;
    if values.len() != 4 {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "icon viewBox must be an array of four numbers",
        ));
    }
    let mut view_box = [0.0_f32; 4];
    for (index, entry) in values.iter().enumerate() {
        view_box[index] = entry.as_f64().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "icon viewBox must be an array of four numbers",
            )
        })? as f32;
    }
    Ok(view_box)
}
