// Auto-extracted from record.rs (Plan 090 task 4). Private submodule: ui family.
use super::*;

use std::collections::HashSet;

use serde_json::Value;

use crate::packages::permissions::PackagePermission;
use crate::perf::budgets::{SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES, SDUI_UPDATE_PAYLOAD_BUDGET_BYTES};
use crate::shell::{
    components::validate_component_kind,
    components::validate_style_variables,
    theme::{ThemeTokenResolver, ThemeTokenType, core_fallback_matches_type},
};

type ParsedUiContributions = (
    Vec<UiPanelContributionDescriptor>,
    Vec<UiComponentContributionDescriptor>,
    Vec<UiOverlayContributionDescriptor>,
);

pub(super) fn parse_sdui_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<SduiContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.sdui must be an array",
        ));
    };

    let mut seen_regions = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "SDUI contribution entries must be objects",
            )
        })?;

        // Reject executable widget fields.
        for forbidden in &[
            "widgetCallback",
            "clientJavaScript",
            "drawCallback",
            "nativeHandle",
        ] {
            if obj.contains_key(*forbidden) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    format!(
                        "SDUI contributions must not include client-side or native-widget fields (`{forbidden}`)"
                    ),
                ));
            }
        }

        let region_id = obj
            .get("regionId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "SDUI contribution must include a non-empty `regionId` field",
                )
            })?;

        if region_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(region_id),
                "SDUI contribution regionIds cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(region_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(region_id),
                "SDUI contribution regionIds must use the package apiPrefix namespace",
            ));
        }
        if !seen_regions.insert(region_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(region_id),
                "SDUI contribution regionIds must be unique within a package",
            ));
        }

        let display_name = obj
            .get("displayName")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(region_id),
                    "SDUI contribution must include a non-empty `displayName` field",
                )
            })?;

        let estimated_snapshot_bytes = obj
            .get("estimatedSnapshotBytes")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or_else(|| {
                serde_json::to_vec(entry)
                    .map(|bytes| bytes.len())
                    .unwrap_or(usize::MAX)
            });
        if estimated_snapshot_bytes > SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                Some(region_id),
                format!(
                    "SDUI snapshot payload estimate ({estimated_snapshot_bytes} bytes) exceeds SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES ({SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }

        let estimated_update_bytes = obj
            .get("estimatedUpdateBytes")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(estimated_snapshot_bytes);
        if estimated_update_bytes > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                Some(region_id),
                format!(
                    "SDUI update payload estimate ({estimated_update_bytes} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }

        descriptors.push(SduiContributionDescriptor {
            region_id: region_id.to_string(),
            display_name: display_name.to_string(),
            estimated_snapshot_bytes,
            estimated_update_bytes,
        });
    }

    Ok(descriptors)
}

pub(super) fn parse_decoration_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<DecorationContributionDescriptor>, PackageRecordError> {
    let Value::Array(entries) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.decorations must be an array",
        ));
    };

    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let obj = entry.as_object().ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "decoration contribution entries must be objects",
            )
        })?;
        for forbidden in &["drawCallback", "clientJavaScript", "nativeHandle"] {
            if obj.contains_key(*forbidden) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    format!(
                        "decoration contributions are inert and must not include `{forbidden}`"
                    ),
                ));
            }
        }
        let primitive_id = obj
            .get("primitiveId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    None,
                    "decoration contribution must include a non-empty `primitiveId` field",
                )
            })?;
        if primitive_id.starts_with("clay.") {
            return Err(ctx.error(
                PackageRecordRule::ReservedClayIdInContribution,
                Some(primitive_id),
                "decoration primitive IDs cannot claim the reserved clay.* namespace",
            ));
        }
        if !is_package_owned_id(primitive_id, api_prefix) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(primitive_id),
                "decoration primitive IDs must use the package apiPrefix namespace",
            ));
        }
        if !seen_ids.insert(primitive_id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(primitive_id),
                "decoration primitive IDs must be unique within a package",
            ));
        }
        let kind = obj
            .get("kind")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(primitive_id),
                    "decoration contribution must include a non-empty `kind` field",
                )
            })?;
        descriptors.push(DecorationContributionDescriptor {
            primitive_id: primitive_id.to_string(),
            kind: kind.to_string(),
        });
    }

    Ok(descriptors)
}

