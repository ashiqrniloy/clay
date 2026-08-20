// Auto-extracted from record.rs (Plan 090 task 4). Private submodule: behavior family.
use super::*;

use std::collections::HashSet;

use serde_json::Value;

use crate::packages::permissions::PackagePermission;
use crate::perf::budgets::SDUI_UPDATE_PAYLOAD_BUDGET_BYTES;

pub(super) fn parse_mode_pattern_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    package_modes: &[String],
    ctx: &ErrorContext,
) -> Result<Vec<ModePatternContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.modePatterns must be an array",
        ));
    };
    if !entries.is_empty() && !permissions.contains(&PackagePermission::ModeRegistration) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "modePatterns contributions require the `mode-registration` permission",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "modePatterns entries must be objects",
            )
        })?;
        let mode_id = obj
            .get("mode")
            .or_else(|| obj.get("modeId"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "modePatterns entries must include a non-empty `mode` or `modeId`",
                )
            })?;
        if mode_id.starts_with("clay.") || mode_id.starts_with("core.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(mode_id),
                "modePatterns cannot claim the reserved clay.* or core.* namespaces",
            ));
        }
        if !is_package_owned_id(mode_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(mode_id),
                "modePatterns mode IDs must use the package apiPrefix or apiPrefix.* namespace",
            ));
        }
        if !package_modes.iter().any(|declared| declared == mode_id) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(mode_id),
                "modePatterns may only name modes declared in clay.modes",
            ));
        }
        if !seen_ids.insert(mode_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(mode_id),
                "modePatterns mode IDs must be unique within a package",
            ));
        }
        let document_font_role = match obj
            .get("defaultFontRole")
            .and_then(Value::as_str)
            .unwrap_or("proportional")
        {
            "monospace" => DocumentFontRole::Monospace,
            "proportional" => DocumentFontRole::Proportional,
            other => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(mode_id),
                    format!("defaultFontRole must be monospace or proportional, not `{other}`"),
                ));
            }
        };
        let editor_rules_json = match obj.get("editorRules") {
            None | Some(Value::Null) => None,
            Some(Value::Object(_)) => Some(
                serde_json::to_string(obj.get("editorRules").expect("editorRules present"))
                    .map_err(|error| {
                        ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            Some(mode_id),
                            format!("editorRules must serialize: {error}"),
                        )
                    })?,
            ),
            Some(_) => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(mode_id),
                    "editorRules must be an object",
                ));
            }
        };
        descriptors.push(ModePatternContributionDescriptor {
            mode_id: mode_id.to_string(),
            display_name: obj
                .get("displayName")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("Mode")
                .to_string(),
            document_font_role,
            extensions: optional_string_array(obj, "extensions", ctx)?,
            mime_types: optional_string_array(obj, "mimeTypes", ctx)?,
            file_names: optional_string_array(obj, "fileNames", ctx)?,
            file_name_patterns: optional_string_array(obj, "fileNamePatterns", ctx)?,
            shebang_patterns: optional_string_array(obj, "shebangPatterns", ctx)?,
            content_probes: optional_string_array(obj, "contentProbes", ctx)?,
            editor_rules_json,
        });
    }
    Ok(descriptors)
}

fn optional_string_array(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    ctx: &ErrorContext,
) -> Result<Vec<String>, PackageRecordError> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(entries)) => entries
            .iter()
            .map(|entry| {
                entry.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        None,
                        format!("{key} entries must be strings"),
                    )
                })
            })
            .collect(),
        Some(_) => Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("{key} must be an array of strings"),
        )),
    }
}

pub(super) fn parse_command_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<CommandContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.commands must be an array",
        ));
    };

    // Commands require `command-registration` permission.
    if !entries.is_empty() && !permissions.contains(&PackagePermission::CommandRegistration) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "command contributions require the `command-registration` permission to be declared in clay.permissions",
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "command contribution entries must be objects",
            )
        })?;

        let id = obj
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "command contribution must include a non-empty `id` field",
                )
            })?;

        if id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(id),
                "command contribution IDs cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "command contribution IDs must use the package apiPrefix or apiPrefix.* namespace",
            ));
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "command contribution IDs must be unique within a package",
            ));
        }

        let display_name = obj
            .get("displayName")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "command contribution must include a non-empty `displayName` field",
                )
            })?;

        let routing_policy = obj
            .get("routingPolicy")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "command contribution must include a non-empty `routingPolicy` field",
                )
            })?;

        // Reject routing policies that would grant built-in client-edit authority.
        if matches!(
            routing_policy,
            "client-first-predictable" | "client-first-requires-ack"
        ) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "package command contributions cannot declare built-in client-edit routing policies",
            ));
        }

        descriptors.push(CommandContributionDescriptor {
            id: id.to_string(),
            display_name: display_name.to_string(),
            routing_policy: routing_policy.to_string(),
        });
    }

    Ok(descriptors)
}

