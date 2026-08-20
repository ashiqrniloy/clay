// Auto-extracted from record.rs (Plan 090 task 4). Private submodule: theme family.
use super::*;

use std::collections::HashSet;

use serde_json::Value;

use crate::perf::budgets::SDUI_UPDATE_PAYLOAD_BUDGET_BYTES;
use crate::shell::theme::{
    DensityLevel, ElevationLevel, MotionDuration, PackageThemeToken, ThemeTokenResolver,
    ThemeTokenType, ZLevel, core_fallback_matches_type, core_token_type, is_valid_dimension,
};

use crate::editor::theme::{parse_hex_rgba, parse_override_token};

pub(super) fn parse_theme_token_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<ThemeTokenContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.themeTokens must be an array",
        ));
    };

    let mut seen = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "theme token declaration payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "theme token contribution entries must be objects",
            )
        })?;
        if obj.contains_key("value") || obj.contains_key("rawColor") || obj.contains_key("css") {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "theme token declarations must use typed fallback contracts, not raw values, raw colors, or CSS",
            ));
        }
        let token = package_owned_field(obj, "token", api_prefix, ctx)?;
        if !seen.insert(token.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(token),
                "theme token IDs must be unique within a package",
            ));
        }
        let token_type_text = required_str_field(obj, "type", ctx)?;
        let Some(token_type) = ThemeTokenType::parse(token_type_text) else {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                format!(
                    "theme token type must be one of: {}",
                    ThemeTokenType::all_as_str().join(", ")
                ),
            ));
        };
        let fallback = required_str_field(obj, "fallback", ctx)?;
        if !core_fallback_matches_type(fallback, token_type) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "theme token fallback must reference a known Clay core token with the same type",
            ));
        }
        required_str_field(obj, "description", ctx)?;
        descriptors.push(ThemeTokenContributionDescriptor {
            token: token.to_string(),
            token_type: token_type.as_str().to_string(),
            fallback: fallback.to_string(),
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

/// Parse the Plan 046 task-5 `clay.contributions.textStyles` array into inert
/// [`TextStyleOverride`]s. This is the theme-package declaration path for
/// text-rendering overrides (the contract `setTheme` in task 7 selects). The
/// SDUI typed-scalar `ThemeTokenResolver` is intentionally untouched: text
/// styles are resolved into the editor `StyleRegistry`, separate from SDUI
/// component theming. Validation mirrors `parse_theme_token_contributions`:—
/// bounded payload, deny-by-default for executable/raw-CSS/raw-color fields,
/// closed override-target vocabulary (TokenType variant names + base-UI keys),
/// valid hex color, deterministic duplicate-token diagnostics, provenance
/// recorded as the declaring package's api prefix.
pub(super) fn parse_text_style_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<TextStyleOverrideDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.textStyles must be an array",
        ));
    };

    let mut seen = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "text style declaration payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "text style contribution entries must be objects",
            )
        })?;
        // The override carries an actual color value via the `color` hex
        // string; reject the raw-injection spelling (`rawColor`/`value`/`css`)
        // so only the validated `color` field is honored.
        if obj.contains_key("value")
            || obj.contains_key("rawColor")
            || obj.contains_key("css")
            || obj.contains_key("rawCss")
            || obj.contains_key("cssText")
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "text style declarations must use the validated `color` hex field, not raw values, raw colors, or CSS",
            ));
        }
        let token = required_str_field(obj, "token", ctx)?;
        if parse_override_token(token).is_none() {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "text style token must name a TokenType variant (e.g. `Keyword`, `Heading1`) or a base UI color key (e.g. `panelBg`, `caret`)",
            ));
        }
        if !seen.insert(token.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(token),
                "text style tokens must be unique within a package",
            ));
        }
        let color = match obj.get("color").and_then(Value::as_str) {
            Some(hex) => Some(parse_hex_rgba(hex).ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "text style `color` must be a #rgb, #rrggbb, or #rrggbbaa hex string",
                )
            })?),
            None => None,
        };
        let background = match obj.get("background").and_then(Value::as_str) {
            Some(hex) => Some(parse_hex_rgba(hex).ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "text style `background` must be a #rgb, #rrggbb, or #rrggbbaa hex string",
                )
            })?),
            None => None,
        };
        let bold = obj.get("bold").and_then(Value::as_bool);
        let italic = obj.get("italic").and_then(Value::as_bool);
        let underline = obj.get("underline").and_then(Value::as_bool);
        let strike = obj.get("strike").and_then(Value::as_bool);
        let scale = match obj.get("scale") {
            Some(Value::Number(number)) => {
                let Some(value) = number.as_f64() else {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        "text style `scale` must be a finite number in (0, 4]",
                    ));
                };
                let scale = value as f32;
                if !scale.is_finite()
                    || scale <= crate::protocol::HIERARCHY_SCALE_MIN
                    || scale > crate::protocol::HIERARCHY_SCALE_MAX
                {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        "text style `scale` must be a finite number in (0, 4]",
                    ));
                }
                Some(crate::editor::theme::scale_to_milli(scale))
            }
            Some(_) => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "text style `scale` must be a finite number in (0, 4]",
                ));
            }
            None => None,
        };
        // At least one override field must be present, otherwise the entry is
        // a no-op declaration.
        if color.is_none()
            && background.is_none()
            && bold.is_none()
            && italic.is_none()
            && underline.is_none()
            && strike.is_none()
            && scale.is_none()
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "text style declaration must override at least one of color, background, bold, italic, underline, strike, scale",
            ));
        }
        descriptors.push(TextStyleOverrideDescriptor {
            token: token.to_string(),
            color,
            background,
            bold,
            italic,
            underline,
            strike,
            scale,
            provenance: api_prefix.to_string(),
        });
    }
    Ok(descriptors)
}