pub(super) fn parse_ui_contributions(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_resolver: &ThemeTokenResolver,
    ctx: &ErrorContext,
) -> Result<ParsedUiContributions, PackageRecordError> {
    let Value::Object(map) = value else {
        return Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            "clay.contributions.ui must be an object with panels, components, and overlays arrays",
        ));
    };

    let components = match map.get("components") {
        Some(v) => parse_ui_component_contributions(
            v,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?,
        None => Vec::new(),
    };
    let panels = match map.get("panels") {
        Some(v) => parse_ui_panel_contributions(
            v,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?,
        None => Vec::new(),
    };
    let overlays = match map.get("overlays") {
        Some(v) => parse_ui_overlay_contributions(
            v,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?,
        None => Vec::new(),
    };
    Ok((panels, components, overlays))
}

pub(super) fn parse_ui_panel_contributions(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_resolver: &ThemeTokenResolver,
    ctx: &ErrorContext,
) -> Result<Vec<UiPanelContributionDescriptor>, PackageRecordError> {
    const VALID_SLOTS: &[&str] = &["left", "right", "top", "bottom"];
    const VALID_VISIBILITY: &[&str] = &["visible", "hidden", "collapsed"];
    let entries = array_field(value, "clay.contributions.ui.panels", ctx)?;
    let mut seen_ids = HashSet::new();
    let mut seen_slots = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "panel contribution payload ({size} bytes) exceeds SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES ({SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "panel contribution", ctx)?;
        let id = package_owned_field(obj, "id", api_prefix, ctx)?;
        if !seen_ids.insert(id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "panel contribution IDs must be unique within a package",
            ));
        }
        let kind = obj.get("kind").and_then(Value::as_str).unwrap_or("fixed");
        if kind != "fixed" {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "Phase 18.3 panel contributions support kind `fixed`; transient UI must use overlays",
            ));
        }
        let slot = required_str_field(obj, "slot", ctx)?;
        if !VALID_SLOTS.contains(&slot) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "panel slot must be one of left, right, top, or bottom",
            ));
        }
        if !seen_slots.insert(slot.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "fixed panel contributions cannot claim the same shell slot within one package",
            ));
        }
        let default_visibility = obj
            .get("defaultVisibility")
            .and_then(Value::as_str)
            .unwrap_or("hidden");
        if !VALID_VISIBILITY.contains(&default_visibility) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "panel defaultVisibility must be visible, hidden, or collapsed",
            ));
        }
        let actions = string_vec_field(obj.get("actionTargets"), "actionTargets", ctx)?;
        validate_registered_action_targets(&actions, registered_command_ids, ctx)?;
        let component = obj.get("component").ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "panel contribution must include a component object",
            )
        })?;
        let summary = validate_ui_component_tree(
            component,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?;
        descriptors.push(UiPanelContributionDescriptor {
            id: id.to_string(),
            slot: slot.to_string(),
            component_id: summary.root_id,
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

pub(super) fn parse_ui_component_contributions(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_resolver: &ThemeTokenResolver,
    ctx: &ErrorContext,
) -> Result<Vec<UiComponentContributionDescriptor>, PackageRecordError> {
    let entries = array_field(value, "clay.contributions.ui.components", ctx)?;
    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "component contribution payload ({size} bytes) exceeds SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES ({SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let summary = validate_ui_component_tree(
            entry,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?;
        if !seen_ids.insert(summary.root_id.clone()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(&summary.root_id),
                "component contribution root IDs must be unique within a package",
            ));
        }
        descriptors.push(UiComponentContributionDescriptor {
            id: summary.root_id,
            root_kind: summary.root_kind,
            component_count: summary.component_count,
            style_variable_count: summary.style_variable_count,
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

pub(super) fn parse_ui_overlay_contributions(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_resolver: &ThemeTokenResolver,
    ctx: &ErrorContext,
) -> Result<Vec<UiOverlayContributionDescriptor>, PackageRecordError> {
    const VALID_ANCHORS: &[&str] = &["working-area", "active-pane", "main", "pointer"];
    const VALID_FOCUS: &[&str] = &["none", "restore", "trap"];
    const VALID_DISMISSAL: &[&str] = &["manual", "escape", "outside", "escape-or-outside"];
    let entries = array_field(value, "clay.contributions.ui.overlays", ctx)?;
    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "overlay contribution payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "overlay contribution", ctx)?;
        let id = package_owned_field(obj, "id", api_prefix, ctx)?;
        if !seen_ids.insert(id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "overlay contribution IDs must be unique within a package",
            ));
        }
        let anchor = obj
            .get("anchor")
            .and_then(Value::as_str)
            .unwrap_or("working-area");
        if !VALID_ANCHORS.contains(&anchor) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "overlay anchor must be one of working-area, active-pane, main, or pointer",
            ));
        }
        let focus_policy = obj
            .get("focusPolicy")
            .and_then(Value::as_str)
            .unwrap_or("restore");
        if !VALID_FOCUS.contains(&focus_policy) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "overlay focusPolicy must be none, restore, or trap",
            ));
        }
        let dismissal_policy = obj
            .get("dismissalPolicy")
            .and_then(Value::as_str)
            .unwrap_or("escape");
        if !VALID_DISMISSAL.contains(&dismissal_policy) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "overlay dismissalPolicy must be manual, escape, outside, or escape-or-outside",
            ));
        }
        let actions = string_vec_field(obj.get("actionTargets"), "actionTargets", ctx)?;
        validate_registered_action_targets(&actions, registered_command_ids, ctx)?;
        let component = obj.get("component").ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "overlay contribution must include a component object",
            )
        })?;
        let summary = validate_ui_component_tree(
            component,
            api_prefix,
            registered_command_ids,
            theme_resolver,
            ctx,
        )?;
        descriptors.push(UiOverlayContributionDescriptor {
            id: id.to_string(),
            anchor: anchor.to_string(),
            focus_policy: focus_policy.to_string(),
            dismissal_policy: dismissal_policy.to_string(),
            component_id: summary.root_id,
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

pub(super) fn parse_input_contributions(
    value: &Value,
    api_prefix: &str,
    package_modes: &[String],
    registered_command_ids: &[String],
    ctx: &ErrorContext,
) -> Result<Vec<InputContributionDescriptor>, PackageRecordError> {
    const VALID_SCOPES: &[&str] = &["component", "panel", "overlay"];
    const VALID_POINTER_CLICK: &[&str] = &["none", "focus", "action", "select"];
    const VALID_POINTER_DRAG: &[&str] = &["none", "select", "pan"];
    const VALID_FOCUS: &[&str] = &["none", "restore-editor", "focus-component", "trap"];
    const VALID_SELECTION: &[&str] = &["preserve-editor", "component-local", "disabled"];

    let entries = array_field(value, "clay.contributions.input", ctx)?;
    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "input contribution payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "input contribution", ctx)?;
        if obj.contains_key("keys") || obj.contains_key("keybindings") || obj.contains_key("onKey")
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                None,
                "package input contributions must not declare key routing; use behavior manifests and clay:keybindings",
            ));
        }
        let id = package_owned_field(obj, "id", api_prefix, ctx)?;
        if !seen_ids.insert(id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "input contribution IDs must be unique within a package",
            ));
        }
        let scope = required_str_field(obj, "scope", ctx)?;
        if !VALID_SCOPES.contains(&scope) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "input scope must be component, panel, or overlay",
            ));
        }
        let component_id = package_owned_field(obj, "componentId", api_prefix, ctx)?;
        let pointer = obj.get("pointer").and_then(Value::as_object);
        let pointer_click = pointer
            .and_then(|p| p.get("click"))
            .and_then(Value::as_str)
            .unwrap_or("none");
        if !VALID_POINTER_CLICK.contains(&pointer_click) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "pointer.click must be none, focus, action, or select",
            ));
        }
        let pointer_drag = pointer
            .and_then(|p| p.get("drag"))
            .and_then(Value::as_str)
            .unwrap_or("none");
        if !VALID_POINTER_DRAG.contains(&pointer_drag) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "pointer.drag must be none, select, or pan",
            ));
        }
        let pointer_action = pointer
            .and_then(|p| p.get("action"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned);
        if pointer_click == "action" && pointer_action.is_none() {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "pointer.click=action requires a registered pointer.action command ID",
            ));
        }
        let focus = obj.get("focus").and_then(Value::as_object);
        let focus_policy = focus
            .and_then(|f| f.get("policy"))
            .and_then(Value::as_str)
            .unwrap_or("restore-editor");
        if !VALID_FOCUS.contains(&focus_policy) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "focus.policy must be none, restore-editor, focus-component, or trap",
            ));
        }
        let selection_policy = obj
            .get("selectionPolicy")
            .and_then(Value::as_str)
            .unwrap_or("preserve-editor");
        if !VALID_SELECTION.contains(&selection_policy) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "selectionPolicy must be preserve-editor, component-local, or disabled",
            ));
        }
        if let Some(context) = obj.get("context") {
            let context = object_field(context, "input context", ctx)?;
            for mode in string_vec_field(context.get("modes"), "context.modes", ctx)? {
                if !package_modes.iter().any(|declared| declared == &mode) {
                    return Err(ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(&mode),
                        "input context modes must be declared by the package manifest",
                    ));
                }
            }
        }
        let mut action_targets = string_vec_field(obj.get("actionTargets"), "actionTargets", ctx)?;
        if let Some(action) = pointer_action {
            action_targets.push(action);
        }
        validate_registered_action_targets(&action_targets, registered_command_ids, ctx)?;
        action_targets.sort();
        action_targets.dedup();

        descriptors.push(InputContributionDescriptor {
            id: id.to_string(),
            scope: scope.to_string(),
            component_id: component_id.to_string(),
            action_targets,
            estimated_payload_bytes: size,
        });
    }

    Ok(descriptors)
}