pub(super) fn parse_configuration_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<ConfigurationContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.configuration must be an array",
        ));
    };

    // Behavior-changing configuration requires `package-configuration`.
    if !entries.is_empty() && !permissions.contains(&PackagePermission::PackageConfiguration) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "configuration contributions require the `package-configuration` permission to be declared in clay.permissions",
        ));
    }

    let mut seen_keys = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "configuration contribution entries must be objects",
            )
        })?;

        let key = obj
            .get("key")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "configuration contribution must include a non-empty `key` field",
                )
            })?;

        if key.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(key),
                "configuration contribution keys cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(key, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(key),
                "configuration keys must use the package apiPrefix or apiPrefix.* namespace",
            ));
        }
        if !seen_keys.insert(key.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(key),
                "configuration contribution keys must be unique within a package",
            ));
        }

        let value_type = obj
            .get("type")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(key),
                    "configuration contribution must include a `type` field",
                )
            })?;

        if !matches!(value_type, "boolean" | "string" | "number" | "integer") {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(key),
                "configuration contribution `type` must be one of: boolean, string, number, integer",
            ));
        }

        let default_value = obj
            .get("default")
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        descriptors.push(ConfigurationContributionDescriptor {
            key: key.to_string(),
            value_type: value_type.to_string(),
            default_value,
        });
    }

    Ok(descriptors)
}

pub(super) fn parse_key_routing_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<KeyRoutingContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.keyRouting must be an array",
        ));
    };

    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "key routing contribution entries must be objects",
            )
        })?;

        let command_id = obj
            .get("commandId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "key routing contribution must include a non-empty `commandId` field",
                )
            })?;

        if command_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(command_id),
                "key routing contributions cannot target reserved clay.* command IDs",
            ));
        }
        if !is_package_owned_id(command_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(command_id),
                "key routing contribution commandId must use the package apiPrefix namespace",
            ));
        }

        let key_binding = obj
            .get("key")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(ToOwned::to_owned);
        let routing_policy = obj
            .get("routingPolicy")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(ToOwned::to_owned);
        let priority = match obj.get("priority") {
            Some(Value::Number(n)) => n.as_i64().map(|v| v as i32),
            Some(_) => {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(command_id),
                    "key routing contribution priority must be an integer when present",
                ));
            }
            None => None,
        };

        descriptors.push(KeyRoutingContributionDescriptor {
            command_id: command_id.to_string(),
            key_binding,
            routing_policy,
            priority,
        });
    }

    Ok(descriptors)
}

pub(super) fn parse_text_transform_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<TextTransformContributionDescriptor>, PackageRecordError> {
    const VALID_KINDS: &[&str] = &[
        "enter-rule",
        "tab-rule",
        "pair-rule",
        "comment-continuation",
        "autocomplete-trigger",
    ];

    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.textTransforms must be an array",
        ));
    };

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "text transform contribution entries must be objects",
            )
        })?;

        // Reject any executable fields.
        for forbidden in &["javascriptCallback", "code", "clientHook", "drawCallback"] {
            if obj.contains_key(*forbidden) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    format!(
                        "text transform contributions are inert manifest data and must not include `{forbidden}`"
                    ),
                ));
            }
        }

        let transform_id = obj
            .get("transformId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "text transform contribution must include a non-empty `transformId` field",
                )
            })?;

        if transform_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(transform_id),
                "text transform IDs cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(transform_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(transform_id),
                "text transform IDs must use the package apiPrefix namespace",
            ));
        }
        if !seen_ids.insert(transform_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(transform_id),
                "text transform IDs must be unique within a package",
            ));
        }

        let kind = obj
            .get("kind")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(transform_id),
                    "text transform contribution must include a `kind` field",
                )
            })?;

        if !VALID_KINDS.contains(&kind) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(transform_id),
                format!(
                    "text transform `kind` must be one of: {}",
                    VALID_KINDS.join(", ")
                ),
            ));
        }

        descriptors.push(TextTransformContributionDescriptor {
            transform_id: transform_id.to_string(),
            kind: kind.to_string(),
        });
    }

    Ok(descriptors)
}