// ponytail: duplicated from src/shell/components.rs (json_value_kind +
// sanitize_rejected) rather than promoted to a shared module — one tiny fn
// pair, two validation modules; sharing would add a module + import churn for
// no callers beyond these two. Fold into a shared diagnostic module if a third
// author-JSON validator appears.

/// Compact description of a rejected `serde_json::Value`'s shape for a "got …"
/// diagnostic fragment.
pub(super) fn json_value_kind(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => format!("boolean {b}"),
        Value::Number(n) => format!("number {n}"),
        Value::String(s) => format!("string `{}`", sanitize_rejected(s)),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    }
}

/// Trim, strip backticks, and bound to 80 chars so an author-supplied value
/// cannot break the diagnostic shape or leak unbounded content.
pub(super) fn sanitize_rejected(value: &str) -> String {
    let trimmed = value.trim().replace('`', "'");
    if trimmed.chars().count() > 80 {
        let bounded: String = trimmed.chars().take(80).collect();
        format!("{bounded}…")
    } else {
        trimmed
    }
}

/// Parse `clay.contributions.designTokens` (Phase 20.1) into validated inert
/// [`DesignTokenOverrideDescriptor`]s. Each entry names a core Clay token and
/// supplies a typed value whose variant must match the core token type and pass
/// domain-specific bounds. Unknown tokens, type mismatches, NaN/infinite/
/// out-of-range scalars, unparseable levels, duplicate tokens, raw CSS/raw color
/// injection fields, and oversize payloads are rejected before the descriptor
/// is stored. Design tokens never override typography variants (that is the
/// separate hierarchy path in task 5).
pub(super) fn parse_design_token_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<DesignTokenOverrideDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.designTokens must be an array",
        ));
    };

    let mut seen = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "design token declaration payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "design token contribution entries must be objects",
            )
        })?;
        // Reject the raw-injection spellings; the validated `value` field below is
        // the only honored value path (it is typed, not raw CSS).
        if obj.contains_key("rawColor")
            || obj.contains_key("css")
            || obj.contains_key("rawCss")
            || obj.contains_key("cssText")
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "design token declarations must use the validated `value` field, not raw colors or CSS",
            ));
        }
        let token = required_str_field(obj, "token", ctx)?;
        let Some(core_type) = core_token_type(token) else {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "design token must name a known Clay core token",
            ));
        };
        // Design tokens carry UI scalar/color/level overrides only; typography
        // variant overrides belong to the separate typography hierarchy path.
        if core_type == ThemeTokenType::Typography {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "design tokens cannot override typography variants; use the typography hierarchy",
            ));
        }
        if !seen.insert(token.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(token),
                "design token names must be unique within a package",
            ));
        }
        let value_field = obj.get("value").ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(token),
                "design token declaration must include a `value` field",
            )
        })?;
        let descriptor_value = match core_type {
            ThemeTokenType::ColorRole => {
                let hex = value_field.as_str().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "color-role design token `value` must be a #rgb, #rrggbb, or #rrggbbaa hex string; got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                let [r, g, b, a] = parse_hex_rgba(hex).ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "color-role design token `value` must be a #rgb, #rrggbb, or #rrggbbaa hex string; got `{}`",
                            sanitize_rejected(hex)
                        ),
                    )
                })?;
                DesignTokenValueDescriptor::Color([r, g, b, a])
            }
            ThemeTokenType::Spacing | ThemeTokenType::Radius | ThemeTokenType::Dimension => {
                let v = value_field.as_f64().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "scalar design token `value` must be a finite number; got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                if !is_valid_dimension(v) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "scalar design token `value` must be finite, non-negative, and bounded; got {v}"
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Scalar(v.to_bits())
            }
            ThemeTokenType::Opacity => {
                let v = value_field.as_f64().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "opacity design token `value` must be a finite number; got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })? as f32;
                if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "opacity design token `value` must be a finite number in [0, 1]; got {v}"
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Opacity(v.to_bits())
            }
            ThemeTokenType::MotionDuration => {
                let v = value_field.as_f64().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "motion-duration design token `value` must be a finite number of milliseconds; got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                if MotionDuration::from_millis(v).is_none() {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "motion-duration design token `value` must be finite, non-negative, and at most 1000 ms; got {v} ms"
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Scalar(v.to_bits())
            }
            ThemeTokenType::Elevation => {
                let s = value_field.as_str().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "elevation design token `value` must be a level name (none, raised, overlay); got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                if ElevationLevel::parse(s).is_none() {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "elevation design token `value` must be one of: none, raised, overlay; got `{}`",
                            sanitize_rejected(s)
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Level(s.to_string())
            }
            ThemeTokenType::ZLevel => {
                let s = value_field.as_str().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "z-level design token `value` must be a level name (base, panel, overlay, modal, tooltip); got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                if ZLevel::parse(s).is_none() {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "z-level design token `value` must be one of: base, panel, overlay, modal, tooltip; got `{}`",
                            sanitize_rejected(s)
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Level(s.to_string())
            }
            ThemeTokenType::Density => {
                let s = value_field.as_str().ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "density design token `value` must be a level name (compact, default, spacious); got {}",
                            json_value_kind(value_field)
                        ),
                    )
                })?;
                if DensityLevel::parse(s).is_none() {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        format!(
                            "density design token `value` must be one of: compact, default, spacious; got `{}`",
                            sanitize_rejected(s)
                        ),
                    ));
                }
                DesignTokenValueDescriptor::Level(s.to_string())
            }
            // `Typography` was rejected above; this arm is unreachable but keeps
            // the match exhaustive without a wildcard.
            ThemeTokenType::Typography => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "design tokens cannot override typography variants; use the typography hierarchy",
                ));
            }
        };
        descriptors.push(DesignTokenOverrideDescriptor {
            token: token.to_string(),
            value: descriptor_value,
            provenance: api_prefix.to_string(),
        });
    }
    Ok(descriptors)
}

pub(super) fn theme_resolver_for_package_tokens(
    tokens: &[ThemeTokenContributionDescriptor],
) -> ThemeTokenResolver {
    let mut resolver = ThemeTokenResolver::new();
    for token in tokens {
        let Some(token_type) = ThemeTokenType::parse(&token.token_type) else {
            continue;
        };
        resolver.insert_package_token(PackageThemeToken {
            token: token.token.clone(),
            token_type,
            fallback: token.fallback.clone(),
            description: String::new(),
        });
    }
    resolver
}

// ── Utility ──────────────────────────────────────────────────────────────────