pub(super) fn parse_ui_state_scope_contributions(
    value: &Value,
    api_prefix: &str,
    ctx: &ErrorContext,
) -> Result<Vec<UiStateScopeContributionDescriptor>, PackageRecordError> {
    const VALID_SCOPES: &[&str] = &[
        "package-global",
        "user-config",
        "workspace",
        "document",
        "pane",
        "component",
        "transient-overlay",
    ];
    const VALID_OWNERS: &[&str] = &["package", "shell", "server"];
    const VALID_LIFETIMES: &[&str] = &["session", "workspace", "document", "transient"];
    const VALID_PERSISTENCE: &[&str] = &["none", "client-local", "server-canonical", "deferred"];
    const VALID_STATUS: &[&str] = &["implemented", "deferred"];
    const VALID_SCHEMA_KINDS: &[&str] = &["boolean", "number", "string", "enum", "object"];

    let entries = array_field(value, "clay.contributions.uiStateScopes", ctx)?;
    let mut seen_ids = HashSet::new();
    let mut descriptors = Vec::with_capacity(entries.len());

    for entry in entries {
        let size = contribution_payload_size(entry);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!(
                    "UI state scope declaration payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "UI state scope declaration", ctx)?;
        let id = package_owned_field(obj, "id", api_prefix, ctx)?;
        if id
            .split('.')
            .any(|segment| segment.is_empty() || segment.starts_with('_'))
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state scope IDs must not use hidden or empty path segments",
            ));
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "UI state scope IDs must be unique within a package",
            ));
        }
        let scope = required_str_field(obj, "scope", ctx)?;
        if !VALID_SCOPES.contains(&scope) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state scope must be package-global, user-config, workspace, document, pane, component, or transient-overlay",
            ));
        }
        let owner = required_str_field(obj, "owner", ctx)?;
        if !VALID_OWNERS.contains(&owner) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state owner must be package, shell, or server",
            ));
        }
        let lifetime = required_str_field(obj, "lifetime", ctx)?;
        if !VALID_LIFETIMES.contains(&lifetime) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state lifetime must be session, workspace, document, or transient",
            ));
        }
        let persistence = required_str_field(obj, "persistence", ctx)?;
        if !VALID_PERSISTENCE.contains(&persistence) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state persistence must be none, client-local, server-canonical, or deferred",
            ));
        }
        let implementation_status = obj
            .get("implementationStatus")
            .and_then(Value::as_str)
            .unwrap_or("deferred");
        if !VALID_STATUS.contains(&implementation_status) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "implementationStatus must be implemented or deferred",
            ));
        }
        let target_id = obj
            .get("targetId")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned);
        if matches!(scope, "pane" | "component" | "transient-overlay") && target_id.is_none() {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "pane, component, and transient-overlay state scopes require a package-prefixed targetId",
            ));
        }
        if let Some(target) = &target_id
            && !is_package_owned_id(target, api_prefix)
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(target),
                "state scope targetId must use the package apiPrefix",
            ));
        }
        if implementation_status == "implemented"
            && matches!(scope, "workspace" | "document" | "user-config")
            && persistence != "client-local"
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "workspace, document, and user-config UI state persistence remains deferred unless explicitly declared client-local",
            ));
        }
        let value_schema = obj.get("valueSchema").ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state scopes require a bounded valueSchema object",
            )
        })?;
        reject_ui_prohibited_authority(value_schema, ctx)?;
        let schema = object_field(value_schema, "valueSchema", ctx)?;
        if schema.contains_key("defaultValue")
            || schema.contains_key("initialValue")
            || schema.contains_key("rawValue")
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "UI state scope declarations define schemas only; state values are not accepted during registration",
            ));
        }
        let value_schema_kind = required_str_field(schema, "kind", ctx)?;
        if !VALID_SCHEMA_KINDS.contains(&value_schema_kind) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "valueSchema.kind must be boolean, number, string, enum, or object",
            ));
        }
        if value_schema_kind == "enum" {
            let values = string_vec_field(schema.get("values"), "valueSchema.values", ctx)?;
            if values.is_empty() || values.len() > 32 {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "enum valueSchema.values must include 1 to 32 string values",
                ));
            }
        }
        descriptors.push(UiStateScopeContributionDescriptor {
            id: id.to_string(),
            scope: scope.to_string(),
            owner: owner.to_string(),
            lifetime: lifetime.to_string(),
            persistence: persistence.to_string(),
            implementation_status: implementation_status.to_string(),
            value_schema_kind: value_schema_kind.to_string(),
            target_id,
            estimated_payload_bytes: size,
        });
    }

    Ok(descriptors)
}