pub(super) fn parse_package_option_contributions(
    value: &Value,
    api_prefix: &str,
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<PackageOptionContributionDescriptor>, PackageRecordError> {
    let entries = array_field(value, "clay.contributions.packageOptions", ctx)?;
    if !entries.is_empty() && !permissions.contains(&PackagePermission::PackageConfiguration) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "package option contributions require the `package-configuration` permission to be declared in clay.permissions",
        ));
    }
    let mut seen = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "package option schema payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "package option declaration", ctx)?;
        let option = package_owned_field(obj, "option", api_prefix, ctx)?;
        validate_package_option_suffix(api_prefix, option, ctx)?;
        if !seen.insert(option.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(option),
                "package option names must be unique within a package",
            ));
        }
        let value_type = required_str_field(obj, "type", ctx)?;
        if !matches!(
            value_type,
            "boolean" | "string" | "number" | "integer" | "object"
        ) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(option),
                "package option type must be boolean, string, number, integer, or object",
            ));
        }
        validate_package_option_type(api_prefix, option, value_type, ctx)?;
        let default_value = obj
            .get("default")
            .map(|value| serde_json::to_string(value).unwrap_or_default());
        if let Some(default) = obj.get("default") {
            validate_package_option_default(api_prefix, option, default, ctx)?;
        }
        descriptors.push(PackageOptionContributionDescriptor {
            option: option.to_string(),
            value_type: value_type.to_string(),
            default_value,
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

pub(super) fn validate_package_option_suffix(
    api_prefix: &str,
    option: &str,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    let suffix = option
        .strip_prefix(&format!("{api_prefix}."))
        .unwrap_or(option);
    if option
        .split('.')
        .any(|segment| segment.is_empty() || segment.starts_with('_'))
    {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(option),
            "package option names must not use hidden or empty path segments",
        ));
    }
    if !matches!(
        suffix,
        "layout.defaultVisibility"
            | "layout.defaultSlot"
            | "layout.splitRatio"
            | "input.default"
            | "action.default"
            | "themeTokenRemap"
            | "fallback"
    ) {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(option),
            "unsupported package option; use documented layout.defaultVisibility, layout.defaultSlot, layout.splitRatio, input.default, action.default, themeTokenRemap, or fallback options",
        ));
    }
    Ok(())
}

pub(super) fn validate_package_option_type(
    api_prefix: &str,
    option: &str,
    value_type: &str,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    let suffix = option
        .strip_prefix(&format!("{api_prefix}."))
        .unwrap_or(option);
    let expected = match suffix {
        "layout.defaultVisibility"
        | "layout.defaultSlot"
        | "input.default"
        | "action.default"
        | "fallback" => "string",
        "layout.splitRatio" => "number",
        "themeTokenRemap" => "object",
        _ => value_type,
    };
    if value_type != expected {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(option),
            format!("package option `{option}` must declare type `{expected}`"),
        ));
    }
    Ok(())
}

pub(super) fn validate_package_option_default(
    api_prefix: &str,
    option: &str,
    value: &Value,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    let suffix = option
        .strip_prefix(&format!("{api_prefix}."))
        .unwrap_or(option);
    match suffix {
        "layout.defaultVisibility" => {
            validate_string_choice(value, &["visible", "hidden", "collapsed"], option, ctx)
        }
        "layout.defaultSlot" => {
            validate_string_choice(value, &["left", "right", "top", "bottom"], option, ctx)
        }
        "layout.splitRatio" => {
            let Some(ratio) = value.as_f64() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(option),
                    "layout.splitRatio default must be a number",
                ));
            };
            if !(0.1..=0.9).contains(&ratio) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(option),
                    "layout.splitRatio default must be between 0.1 and 0.9",
                ));
            }
            Ok(())
        }
        "input.default" | "action.default" => {
            let Some(id) = value.as_str() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(option),
                    "input.default and action.default defaults must be package-prefixed strings",
                ));
            };
            if !is_package_owned_id(id, api_prefix) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "input.default and action.default defaults must use package-prefixed public IDs",
                ));
            }
            Ok(())
        }
        "themeTokenRemap" => {
            let object = object_field(value, "themeTokenRemap default", ctx)?;
            let token = required_str_field(object, "token", ctx)?;
            if !is_package_owned_id(token, api_prefix) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "themeTokenRemap token must use the package apiPrefix",
                ));
            }
            required_str_field(object, "fallback", ctx)?;
            Ok(())
        }
        "fallback" => {
            validate_string_choice(value, &["package-default", "hide", "ignore"], option, ctx)
        }
        _ => Ok(()),
    }
}

pub(super) fn validate_string_choice(
    value: &Value,
    allowed: &[&str],
    contribution_id: &str,
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    let Some(text) = value.as_str() else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            "package option default must be a string for this option",
        ));
    };
    if !allowed.contains(&text) {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            Some(contribution_id),
            format!(
                "package option default must be one of: {}",
                allowed.join(", ")
            ),
        ));
    }
    Ok(())
}
