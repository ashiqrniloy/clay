use std::sync::Arc;

use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde_json::{Value, json};

use crate::server::ui::{
    RegisteredComponentContribution, RegisteredLayoutIntent, RegisteredPackageInputContribution,
    RegisteredPackageLayoutOverride, RegisteredPackageThemeTokenDeclaration,
    RegisteredPackageUiStateScope, RegisteredPanelContribution,
    RegisteredTransientOverlayContribution, UiContributionDiagnostic,
};

use super::ClayOpState;

#[op2]
#[string]
pub(super) fn op_clay_ui_register_panel_contribution(
    state: &mut OpState,
    #[string] declaration_json: String,
) -> Result<String, JsErrorBox> {
    // Provenance comes from the host-owned executing-package context.
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .current_package_record()?;
    let declaration = parse_json(&declaration_json, "ui.invalid_panel_contribution")?;
    let op_state = state.borrow::<Arc<ClayOpState>>();
    let registered = op_state
        .register_panel_contribution(&package.manifest, &declaration)
        .map_err(ui_error("ui.registration_failed"))?;
    serde_json::to_string(&panel_result(&registered))
        .map_err(serialize_error("ui.registration_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_ui_register_component_contribution(
    state: &mut OpState,
    #[string] declaration_json: String,
) -> Result<String, JsErrorBox> {
    // Provenance comes from the host-owned executing-package context.
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .current_package_record()?;
    let declaration = parse_json(&declaration_json, "ui.invalid_component_contribution")?;
    let op_state = state.borrow::<Arc<ClayOpState>>();
    let registered = op_state
        .register_component_contribution(&package.manifest, &declaration)
        .map_err(ui_error("ui.registration_failed"))?;
    serde_json::to_string(&component_result(&registered))
        .map_err(serialize_error("ui.registration_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_ui_register_transient_overlay_contribution(
    state: &mut OpState,
    #[string] declaration_json: String,
) -> Result<String, JsErrorBox> {
    // Provenance comes from the host-owned executing-package context.
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .current_package_record()?;
    let declaration = parse_json(
        &declaration_json,
        "ui.invalid_transient_overlay_contribution",
    )?;
    let op_state = state.borrow::<Arc<ClayOpState>>();
    let registered = op_state
        .register_transient_overlay_contribution(&package.manifest, &declaration)
        .map_err(ui_error("ui.registration_failed"))?;
    serde_json::to_string(&overlay_result(&registered))
        .map_err(serialize_error("ui.registration_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_ui_register_input_contribution(
    state: &mut OpState,
    #[string] declaration_json: String,
) -> Result<String, JsErrorBox> {
    // Provenance comes from the host-owned executing-package context.
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .current_package_record()?;
    let declaration = parse_json(&declaration_json, "ui.invalid_input_contribution")?;
    let op_state = state.borrow::<Arc<ClayOpState>>();
    let registered = op_state
        .register_input_contribution(&package.manifest, &declaration)
        .map_err(ui_error("ui.registration_failed"))?;
    serde_json::to_string(&input_result(&registered))
        .map_err(serialize_error("ui.registration_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_ui_register_ui_state_scope(
    state: &mut OpState,
    #[string] declaration_json: String,
) -> Result<String, JsErrorBox> {
    // Provenance comes from the host-owned executing-package context.
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .current_package_record()?;
    let declaration = parse_json(&declaration_json, "ui.invalid_ui_state_scope")?;
    let op_state = state.borrow::<Arc<ClayOpState>>();
    let registered = op_state
        .register_ui_state_scope(&package.manifest, &declaration)
        .map_err(ui_error("ui.registration_failed"))?;
    serde_json::to_string(&ui_state_scope_result(&registered))
        .map_err(serialize_error("ui.registration_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_ui_set_layout_override(
    state: &mut OpState,
    #[string] declaration_json: String,
) -> Result<String, JsErrorBox> {
    let declaration = parse_json(&declaration_json, "ui.invalid_layout_override")?;
    let op_state = state.borrow::<Arc<ClayOpState>>();
    let registered = op_state
        .set_layout_override(&declaration)
        .map_err(ui_error("ui.layout_override_failed"))?;
    serde_json::to_string(&layout_override_result(&registered))
        .map_err(serialize_error("ui.layout_override_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_ui_request_layout_intent(
    state: &mut OpState,
    #[string] declaration_json: String,
) -> Result<String, JsErrorBox> {
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .current_package_record()?;
    let declaration = parse_json(&declaration_json, "ui.invalid_layout_intent")?;
    let op_state = state.borrow::<Arc<ClayOpState>>();
    let registered = op_state
        .request_layout_intent(&package.manifest, &declaration)
        .map_err(ui_error("ui.layout_intent_failed"))?;
    serde_json::to_string(&layout_intent_result(&registered))
        .map_err(serialize_error("ui.layout_intent_failed"))
}

#[op2]
#[string]
pub(super) fn op_clay_ui_register_theme_token(
    state: &mut OpState,
    #[string] declaration_json: String,
) -> Result<String, JsErrorBox> {
    // Provenance comes from the host-owned executing-package context.
    let package = state
        .borrow::<Arc<ClayOpState>>()
        .current_package_record()?;
    let declaration = parse_json(&declaration_json, "ui.invalid_theme_token")?;
    let op_state = state.borrow::<Arc<ClayOpState>>();
    let registered = op_state
        .register_theme_token(&package.manifest, &declaration)
        .map_err(ui_error("ui.registration_failed"))?;
    serde_json::to_string(&theme_token_result(&registered))
        .map_err(serialize_error("ui.registration_failed"))
}

fn parse_json(json_text: &str, code: &str) -> Result<Value, JsErrorBox> {
    serde_json::from_str(json_text)
        .map_err(|error| JsErrorBox::generic(format!("{code}: input must be valid JSON ({error})")))
}

fn ui_error(code: &'static str) -> impl Fn(UiContributionDiagnostic) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: {:?}: {}", error.rule, error.message))
}

fn serialize_error(code: &'static str) -> impl Fn(serde_json::Error) -> JsErrorBox {
    move |error| JsErrorBox::generic(format!("{code}: failed to serialize result ({error})"))
}

fn provenance_json(provenance: &crate::server::ui::UiContributionProvenance) -> serde_json::Value {
    json!({
        "packageName": provenance.package_name,
        "packageVersion": provenance.package_version,
        "apiPrefix": provenance.api_prefix,
    })
}

fn panel_result(registered: &RegisteredPanelContribution) -> serde_json::Value {
    json!({
        "registered": true,
        "id": registered.id,
        "slot": registered.slot,
        "defaultVisibility": registered.default_visibility,
        "componentId": registered.component_id,
        "actionTargets": registered.action_targets,
        "estimatedPayloadBytes": registered.estimated_payload_bytes,
        "provenance": provenance_json(&registered.provenance),
    })
}

fn component_result(registered: &RegisteredComponentContribution) -> serde_json::Value {
    json!({
        "registered": true,
        "id": registered.id,
        "rootKind": registered.root_kind,
        "componentCount": registered.component_count,
        "styleVariableCount": registered.style_variable_count,
        "actionTargets": registered.action_targets,
        "estimatedPayloadBytes": registered.estimated_payload_bytes,
        "provenance": provenance_json(&registered.provenance),
    })
}

fn overlay_result(registered: &RegisteredTransientOverlayContribution) -> serde_json::Value {
    json!({
        "registered": true,
        "id": registered.id,
        "anchor": registered.anchor,
        "focusPolicy": registered.focus_policy,
        "dismissalPolicy": registered.dismissal_policy,
        "componentId": registered.component_id,
        "actionTargets": registered.action_targets,
        "estimatedPayloadBytes": registered.estimated_payload_bytes,
        "provenance": provenance_json(&registered.provenance),
    })
}

fn input_result(registered: &RegisteredPackageInputContribution) -> serde_json::Value {
    json!({
        "registered": true,
        "id": registered.id,
        "scope": registered.scope,
        "componentId": registered.component_id,
        "pointerClick": registered.pointer_click,
        "pointerAction": registered.pointer_action,
        "pointerDrag": registered.pointer_drag,
        "focusPolicy": registered.focus_policy,
        "selectionPolicy": registered.selection_policy,
        "contextModes": registered.context_modes,
        "actionTargets": registered.action_targets,
        "estimatedPayloadBytes": registered.estimated_payload_bytes,
        "provenance": provenance_json(&registered.provenance),
    })
}

fn ui_state_scope_result(registered: &RegisteredPackageUiStateScope) -> serde_json::Value {
    json!({
        "registered": true,
        "id": registered.id,
        "scope": registered.scope,
        "owner": registered.owner,
        "lifetime": registered.lifetime,
        "persistence": registered.persistence,
        "implementationStatus": registered.implementation_status,
        "valueSchemaKind": registered.value_schema_kind,
        "targetId": registered.target_id,
        "estimatedPayloadBytes": registered.estimated_payload_bytes,
        "provenance": provenance_json(&registered.provenance),
    })
}

fn layout_override_result(registered: &RegisteredPackageLayoutOverride) -> serde_json::Value {
    json!({
        "registered": true,
        "id": registered.id,
        "targetId": registered.target_id,
        "property": registered.property,
        "value": registered.value,
        "source": registered.source,
        "precedenceRank": registered.precedence_rank,
        "estimatedPayloadBytes": registered.estimated_payload_bytes,
    })
}

fn layout_intent_result(registered: &RegisteredLayoutIntent) -> serde_json::Value {
    json!({
        "registered": true,
        "id": registered.id,
        "targetPane": registered.target_pane,
        "orientation": registered.orientation,
        "ratio": registered.ratio,
        "position": registered.position,
        "source": registered.source,
        "estimatedPayloadBytes": registered.estimated_payload_bytes,
    })
}

fn theme_token_result(registered: &RegisteredPackageThemeTokenDeclaration) -> serde_json::Value {
    json!({
        "registered": true,
        "token": registered.token,
        "type": registered.token_type,
        "fallback": registered.fallback,
        "description": registered.description,
        "resolvedCoreToken": registered.resolved_core_token,
        "estimatedPayloadBytes": registered.estimated_payload_bytes,
        "provenance": provenance_json(&registered.provenance),
    })
}