pub(super) fn parse_layout_override_contributions(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_tokens: &[ThemeTokenContributionDescriptor],
    input_contributions: &[InputContributionDescriptor],
    permissions: &[PackagePermission],
    ctx: &ErrorContext,
) -> Result<Vec<LayoutOverrideContributionDescriptor>, PackageRecordError> {
    const VALID_PROPERTIES: &[&str] = &[
        "slot",
        "visibility",
        "splitRatio",
        "themeToken",
        "inputDefault",
        "actionDefault",
        "fallback",
    ];
    const VALID_SOURCES: &[&str] = &["global-package", "package-default"];

    let entries = array_field(value, "clay.contributions.layoutOverrides", ctx)?;
    if !entries.is_empty() && !permissions.contains(&PackagePermission::PackageConfiguration) {
        return Err(ctx.error(
            PackageRecordRule::UndeclaredPermissionForContribution,
            None,
            "layout override contributions require the `package-configuration` permission to be declared in clay.permissions",
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
                    "layout override payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_ui_prohibited_authority(entry, ctx)?;
        let obj = object_field(entry, "layout override declaration", ctx)?;
        let target_id = package_owned_field(obj, "targetId", api_prefix, ctx)?;
        let property = required_str_field(obj, "property", ctx)?;
        if !VALID_PROPERTIES.contains(&property) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(property),
                "layout override property must be slot, visibility, splitRatio, themeToken, inputDefault, actionDefault, or fallback",
            ));
        }
        let source = obj
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("package-default");
        if !VALID_SOURCES.contains(&source) {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(source),
                "manifest layout override source must be global-package or package-default; user and mode overrides flow through documented configuration APIs",
            ));
        }
        let value = obj.get("value").ok_or_else(|| {
            ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(target_id),
                "layout override requires a typed value",
            )
        })?;
        validate_layout_override_contribution_value(
            property,
            target_id,
            value,
            registered_command_ids,
            theme_tokens,
            input_contributions,
            ctx,
        )?;
        let key = format!("{target_id}:{property}");
        if !seen.insert(key) {
            return Err(ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(target_id),
                "layout override targets and properties must be unique within a package",
            ));
        }
        descriptors.push(LayoutOverrideContributionDescriptor {
            target_id: target_id.to_string(),
            property: property.to_string(),
            source: source.to_string(),
            estimated_payload_bytes: size,
        });
    }
    Ok(descriptors)
}

pub(super) struct UiComponentSummary {
    root_id: String,
    root_kind: String,
    component_count: usize,
    style_variable_count: usize,
}

pub(super) fn validate_ui_component_tree(
    value: &Value,
    api_prefix: &str,
    registered_command_ids: &[String],
    theme_resolver: &ThemeTokenResolver,
    ctx: &ErrorContext,
) -> Result<UiComponentSummary, PackageRecordError> {
    let mut state = UiComponentValidationState {
        api_prefix,
        registered_command_ids,
        theme_resolver,
        ctx,
        seen_ids: HashSet::new(),
        component_count: 0,
        style_variable_count: 0,
    };
    let (root_id, root_kind) = state.validate_node(value)?;
    Ok(UiComponentSummary {
        root_id,
        root_kind,
        component_count: state.component_count,
        style_variable_count: state.style_variable_count,
    })
}

struct UiComponentValidationState<'a> {
    api_prefix: &'a str,
    registered_command_ids: &'a [String],
    theme_resolver: &'a ThemeTokenResolver,
    ctx: &'a ErrorContext,
    seen_ids: HashSet<String>,
    component_count: usize,
    style_variable_count: usize,
}

impl UiComponentValidationState<'_> {
    fn validate_node(&mut self, value: &Value) -> Result<(String, String), PackageRecordError> {
        const MAX_COMPONENT_NODES: usize = 128;
        self.component_count += 1;
        if self.component_count > MAX_COMPONENT_NODES {
            return Err(self.ctx.error(
                PackageRecordRule::PayloadBudgetExceeded,
                None,
                format!("component tree exceeds {MAX_COMPONENT_NODES} nodes"),
            ));
        }
        reject_ui_prohibited_authority(value, self.ctx)?;
        let obj = object_field(value, "component", self.ctx)?;
        let kind = required_str_field(obj, "kind", self.ctx)?;
        let component_kind = validate_component_kind(kind).map_err(|error| {
            self.ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(&error.field),
                error.message,
            )
        })?;
        let id = package_owned_field(obj, "id", self.api_prefix, self.ctx)?;
        if !self.seen_ids.insert(id.to_string()) {
            return Err(self.ctx.error(
                PackageRecordRule::DuplicateContributionId,
                Some(id),
                "component IDs must be unique within a package UI contribution tree",
            ));
        }
        if obj.contains_key("styleString") || obj.contains_key("className") {
            return Err(self.ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "component declarations must use typed style variables, not raw CSS/style strings or class names",
            ));
        }
        let style_variables =
            validate_style_variables(obj, self.theme_resolver).map_err(|error| {
                self.ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(&error.field),
                    error.message,
                )
            })?;
        if style_variables
            .iter()
            .any(|variable| variable.name == "fontRole")
            && !component_kind.supports_text_font_role()
        {
            return Err(self.ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(id),
                "style.fontRole is only supported by text-bearing panel, label, button, list, and statusItem components",
            ));
        }
        self.style_variable_count += style_variables.len();
        if let Some(action) = obj.get("action").and_then(Value::as_object) {
            let command_id = required_str_field(action, "commandId", self.ctx)?;
            validate_registered_action_targets(
                &[command_id.to_string()],
                self.registered_command_ids,
                self.ctx,
            )?;
        }
        if let Some(items) = obj.get("items") {
            let items = items.as_array().ok_or_else(|| {
                self.ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "component items must be an array",
                )
            })?;
            for item in items {
                let item_object = item.as_object().ok_or_else(|| {
                    self.ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(id),
                        "component list items must be objects",
                    )
                })?;
                if let Some(action) = item_object.get("action").and_then(Value::as_object) {
                    let command_id = required_str_field(action, "commandId", self.ctx)?;
                    validate_registered_action_targets(
                        &[command_id.to_string()],
                        self.registered_command_ids,
                        self.ctx,
                    )?;
                }
            }
        }
        if let Some(children) = obj.get("children") {
            let children = children.as_array().ok_or_else(|| {
                self.ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(id),
                    "component children must be an array",
                )
            })?;
            for child in children {
                self.validate_node(child)?;
            }
        }
        Ok((id.to_string(), kind.to_string()))
    }
}

pub(super) fn validate_layout_override_contribution_value(
    property: &str,
    target_id: &str,
    value: &Value,
    registered_command_ids: &[String],
    theme_tokens: &[ThemeTokenContributionDescriptor],
    input_contributions: &[InputContributionDescriptor],
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    match property {
        "slot" => {
            let Some(slot) = value.as_str() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "slot override value must be a string",
                ));
            };
            if !matches!(slot, "left" | "right" | "top" | "bottom") {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(slot),
                    "slot override value must be left, right, top, or bottom",
                ));
            }
        }
        "visibility" => {
            let Some(visibility) = value.as_str() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "visibility override value must be a string",
                ));
            };
            if !matches!(visibility, "visible" | "hidden" | "collapsed") {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(visibility),
                    "visibility override value must be visible, hidden, or collapsed",
                ));
            }
        }
        "splitRatio" => {
            let Some(ratio) = value.as_f64() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "splitRatio override value must be a number",
                ));
            };
            if !(0.1..=0.9).contains(&ratio) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "splitRatio override value must be between 0.1 and 0.9",
                ));
            }
        }
        "themeToken" => {
            let obj = object_field(value, "themeToken override value", ctx)?;
            let token = required_str_field(obj, "token", ctx)?;
            if !is_package_owned_id(token, target_package_prefix(target_id)) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "themeToken override token must use the target package prefix",
                ));
            }
            let fallback = required_str_field(obj, "fallback", ctx)?;
            let declared = theme_tokens
                .iter()
                .find(|declared| declared.token == token)
                .ok_or_else(|| {
                    ctx.error(
                        PackageRecordRule::InvalidContributionDescriptor,
                        Some(token),
                        "themeToken override token must be declared in clay.contributions.themeTokens",
                    )
                })?;
            let Some(token_type) = ThemeTokenType::parse(&declared.token_type) else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(token),
                    "theme token declaration has an invalid type",
                ));
            };
            if !core_fallback_matches_type(fallback, token_type) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(fallback),
                    "themeToken fallback must reference a known Clay core token with the same type",
                ));
            }
        }
        "inputDefault" => {
            let obj = object_field(value, "inputDefault override value", ctx)?;
            let input_id = required_str_field(obj, "inputId", ctx)?;
            if !input_contributions.iter().any(|input| input.id == input_id) {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(input_id),
                    "inputDefault.inputId must reference a declared package input contribution",
                ));
            }
        }
        "actionDefault" => {
            let Some(action_id) = value.as_str() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "actionDefault override value must be a registered action ID string",
                ));
            };
            if !registered_command_ids
                .iter()
                .any(|command| command == action_id)
            {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(action_id),
                    "actionDefault must reference a command declared in clay.contributions.commands",
                ));
            }
        }
        "fallback" => {
            let Some(fallback) = value.as_str() else {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(target_id),
                    "fallback override value must be a string",
                ));
            };
            if !matches!(fallback, "package-default" | "hide" | "ignore") {
                return Err(ctx.error(
                    PackageRecordRule::InvalidContributionDescriptor,
                    Some(fallback),
                    "fallback override value must be package-default, hide, or ignore",
                ));
            }
        }
        _ => unreachable!("layout override property validated before value validation"),
    }
    Ok(())
}

pub(super) fn target_package_prefix(target_id: &str) -> &str {
    target_id.split('.').next().unwrap_or(target_id)
}

pub(super) fn string_vec_field(
    value: Option<&Value>,
    key: &str,
    ctx: &ErrorContext,
) -> Result<Vec<String>, PackageRecordError> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|text| !text.trim().is_empty())
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| {
                        ctx.error(
                            PackageRecordRule::InvalidContributionDescriptor,
                            None,
                            format!("{key} entries must be non-empty strings"),
                        )
                    })
            })
            .collect(),
        _ => Err(ctx.error(
            PackageRecordRule::InvalidContributionDescriptor,
            None,
            format!("{key} must be an array"),
        )),
    }
}

pub(super) fn validate_registered_action_targets(
    action_targets: &[String],
    registered_command_ids: &[String],
    ctx: &ErrorContext,
) -> Result<(), PackageRecordError> {
    for command_id in action_targets {
        if !registered_command_ids
            .iter()
            .any(|registered| registered == command_id)
        {
            return Err(ctx.error(
                PackageRecordRule::InvalidContributionDescriptor,
                Some(command_id),
                format!(
                    "UI action target `{command_id}` must be declared in clay.contributions.commands"
                ),
            ));
        }
    }
    Ok(())
}
