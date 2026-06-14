//! Runtime-backed package UI contribution registry and validators.
//!
//! The public package boundary is the `clay:ui` JavaScript facade.  This module
//! is deliberately crate-internal: it validates inert package UI declarations,
//! preserves package provenance, and stores accepted declarations for later
//! client publication without exposing Masonry widgets, raw ops, CSS, or
//! executable package code.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::{
    packages::manifest::{ClayPackageManifest, is_valid_api_prefix},
    perf::budgets::{SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES, SDUI_UPDATE_PAYLOAD_BUDGET_BYTES},
    shell::{
        FixedPackagePanel, FixedSlotId, PackageOverlayAnchor, PackagePanelVisibility,
        PackageUiComponentTree, PackageUiRuntimeUpdate, TransientPackageOverlay,
        components::{validate_component_kind, validate_style_variables},
        theme::{
            PackageThemeToken, ThemeTokenResolver, ThemeTokenType, core_fallback_matches_type,
        },
    },
};

const MAX_COMPONENT_NODES: usize = 128;
const VALID_SLOTS: &[&str] = &["left", "right", "top", "bottom"];
const VALID_VISIBILITY: &[&str] = &["visible", "hidden", "collapsed"];
const VALID_OVERLAY_ANCHORS: &[&str] = &["working-area", "active-pane", "main", "pointer"];
const VALID_FOCUS_POLICIES: &[&str] = &["none", "restore", "trap"];
const VALID_DISMISSAL_POLICIES: &[&str] = &["manual", "escape", "outside", "escape-or-outside"];
const VALID_INPUT_SCOPES: &[&str] = &["component", "panel", "overlay"];
const VALID_POINTER_CLICK_POLICIES: &[&str] = &["none", "focus", "action", "select"];
const VALID_POINTER_DRAG_POLICIES: &[&str] = &["none", "select", "pan"];
const VALID_COMPONENT_FOCUS_POLICIES: &[&str] =
    &["none", "restore-editor", "focus-component", "trap"];
const VALID_SELECTION_POLICIES: &[&str] = &["preserve-editor", "component-local", "disabled"];
const VALID_UI_STATE_SCOPES: &[&str] = &[
    "package-global",
    "user-config",
    "workspace",
    "document",
    "pane",
    "component",
    "transient-overlay",
];
const VALID_UI_STATE_OWNERS: &[&str] = &["package", "shell", "server"];
const VALID_UI_STATE_LIFETIMES: &[&str] = &["session", "workspace", "document", "transient"];
const VALID_UI_STATE_PERSISTENCE: &[&str] =
    &["none", "client-local", "server-canonical", "deferred"];
const VALID_UI_STATE_STATUSES: &[&str] = &["implemented", "deferred"];
const VALID_UI_STATE_SCHEMA_KINDS: &[&str] = &["boolean", "number", "string", "enum", "object"];
const VALID_LAYOUT_OVERRIDE_PROPERTIES: &[&str] = &[
    "slot",
    "visibility",
    "splitRatio",
    "themeToken",
    "inputDefault",
    "actionDefault",
    "fallback",
];
const VALID_LAYOUT_OVERRIDE_SOURCES: &[&str] = &[
    "user-config",
    "active-major-mode",
    "compatible-minor-mode",
    "global-package",
    "package-default",
];
const VALID_FALLBACK_BEHAVIORS: &[&str] = &["package-default", "hide", "ignore"];
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PackageUiRegistry {
    panels: BTreeMap<String, RegisteredPanelContribution>,
    components: BTreeMap<String, RegisteredComponentContribution>,
    overlays: BTreeMap<String, RegisteredTransientOverlayContribution>,
    theme_tokens: BTreeMap<String, RegisteredPackageThemeTokenDeclaration>,
    input_contributions: BTreeMap<String, RegisteredPackageInputContribution>,
    ui_state_scopes: BTreeMap<String, RegisteredPackageUiStateScope>,
    layout_overrides: BTreeMap<String, RegisteredPackageLayoutOverride>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PackageUiRegistrySnapshot {
    pub(crate) panels: Vec<RegisteredPanelContribution>,
    pub(crate) components: Vec<RegisteredComponentContribution>,
    pub(crate) overlays: Vec<RegisteredTransientOverlayContribution>,
    pub(crate) theme_tokens: Vec<RegisteredPackageThemeTokenDeclaration>,
    pub(crate) input_contributions: Vec<RegisteredPackageInputContribution>,
    pub(crate) ui_state_scopes: Vec<RegisteredPackageUiStateScope>,
    pub(crate) layout_overrides: Vec<RegisteredPackageLayoutOverride>,
}

impl PackageUiRegistrySnapshot {
    pub(crate) fn runtime_update(&self, base_version: u64) -> PackageUiRuntimeUpdate {
        PackageUiRuntimeUpdate {
            base_version,
            fixed_panels: self
                .panels
                .iter()
                .map(|panel| {
                    FixedPackagePanel::new(
                        panel.id.clone(),
                        fixed_slot_id(&panel.slot),
                        PackagePanelVisibility::parse(&panel.default_visibility),
                        panel.component_tree.clone(),
                        panel.action_targets.clone(),
                    )
                })
                .collect(),
            transient_overlays: self
                .overlays
                .iter()
                .map(|overlay| {
                    TransientPackageOverlay::new(
                        overlay.id.clone(),
                        PackageOverlayAnchor::parse(&overlay.anchor),
                        overlay.focus_policy.clone(),
                        overlay.dismissal_policy.clone(),
                        overlay.component_tree.clone(),
                        overlay.action_targets.clone(),
                    )
                })
                .collect(),
            input_routing: self
                .input_contributions
                .iter()
                .map(|input| {
                    crate::shell::PackageInputRouting::new(
                        input.id.clone(),
                        input.scope.clone(),
                        input.component_id.clone(),
                        input.pointer_click.clone(),
                        input.pointer_action.clone(),
                        input.pointer_drag.clone(),
                        input.focus_policy.clone(),
                        input.selection_policy.clone(),
                        input.context_modes.clone(),
                        input.action_targets.clone(),
                    )
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UiContributionProvenance {
    pub(crate) package_name: String,
    pub(crate) package_version: String,
    pub(crate) api_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredPanelContribution {
    pub(crate) id: String,
    pub(crate) slot: String,
    pub(crate) default_visibility: String,
    pub(crate) component_id: String,
    pub(crate) component_tree: PackageUiComponentTree,
    pub(crate) action_targets: Vec<String>,
    pub(crate) provenance: UiContributionProvenance,
    pub(crate) estimated_payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredComponentContribution {
    pub(crate) id: String,
    pub(crate) root_kind: String,
    pub(crate) component_count: usize,
    pub(crate) style_variable_count: usize,
    pub(crate) action_targets: Vec<String>,
    pub(crate) provenance: UiContributionProvenance,
    pub(crate) estimated_payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredTransientOverlayContribution {
    pub(crate) id: String,
    pub(crate) anchor: String,
    pub(crate) focus_policy: String,
    pub(crate) dismissal_policy: String,
    pub(crate) component_id: String,
    pub(crate) component_tree: PackageUiComponentTree,
    pub(crate) action_targets: Vec<String>,
    pub(crate) provenance: UiContributionProvenance,
    pub(crate) estimated_payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredPackageThemeTokenDeclaration {
    pub(crate) token: String,
    pub(crate) token_type: String,
    pub(crate) fallback: String,
    pub(crate) description: String,
    pub(crate) resolved_core_token: String,
    pub(crate) provenance: UiContributionProvenance,
    pub(crate) estimated_payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredPackageInputContribution {
    pub(crate) id: String,
    pub(crate) scope: String,
    pub(crate) component_id: String,
    pub(crate) pointer_click: String,
    pub(crate) pointer_action: Option<String>,
    pub(crate) pointer_drag: String,
    pub(crate) focus_policy: String,
    pub(crate) selection_policy: String,
    pub(crate) context_modes: Vec<String>,
    pub(crate) action_targets: Vec<String>,
    pub(crate) provenance: UiContributionProvenance,
    pub(crate) estimated_payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredPackageUiStateScope {
    pub(crate) id: String,
    pub(crate) scope: String,
    pub(crate) owner: String,
    pub(crate) lifetime: String,
    pub(crate) persistence: String,
    pub(crate) implementation_status: String,
    pub(crate) value_schema_kind: String,
    pub(crate) value_schema: Value,
    pub(crate) target_id: Option<String>,
    pub(crate) provenance: UiContributionProvenance,
    pub(crate) estimated_payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredPackageLayoutOverride {
    pub(crate) id: String,
    pub(crate) target_id: String,
    pub(crate) property: String,
    pub(crate) value: Value,
    pub(crate) source: String,
    pub(crate) precedence_rank: u8,
    pub(crate) estimated_payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UiContributionDiagnostic {
    pub(crate) package_name: Option<String>,
    pub(crate) package_version: Option<String>,
    pub(crate) api_prefix: Option<String>,
    pub(crate) contribution_id: Option<String>,
    pub(crate) rule: UiContributionRule,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UiContributionRule {
    InvalidProvenance,
    InvalidId,
    DuplicateId,
    InvalidSlot,
    InvalidPolicy,
    InvalidComponent,
    InvalidActionTarget,
    InvalidInputScope,
    InvalidFocusPolicy,
    InvalidStateScope,
    InvalidLifecycle,
    InvalidStateSchema,
    PayloadTooLarge,
    ProhibitedAuthority,
    InvalidThemeToken,
    InvalidLayoutOverride,
}

impl PackageUiRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn snapshot(&self) -> PackageUiRegistrySnapshot {
        PackageUiRegistrySnapshot {
            panels: self.panels.values().cloned().collect(),
            components: self.components.values().cloned().collect(),
            overlays: self.overlays.values().cloned().collect(),
            theme_tokens: self.theme_tokens.values().cloned().collect(),
            input_contributions: self.input_contributions.values().cloned().collect(),
            ui_state_scopes: self.ui_state_scopes.values().cloned().collect(),
            layout_overrides: self.layout_overrides.values().cloned().collect(),
        }
    }

    fn theme_resolver(&self) -> ThemeTokenResolver {
        let mut resolver = ThemeTokenResolver::new();
        for token in self.theme_tokens.values() {
            let Some(token_type) = ThemeTokenType::parse(&token.token_type) else {
                continue;
            };
            resolver.insert_package_token(PackageThemeToken {
                token: token.token.clone(),
                token_type,
                fallback: token.fallback.clone(),
                description: token.description.clone(),
            });
        }
        resolver
    }

    pub(crate) fn register_panel(
        &mut self,
        package: &ClayPackageManifest,
        declaration: &Value,
        registered_command_ids: &[String],
    ) -> Result<RegisteredPanelContribution, UiContributionDiagnostic> {
        let context = UiDiagnosticContext::from_package(package, None);
        validate_provenance(package, &context)?;
        let size = payload_size(declaration);
        if size > SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES {
            return Err(context.error(
                UiContributionRule::PayloadTooLarge,
                None,
                format!(
                    "panel contribution payload ({size} bytes) exceeds SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES ({SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_prohibited_authority(declaration, &context)?;
        let object = declaration.as_object().ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidComponent,
                None,
                "panel contribution declaration must be an object",
            )
        })?;
        let id = package_owned_string(object, "id", package, UiContributionRule::InvalidId)?;
        let context = UiDiagnosticContext::from_package(package, Some(id.clone()));
        if self.panels.contains_key(&id) {
            return Err(context.error(
                UiContributionRule::DuplicateId,
                Some(&id),
                "panel contribution IDs must be unique among registered package UI panels",
            ));
        }
        let kind = optional_str(object, "kind").unwrap_or("fixed");
        if kind != "fixed" {
            return Err(context.error(
                UiContributionRule::InvalidPolicy,
                Some(&id),
                "Phase 18.3 panel contributions support kind `fixed`; transient UI must use serverRegisterTransientOverlayContribution",
            ));
        }
        let slot = required_str(object, "slot", UiContributionRule::InvalidSlot, &context)?;
        if !VALID_SLOTS.contains(&slot) {
            return Err(context.error(
                UiContributionRule::InvalidSlot,
                Some(&id),
                "panel slot must be one of left, right, top, or bottom",
            ));
        }
        let default_visibility = optional_str(object, "defaultVisibility").unwrap_or("hidden");
        if !VALID_VISIBILITY.contains(&default_visibility) {
            return Err(context.error(
                UiContributionRule::InvalidPolicy,
                Some(&id),
                "panel defaultVisibility must be visible, hidden, or collapsed",
            ));
        }
        let theme_resolver = self.theme_resolver();
        let mut component_context =
            ComponentValidationContext::new(package, registered_command_ids, &theme_resolver);
        let component = required_object(
            object,
            "component",
            UiContributionRule::InvalidComponent,
            &context,
        )?;
        let component_value = object.get("component").expect("required component exists");
        let component_id = component_context.validate_component_object(component)?;
        let component_tree =
            PackageUiComponentTree::from_declaration(component_value).map_err(|message| {
                context.error(UiContributionRule::InvalidComponent, Some(&id), message)
            })?;
        let mut action_targets =
            string_array(object.get("actionTargets"), "actionTargets", &context)?;
        validate_registered_actions(&action_targets, registered_command_ids, &context)?;
        action_targets.extend(component_context.action_targets);
        action_targets.sort();
        action_targets.dedup();

        let registered = RegisteredPanelContribution {
            id: id.clone(),
            slot: slot.to_string(),
            default_visibility: default_visibility.to_string(),
            component_id,
            component_tree,
            action_targets,
            provenance: UiContributionProvenance::from(package),
            estimated_payload_bytes: size,
        };
        self.panels.insert(id, registered.clone());
        Ok(registered)
    }

    pub(crate) fn register_component(
        &mut self,
        package: &ClayPackageManifest,
        declaration: &Value,
        registered_command_ids: &[String],
    ) -> Result<RegisteredComponentContribution, UiContributionDiagnostic> {
        let context = UiDiagnosticContext::from_package(package, None);
        validate_provenance(package, &context)?;
        let size = payload_size(declaration);
        if size > SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES {
            return Err(context.error(
                UiContributionRule::PayloadTooLarge,
                None,
                format!(
                    "component contribution payload ({size} bytes) exceeds SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES ({SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_prohibited_authority(declaration, &context)?;
        let object = declaration.as_object().ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidComponent,
                None,
                "component contribution declaration must be an object",
            )
        })?;
        let theme_resolver = self.theme_resolver();
        let mut component_context =
            ComponentValidationContext::new(package, registered_command_ids, &theme_resolver);
        let id = component_context.validate_component_object(object)?;
        let context = UiDiagnosticContext::from_package(package, Some(id.clone()));
        if self.components.contains_key(&id) {
            return Err(context.error(
                UiContributionRule::DuplicateId,
                Some(&id),
                "component contribution IDs must be unique among registered package UI components",
            ));
        }
        let root_kind = required_str(
            object,
            "kind",
            UiContributionRule::InvalidComponent,
            &context,
        )?
        .to_string();
        let mut action_targets = component_context.action_targets;
        action_targets.sort();
        action_targets.dedup();
        let registered = RegisteredComponentContribution {
            id: id.clone(),
            root_kind,
            component_count: component_context.component_count,
            style_variable_count: component_context.style_variable_count,
            action_targets,
            provenance: UiContributionProvenance::from(package),
            estimated_payload_bytes: size,
        };
        self.components.insert(id, registered.clone());
        Ok(registered)
    }

    pub(crate) fn register_overlay(
        &mut self,
        package: &ClayPackageManifest,
        declaration: &Value,
        registered_command_ids: &[String],
    ) -> Result<RegisteredTransientOverlayContribution, UiContributionDiagnostic> {
        let context = UiDiagnosticContext::from_package(package, None);
        validate_provenance(package, &context)?;
        let size = payload_size(declaration);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(context.error(
                UiContributionRule::PayloadTooLarge,
                None,
                format!(
                    "transient overlay payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_prohibited_authority(declaration, &context)?;
        let object = declaration.as_object().ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidComponent,
                None,
                "transient overlay declaration must be an object",
            )
        })?;
        let id = package_owned_string(object, "id", package, UiContributionRule::InvalidId)?;
        let context = UiDiagnosticContext::from_package(package, Some(id.clone()));
        if self.overlays.contains_key(&id) {
            return Err(context.error(
                UiContributionRule::DuplicateId,
                Some(&id),
                "overlay contribution IDs must be unique among registered package UI overlays",
            ));
        }
        let anchor = optional_str(object, "anchor").unwrap_or("working-area");
        if !VALID_OVERLAY_ANCHORS.contains(&anchor) {
            return Err(context.error(
                UiContributionRule::InvalidPolicy,
                Some(&id),
                "overlay anchor must be one of working-area, active-pane, main, or pointer",
            ));
        }
        let focus_policy = optional_str(object, "focusPolicy").unwrap_or("restore");
        if !VALID_FOCUS_POLICIES.contains(&focus_policy) {
            return Err(context.error(
                UiContributionRule::InvalidPolicy,
                Some(&id),
                "overlay focusPolicy must be none, restore, or trap",
            ));
        }
        let dismissal_policy = optional_str(object, "dismissalPolicy").unwrap_or("escape");
        if !VALID_DISMISSAL_POLICIES.contains(&dismissal_policy) {
            return Err(context.error(
                UiContributionRule::InvalidPolicy,
                Some(&id),
                "overlay dismissalPolicy must be manual, escape, outside, or escape-or-outside",
            ));
        }
        let theme_resolver = self.theme_resolver();
        let mut component_context =
            ComponentValidationContext::new(package, registered_command_ids, &theme_resolver);
        let component = required_object(
            object,
            "component",
            UiContributionRule::InvalidComponent,
            &context,
        )?;
        let component_value = object.get("component").expect("required component exists");
        let component_id = component_context.validate_component_object(component)?;
        let component_tree =
            PackageUiComponentTree::from_declaration(component_value).map_err(|message| {
                context.error(UiContributionRule::InvalidComponent, Some(&id), message)
            })?;
        let mut action_targets =
            string_array(object.get("actionTargets"), "actionTargets", &context)?;
        validate_registered_actions(&action_targets, registered_command_ids, &context)?;
        action_targets.extend(component_context.action_targets);
        action_targets.sort();
        action_targets.dedup();
        let registered = RegisteredTransientOverlayContribution {
            id: id.clone(),
            anchor: anchor.to_string(),
            focus_policy: focus_policy.to_string(),
            dismissal_policy: dismissal_policy.to_string(),
            component_id,
            component_tree,
            action_targets,
            provenance: UiContributionProvenance::from(package),
            estimated_payload_bytes: size,
        };
        self.overlays.insert(id, registered.clone());
        Ok(registered)
    }

    pub(crate) fn register_input(
        &mut self,
        package: &ClayPackageManifest,
        declaration: &Value,
        registered_command_ids: &[String],
    ) -> Result<RegisteredPackageInputContribution, UiContributionDiagnostic> {
        let context = UiDiagnosticContext::from_package(package, None);
        validate_provenance(package, &context)?;
        let size = payload_size(declaration);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(context.error(
                UiContributionRule::PayloadTooLarge,
                None,
                format!(
                    "input contribution payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_prohibited_authority(declaration, &context)?;
        let object = declaration.as_object().ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidInputScope,
                None,
                "input contribution declaration must be an object",
            )
        })?;
        if object.contains_key("keys")
            || object.contains_key("keybindings")
            || object.contains_key("onKey")
        {
            return Err(context.error(
                UiContributionRule::ProhibitedAuthority,
                None,
                "package input contributions must not declare key routing; use behavior manifests and clay:keybindings",
            ));
        }
        let id = package_owned_string(object, "id", package, UiContributionRule::InvalidId)?;
        let context = UiDiagnosticContext::from_package(package, Some(id.clone()));
        if self.input_contributions.contains_key(&id) {
            return Err(context.error(
                UiContributionRule::DuplicateId,
                Some(&id),
                "input contribution IDs must be unique among registered package UI input declarations",
            ));
        }
        let scope = required_str(
            object,
            "scope",
            UiContributionRule::InvalidInputScope,
            &context,
        )?;
        if !VALID_INPUT_SCOPES.contains(&scope) {
            return Err(context.error(
                UiContributionRule::InvalidInputScope,
                Some(&id),
                "input scope must be component, panel, or overlay",
            ));
        }
        let component_id = package_owned_string(
            object,
            "componentId",
            package,
            UiContributionRule::InvalidComponent,
        )?;
        let pointer = optional_object(
            object,
            "pointer",
            UiContributionRule::InvalidPolicy,
            &context,
        )?;
        let pointer_click = pointer
            .and_then(|pointer| optional_str(pointer, "click"))
            .unwrap_or("none");
        if !VALID_POINTER_CLICK_POLICIES.contains(&pointer_click) {
            return Err(context.error(
                UiContributionRule::InvalidPolicy,
                Some(&id),
                "pointer.click must be none, focus, action, or select",
            ));
        }
        let pointer_drag = pointer
            .and_then(|pointer| optional_str(pointer, "drag"))
            .unwrap_or("none");
        if !VALID_POINTER_DRAG_POLICIES.contains(&pointer_drag) {
            return Err(context.error(
                UiContributionRule::InvalidPolicy,
                Some(&id),
                "pointer.drag must be none, select, or pan",
            ));
        }
        let pointer_action = pointer
            .and_then(|pointer| optional_str(pointer, "action"))
            .map(ToOwned::to_owned);
        if pointer_click == "action" && pointer_action.is_none() {
            return Err(context.error(
                UiContributionRule::InvalidActionTarget,
                Some(&id),
                "pointer.click=action requires a registered pointer.action command ID",
            ));
        }
        if let Some(action) = &pointer_action {
            validate_registered_actions(
                std::slice::from_ref(action),
                registered_command_ids,
                &context,
            )?;
        }
        let focus = optional_object(
            object,
            "focus",
            UiContributionRule::InvalidFocusPolicy,
            &context,
        )?;
        let focus_policy = focus
            .and_then(|focus| optional_str(focus, "policy"))
            .unwrap_or("restore-editor");
        if !VALID_COMPONENT_FOCUS_POLICIES.contains(&focus_policy) {
            return Err(context.error(
                UiContributionRule::InvalidFocusPolicy,
                Some(&id),
                "focus.policy must be none, restore-editor, focus-component, or trap",
            ));
        }
        let selection_policy = optional_str(object, "selectionPolicy").unwrap_or("preserve-editor");
        if !VALID_SELECTION_POLICIES.contains(&selection_policy) {
            return Err(context.error(
                UiContributionRule::InvalidPolicy,
                Some(&id),
                "selectionPolicy must be preserve-editor, component-local, or disabled",
            ));
        }
        let context_modes = match optional_object(
            object,
            "context",
            UiContributionRule::InvalidPolicy,
            &context,
        )? {
            Some(context_object) => {
                string_array(context_object.get("modes"), "context.modes", &context)?
            }
            None => Vec::new(),
        };
        for mode in &context_modes {
            if !package.clay.modes.iter().any(|declared| declared == mode) {
                return Err(context.error(
                    UiContributionRule::InvalidPolicy,
                    Some(mode),
                    "input context modes must be declared by the package manifest",
                ));
            }
        }
        let mut action_targets =
            string_array(object.get("actionTargets"), "actionTargets", &context)?;
        if let Some(action) = &pointer_action {
            action_targets.push(action.clone());
        }
        validate_registered_actions(&action_targets, registered_command_ids, &context)?;
        action_targets.sort();
        action_targets.dedup();

        let registered = RegisteredPackageInputContribution {
            id: id.clone(),
            scope: scope.to_string(),
            component_id,
            pointer_click: pointer_click.to_string(),
            pointer_action,
            pointer_drag: pointer_drag.to_string(),
            focus_policy: focus_policy.to_string(),
            selection_policy: selection_policy.to_string(),
            context_modes,
            action_targets,
            provenance: UiContributionProvenance::from(package),
            estimated_payload_bytes: size,
        };
        self.input_contributions.insert(id, registered.clone());
        Ok(registered)
    }

    pub(crate) fn register_ui_state_scope(
        &mut self,
        package: &ClayPackageManifest,
        declaration: &Value,
    ) -> Result<RegisteredPackageUiStateScope, UiContributionDiagnostic> {
        let context = UiDiagnosticContext::from_package(package, None);
        validate_provenance(package, &context)?;
        let size = payload_size(declaration);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(context.error(
                UiContributionRule::PayloadTooLarge,
                None,
                format!(
                    "UI state scope declaration payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_prohibited_authority(declaration, &context)?;
        let object = declaration.as_object().ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidStateScope,
                None,
                "UI state scope declaration must be an object",
            )
        })?;
        let id = package_owned_string(object, "id", package, UiContributionRule::InvalidId)?;
        let context = UiDiagnosticContext::from_package(package, Some(id.clone()));
        if self.ui_state_scopes.contains_key(&id) {
            return Err(context.error(
                UiContributionRule::DuplicateId,
                Some(&id),
                "UI state scope IDs must be unique among registered package UI state declarations",
            ));
        }
        if id
            .split('.')
            .any(|segment| segment.starts_with('_') || segment.is_empty())
        {
            return Err(context.error(
                UiContributionRule::InvalidId,
                Some(&id),
                "UI state scope IDs must not use hidden or empty path segments",
            ));
        }
        let scope = required_str(
            object,
            "scope",
            UiContributionRule::InvalidStateScope,
            &context,
        )?;
        if !VALID_UI_STATE_SCOPES.contains(&scope) {
            return Err(context.error(
                UiContributionRule::InvalidStateScope,
                Some(&id),
                "UI state scope must be package-global, user-config, workspace, document, pane, component, or transient-overlay",
            ));
        }
        let owner = required_str(
            object,
            "owner",
            UiContributionRule::InvalidLifecycle,
            &context,
        )?;
        if !VALID_UI_STATE_OWNERS.contains(&owner) {
            return Err(context.error(
                UiContributionRule::InvalidLifecycle,
                Some(&id),
                "UI state owner must be package, shell, or server",
            ));
        }
        let lifetime = required_str(
            object,
            "lifetime",
            UiContributionRule::InvalidLifecycle,
            &context,
        )?;
        if !VALID_UI_STATE_LIFETIMES.contains(&lifetime) {
            return Err(context.error(
                UiContributionRule::InvalidLifecycle,
                Some(&id),
                "UI state lifetime must be session, workspace, document, or transient",
            ));
        }
        let persistence = required_str(
            object,
            "persistence",
            UiContributionRule::InvalidLifecycle,
            &context,
        )?;
        if !VALID_UI_STATE_PERSISTENCE.contains(&persistence) {
            return Err(context.error(
                UiContributionRule::InvalidLifecycle,
                Some(&id),
                "UI state persistence must be none, client-local, server-canonical, or deferred",
            ));
        }
        let implementation_status =
            optional_str(object, "implementationStatus").unwrap_or("deferred");
        if !VALID_UI_STATE_STATUSES.contains(&implementation_status) {
            return Err(context.error(
                UiContributionRule::InvalidLifecycle,
                Some(&id),
                "implementationStatus must be implemented or deferred",
            ));
        }
        let target_id = optional_str(object, "targetId").map(ToOwned::to_owned);
        if matches!(scope, "pane" | "component" | "transient-overlay") && target_id.is_none() {
            return Err(context.error(
                UiContributionRule::InvalidStateScope,
                Some(&id),
                "pane, component, and transient-overlay state scopes require a package-prefixed targetId",
            ));
        }
        if let Some(target_id) = &target_id {
            if !target_id.starts_with(&format!("{}.", package.clay.api_prefix)) {
                return Err(context.error(
                    UiContributionRule::InvalidId,
                    Some(target_id),
                    "state scope targetId must use the package apiPrefix",
                ));
            }
        }
        if implementation_status == "implemented"
            && matches!(scope, "workspace" | "document" | "user-config")
            && persistence != "client-local"
        {
            return Err(context.error(
                UiContributionRule::InvalidLifecycle,
                Some(&id),
                "workspace, document, and user-config UI state persistence remains deferred unless explicitly declared client-local",
            ));
        }
        let value_schema = object.get("valueSchema").ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidStateSchema,
                Some(&id),
                "UI state scopes require a bounded valueSchema object",
            )
        })?;
        reject_prohibited_authority(value_schema, &context)?;
        let schema_object = value_schema.as_object().ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidStateSchema,
                Some(&id),
                "valueSchema must be an object",
            )
        })?;
        if schema_object.contains_key("defaultValue")
            || schema_object.contains_key("initialValue")
            || schema_object.contains_key("rawValue")
        {
            return Err(context.error(
                UiContributionRule::ProhibitedAuthority,
                Some(&id),
                "UI state scope declarations define schemas only; state values are not accepted during registration",
            ));
        }
        let value_schema_kind = required_str(
            schema_object,
            "kind",
            UiContributionRule::InvalidStateSchema,
            &context,
        )?;
        if !VALID_UI_STATE_SCHEMA_KINDS.contains(&value_schema_kind) {
            return Err(context.error(
                UiContributionRule::InvalidStateSchema,
                Some(&id),
                "valueSchema.kind must be boolean, number, string, enum, or object",
            ));
        }
        if value_schema_kind == "enum" {
            let values = string_array(schema_object.get("values"), "valueSchema.values", &context)?;
            if values.is_empty() || values.len() > 32 {
                return Err(context.error(
                    UiContributionRule::InvalidStateSchema,
                    Some(&id),
                    "enum valueSchema.values must include 1 to 32 string values",
                ));
            }
        }

        let registered = RegisteredPackageUiStateScope {
            id: id.clone(),
            scope: scope.to_string(),
            owner: owner.to_string(),
            lifetime: lifetime.to_string(),
            persistence: persistence.to_string(),
            implementation_status: implementation_status.to_string(),
            value_schema_kind: value_schema_kind.to_string(),
            value_schema: value_schema.clone(),
            target_id,
            provenance: UiContributionProvenance::from(package),
            estimated_payload_bytes: size,
        };
        self.ui_state_scopes.insert(id, registered.clone());
        Ok(registered)
    }

    pub(crate) fn set_layout_override(
        &mut self,
        declaration: &Value,
    ) -> Result<RegisteredPackageLayoutOverride, UiContributionDiagnostic> {
        let context = UiDiagnosticContext::configuration(None);
        let size = payload_size(declaration);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(context.error(
                UiContributionRule::PayloadTooLarge,
                None,
                format!(
                    "layout override payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_prohibited_authority(declaration, &context)?;
        let object = declaration.as_object().ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidLayoutOverride,
                None,
                "layout override declaration must be an object",
            )
        })?;
        let target_id = required_str(
            object,
            "targetId",
            UiContributionRule::InvalidLayoutOverride,
            &context,
        )?;
        validate_prefixed_public_id(target_id, "targetId", &context)?;
        let property = required_str(
            object,
            "property",
            UiContributionRule::InvalidLayoutOverride,
            &context,
        )?;
        if !VALID_LAYOUT_OVERRIDE_PROPERTIES.contains(&property) {
            return Err(context.error(
                UiContributionRule::InvalidLayoutOverride,
                Some(property),
                "layout override property must be slot, visibility, splitRatio, themeToken, inputDefault, actionDefault, or fallback",
            ));
        }
        let source = optional_str(object, "source").unwrap_or("user-config");
        if !VALID_LAYOUT_OVERRIDE_SOURCES.contains(&source) {
            return Err(context.error(
                UiContributionRule::InvalidLayoutOverride,
                Some(source),
                "layout override source must be user-config, active-major-mode, compatible-minor-mode, global-package, or package-default",
            ));
        }
        let value = object.get("value").ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidLayoutOverride,
                Some(target_id),
                "layout override requires a typed value",
            )
        })?;
        reject_prohibited_authority(value, &context)?;
        validate_layout_override_value(property, target_id, value, self, &context)?;
        let id = format!("{source}:{target_id}:{property}");
        let registered = RegisteredPackageLayoutOverride {
            id: id.clone(),
            target_id: target_id.to_string(),
            property: property.to_string(),
            value: value.clone(),
            source: source.to_string(),
            precedence_rank: layout_precedence_rank(source),
            estimated_payload_bytes: size,
        };
        self.layout_overrides.insert(id, registered.clone());
        Ok(registered)
    }

    pub(crate) fn register_theme_token(
        &mut self,
        package: &ClayPackageManifest,
        declaration: &Value,
    ) -> Result<RegisteredPackageThemeTokenDeclaration, UiContributionDiagnostic> {
        let context = UiDiagnosticContext::from_package(package, None);
        validate_provenance(package, &context)?;
        let size = payload_size(declaration);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(context.error(
                UiContributionRule::PayloadTooLarge,
                None,
                "theme token declaration exceeds bounded package UI update payload budget",
            ));
        }
        reject_prohibited_authority(declaration, &context)?;
        let object = declaration.as_object().ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidThemeToken,
                None,
                "theme token declaration must be an object",
            )
        })?;
        if object.contains_key("value")
            || object.contains_key("rawColor")
            || object.contains_key("css")
        {
            return Err(context.error(
                UiContributionRule::ProhibitedAuthority,
                None,
                "theme token declarations must provide typed fallback contracts, not raw CSS, raw colors, or direct values",
            ));
        }
        let token = package_owned_string(
            object,
            "token",
            package,
            UiContributionRule::InvalidThemeToken,
        )?;
        let context = UiDiagnosticContext::from_package(package, Some(token.clone()));
        if self.theme_tokens.contains_key(&token) {
            return Err(context.error(
                UiContributionRule::DuplicateId,
                Some(&token),
                "theme token IDs must be unique among registered package UI tokens",
            ));
        }
        let token_type_text = required_str(
            object,
            "type",
            UiContributionRule::InvalidThemeToken,
            &context,
        )?;
        let Some(token_type) = ThemeTokenType::parse(token_type_text) else {
            return Err(context.error(
                UiContributionRule::InvalidThemeToken,
                Some(&token),
                "theme token type must be color-role, spacing, radius, typography, or opacity",
            ));
        };
        let fallback = required_str(
            object,
            "fallback",
            UiContributionRule::InvalidThemeToken,
            &context,
        )?;
        if !core_fallback_matches_type(fallback, token_type) {
            return Err(context.error(
                UiContributionRule::InvalidThemeToken,
                Some(&token),
                "theme token fallback must reference a known Clay core token with the same type",
            ));
        }
        let description = required_str(
            object,
            "description",
            UiContributionRule::InvalidThemeToken,
            &context,
        )?;
        let registered = RegisteredPackageThemeTokenDeclaration {
            token: token.clone(),
            token_type: token_type.as_str().to_string(),
            fallback: fallback.to_string(),
            description: description.to_string(),
            resolved_core_token: fallback.to_string(),
            provenance: UiContributionProvenance::from(package),
            estimated_payload_bytes: size,
        };
        self.theme_tokens.insert(token, registered.clone());
        Ok(registered)
    }
}

impl From<&ClayPackageManifest> for UiContributionProvenance {
    fn from(package: &ClayPackageManifest) -> Self {
        Self {
            package_name: package.name.clone(),
            package_version: package.version.clone(),
            api_prefix: package.clay.api_prefix.clone(),
        }
    }
}

struct ComponentValidationContext<'a> {
    package: &'a ClayPackageManifest,
    registered_command_ids: &'a [String],
    theme_resolver: &'a ThemeTokenResolver,
    seen_ids: BTreeSet<String>,
    action_targets: Vec<String>,
    component_count: usize,
    style_variable_count: usize,
}

impl<'a> ComponentValidationContext<'a> {
    fn new(
        package: &'a ClayPackageManifest,
        registered_command_ids: &'a [String],
        theme_resolver: &'a ThemeTokenResolver,
    ) -> Self {
        Self {
            package,
            registered_command_ids,
            theme_resolver,
            seen_ids: BTreeSet::new(),
            action_targets: Vec::new(),
            component_count: 0,
            style_variable_count: 0,
        }
    }

    fn validate_component_object(
        &mut self,
        object: &Map<String, Value>,
    ) -> Result<String, UiContributionDiagnostic> {
        self.component_count += 1;
        let context = UiDiagnosticContext::from_package(self.package, None);
        if self.component_count > MAX_COMPONENT_NODES {
            return Err(context.error(
                UiContributionRule::PayloadTooLarge,
                None,
                format!("component tree exceeds {MAX_COMPONENT_NODES} nodes"),
            ));
        }
        let kind = required_str(
            object,
            "kind",
            UiContributionRule::InvalidComponent,
            &context,
        )?;
        validate_component_kind(kind).map_err(|error| {
            context.error(
                UiContributionRule::InvalidComponent,
                Some(&error.field),
                error.message,
            )
        })?;
        let id = package_owned_string(object, "id", self.package, UiContributionRule::InvalidId)?;
        let context = UiDiagnosticContext::from_package(self.package, Some(id.clone()));
        if !self.seen_ids.insert(id.clone()) {
            return Err(context.error(
                UiContributionRule::DuplicateId,
                Some(&id),
                "component IDs must be unique within a contribution tree",
            ));
        }
        if object.contains_key("styleString") || object.contains_key("className") {
            return Err(context.error(
                UiContributionRule::ProhibitedAuthority,
                Some(&id),
                "component declarations must not include raw CSS/style strings or class names; use typed style variables",
            ));
        }
        let style_variables =
            validate_style_variables(object, self.theme_resolver).map_err(|error| {
                let rule = if error.field == "style" || error.message.contains("raw CSS") {
                    UiContributionRule::ProhibitedAuthority
                } else {
                    UiContributionRule::InvalidThemeToken
                };
                context.error(rule, Some(&error.field), error.message)
            })?;
        self.style_variable_count += style_variables.len();
        if let Some(action) = object.get("action").and_then(Value::as_object) {
            let command_id = required_str(
                action,
                "commandId",
                UiContributionRule::InvalidActionTarget,
                &context,
            )?;
            validate_registered_actions(
                &[command_id.to_string()],
                self.registered_command_ids,
                &context,
            )?;
            self.action_targets.push(command_id.to_string());
        }
        if let Some(items) = object.get("items") {
            let items = items.as_array().ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidComponent,
                    Some(&id),
                    "component items must be an array",
                )
            })?;
            for item in items {
                let item_object = item.as_object().ok_or_else(|| {
                    context.error(
                        UiContributionRule::InvalidComponent,
                        Some(&id),
                        "component list items must be objects",
                    )
                })?;
                if let Some(action) = item_object.get("action").and_then(Value::as_object) {
                    let command_id = required_str(
                        action,
                        "commandId",
                        UiContributionRule::InvalidActionTarget,
                        &context,
                    )?;
                    validate_registered_actions(
                        &[command_id.to_string()],
                        self.registered_command_ids,
                        &context,
                    )?;
                    self.action_targets.push(command_id.to_string());
                }
            }
        }
        if let Some(children) = object.get("children") {
            let children = children.as_array().ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidComponent,
                    Some(&id),
                    "component children must be an array",
                )
            })?;
            for child in children {
                let child_object = child.as_object().ok_or_else(|| {
                    context.error(
                        UiContributionRule::InvalidComponent,
                        Some(&id),
                        "component children must be objects",
                    )
                })?;
                self.validate_component_object(child_object)?;
            }
        }
        Ok(id)
    }
}

#[derive(Clone)]
struct UiDiagnosticContext {
    package_name: Option<String>,
    package_version: Option<String>,
    api_prefix: Option<String>,
    contribution_id: Option<String>,
}

impl UiDiagnosticContext {
    fn from_package(package: &ClayPackageManifest, contribution_id: Option<String>) -> Self {
        Self {
            package_name: Some(package.name.clone()),
            package_version: Some(package.version.clone()),
            api_prefix: Some(package.clay.api_prefix.clone()),
            contribution_id,
        }
    }

    fn configuration(contribution_id: Option<String>) -> Self {
        Self {
            package_name: None,
            package_version: None,
            api_prefix: None,
            contribution_id,
        }
    }

    fn error(
        &self,
        rule: UiContributionRule,
        contribution_id: Option<&str>,
        message: impl Into<String>,
    ) -> UiContributionDiagnostic {
        UiContributionDiagnostic {
            package_name: self.package_name.clone(),
            package_version: self.package_version.clone(),
            api_prefix: self.api_prefix.clone(),
            contribution_id: contribution_id
                .map(ToOwned::to_owned)
                .or_else(|| self.contribution_id.clone()),
            rule,
            message: message.into(),
        }
    }
}

fn validate_provenance(
    package: &ClayPackageManifest,
    context: &UiDiagnosticContext,
) -> Result<(), UiContributionDiagnostic> {
    if package.name.trim().is_empty()
        || package.version.trim().is_empty()
        || !is_valid_api_prefix(&package.clay.api_prefix)
    {
        return Err(context.error(
            UiContributionRule::InvalidProvenance,
            None,
            "package UI contribution provenance must come from a validated package manifest",
        ));
    }
    Ok(())
}

fn package_owned_string(
    object: &Map<String, Value>,
    key: &str,
    package: &ClayPackageManifest,
    rule: UiContributionRule,
) -> Result<String, UiContributionDiagnostic> {
    let context = UiDiagnosticContext::from_package(package, None);
    let value = required_str(object, key, rule.clone(), &context)?;
    if value.starts_with("clay.") || !is_package_owned_id(value, &package.clay.api_prefix) {
        return Err(context.error(
            rule,
            Some(value),
            format!("{key} must use the package apiPrefix or apiPrefix.* namespace"),
        ));
    }
    Ok(value.to_string())
}

fn is_package_owned_id(value: &str, api_prefix: &str) -> bool {
    value == api_prefix
        || value
            .strip_prefix(api_prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

fn validate_prefixed_public_id(
    value: &str,
    key: &str,
    context: &UiDiagnosticContext,
) -> Result<(), UiContributionDiagnostic> {
    if value.starts_with("clay.")
        || !value.contains('.')
        || value
            .split('.')
            .any(|segment| segment.is_empty() || segment.starts_with('_'))
    {
        return Err(context.error(
            UiContributionRule::InvalidId,
            Some(value),
            format!("{key} must be package-prefixed and must not use hidden or empty segments"),
        ));
    }
    Ok(())
}

fn validate_registered_actions(
    action_targets: &[String],
    registered_command_ids: &[String],
    context: &UiDiagnosticContext,
) -> Result<(), UiContributionDiagnostic> {
    for command_id in action_targets {
        if !registered_command_ids
            .iter()
            .any(|registered| registered == command_id)
        {
            return Err(context.error(
                UiContributionRule::InvalidActionTarget,
                Some(command_id),
                format!("action target `{command_id}` must be registered with clay:commands before package UI registration"),
            ));
        }
    }
    Ok(())
}

fn validate_layout_override_value(
    property: &str,
    target_id: &str,
    value: &Value,
    registry: &PackageUiRegistry,
    context: &UiDiagnosticContext,
) -> Result<(), UiContributionDiagnostic> {
    match property {
        "slot" => {
            let slot = value.as_str().ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidLayoutOverride,
                    Some(target_id),
                    "slot override value must be a string",
                )
            })?;
            if !VALID_SLOTS.contains(&slot) {
                return Err(context.error(
                    UiContributionRule::InvalidSlot,
                    Some(slot),
                    "slot override value must be left, right, top, or bottom",
                ));
            }
        }
        "visibility" => {
            let visibility = value.as_str().ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidLayoutOverride,
                    Some(target_id),
                    "visibility override value must be a string",
                )
            })?;
            if !VALID_VISIBILITY.contains(&visibility) {
                return Err(context.error(
                    UiContributionRule::InvalidPolicy,
                    Some(visibility),
                    "visibility override value must be visible, hidden, or collapsed",
                ));
            }
        }
        "splitRatio" => {
            let ratio = value.as_f64().ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidLayoutOverride,
                    Some(target_id),
                    "splitRatio override value must be a number",
                )
            })?;
            if !(0.1..=0.9).contains(&ratio) {
                return Err(context.error(
                    UiContributionRule::InvalidLayoutOverride,
                    Some(target_id),
                    "splitRatio override value must be between 0.1 and 0.9",
                ));
            }
        }
        "themeToken" => {
            let object = value.as_object().ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidThemeToken,
                    Some(target_id),
                    "themeToken override value must be { token, fallback }",
                )
            })?;
            let token = required_str(
                object,
                "token",
                UiContributionRule::InvalidThemeToken,
                context,
            )?;
            validate_prefixed_public_id(token, "theme token", context)?;
            let fallback = required_str(
                object,
                "fallback",
                UiContributionRule::InvalidThemeToken,
                context,
            )?;
            let declared = registry.theme_tokens.get(token).ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidThemeToken,
                    Some(token),
                    "themeToken override token must be registered before it can be remapped",
                )
            })?;
            let token_type = ThemeTokenType::parse(&declared.token_type).ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidThemeToken,
                    Some(token),
                    "registered theme token has an invalid type",
                )
            })?;
            if !core_fallback_matches_type(fallback, token_type) {
                return Err(context.error(
                    UiContributionRule::InvalidThemeToken,
                    Some(fallback),
                    "themeToken fallback must reference a known Clay core token with the same type",
                ));
            }
        }
        "inputDefault" => {
            let object = value.as_object().ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidLayoutOverride,
                    Some(target_id),
                    "inputDefault override value must be an object",
                )
            })?;
            let input_id = required_str(
                object,
                "inputId",
                UiContributionRule::InvalidInputScope,
                context,
            )?;
            if !registry.input_contributions.contains_key(input_id) {
                return Err(context.error(
                    UiContributionRule::InvalidInputScope,
                    Some(input_id),
                    "inputDefault.inputId must reference a registered package input contribution",
                ));
            }
        }
        "actionDefault" => {
            let action_id = value.as_str().ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidActionTarget,
                    Some(target_id),
                    "actionDefault override value must be a registered action ID string",
                )
            })?;
            let known_action = registry.panels.values().any(|panel| {
                panel
                    .action_targets
                    .iter()
                    .any(|action| action == action_id)
            }) || registry.components.values().any(|component| {
                component
                    .action_targets
                    .iter()
                    .any(|action| action == action_id)
            }) || registry.overlays.values().any(|overlay| {
                overlay
                    .action_targets
                    .iter()
                    .any(|action| action == action_id)
            }) || registry.input_contributions.values().any(|input| {
                input
                    .action_targets
                    .iter()
                    .any(|action| action == action_id)
            });
            if !known_action {
                return Err(context.error(
                    UiContributionRule::InvalidActionTarget,
                    Some(action_id),
                    "actionDefault must reference an action target already declared by package UI/input contributions",
                ));
            }
        }
        "fallback" => {
            let fallback = value.as_str().ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidLayoutOverride,
                    Some(target_id),
                    "fallback override value must be a string",
                )
            })?;
            if !VALID_FALLBACK_BEHAVIORS.contains(&fallback) {
                return Err(context.error(
                    UiContributionRule::InvalidLayoutOverride,
                    Some(fallback),
                    "fallback override value must be package-default, hide, or ignore",
                ));
            }
        }
        _ => unreachable!("layout override property validated before value validation"),
    }
    Ok(())
}

fn layout_precedence_rank(source: &str) -> u8 {
    match source {
        "user-config" => 1,
        "active-major-mode" => 2,
        "compatible-minor-mode" => 3,
        "global-package" => 4,
        _ => 5,
    }
}

fn required_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    rule: UiContributionRule,
    context: &UiDiagnosticContext,
) -> Result<&'a Map<String, Value>, UiContributionDiagnostic> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| context.error(rule, None, format!("{key} must be an object")))
}

fn optional_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    rule: UiContributionRule,
    context: &UiDiagnosticContext,
) -> Result<Option<&'a Map<String, Value>>, UiContributionDiagnostic> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(value)) => Ok(Some(value)),
        Some(_) => Err(context.error(rule, None, format!("{key} must be an object"))),
    }
}

fn required_str<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    rule: UiContributionRule,
    context: &UiDiagnosticContext,
) -> Result<&'a str, UiContributionDiagnostic> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| context.error(rule, None, format!("{key} must be a non-empty string")))
}

fn optional_str<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn string_array(
    value: Option<&Value>,
    key: &str,
    context: &UiDiagnosticContext,
) -> Result<Vec<String>, UiContributionDiagnostic> {
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
                        context.error(
                            UiContributionRule::InvalidActionTarget,
                            None,
                            format!("{key} entries must be non-empty strings"),
                        )
                    })
            })
            .collect(),
        _ => Err(context.error(
            UiContributionRule::InvalidActionTarget,
            None,
            format!("{key} must be an array"),
        )),
    }
}

fn reject_prohibited_authority(
    value: &Value,
    context: &UiDiagnosticContext,
) -> Result<(), UiContributionDiagnostic> {
    match value {
        Value::String(text) if text.contains("Deno.core.ops") || text.contains("op_clay_") => {
            Err(context.error(
                UiContributionRule::ProhibitedAuthority,
                None,
                "package UI declarations must not expose raw Deno.core.ops or op names",
            ))
        }
        Value::Object(object) => {
            for (key, nested) in object {
                if matches!(
                    key.as_str(),
                    "rawOps"
                        | "nativeHandle"
                        | "nativeWidget"
                        | "masonryWidget"
                        | "widgetCallback"
                        | "rendererCallback"
                        | "drawCallback"
                        | "clientHook"
                        | "clientJavaScript"
                        | "javascript"
                        | "code"
                        | "rawCss"
                        | "cssText"
                ) {
                    return Err(context.error(
                        UiContributionRule::ProhibitedAuthority,
                        Some(key),
                        "package UI declarations must not include raw ops, native widgets, raw CSS, renderer callbacks, or client-side JavaScript hooks",
                    ));
                }
                reject_prohibited_authority(nested, context)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_prohibited_authority(nested, context)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn payload_size(value: &Value) -> usize {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX)
}

fn fixed_slot_id(slot: &str) -> FixedSlotId {
    match slot {
        "right" => FixedSlotId::Right,
        "top" => FixedSlotId::Top,
        "bottom" => FixedSlotId::Bottom,
        _ => FixedSlotId::Left,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::packages::{manifest::validate_manifest_value, permissions::PackagePermission};

    fn package() -> ClayPackageManifest {
        validate_manifest_value(&json!({
            "name": "@clay/markdown",
            "version": "0.1.0",
            "clay": {
                "apiPrefix": "markdown",
                "entry": "./dist/index.js",
                "permissions": ["command-registration"],
                "modes": ["markdown"]
            }
        }))
        .unwrap()
    }

    #[test]
    fn ui_registry_accepts_valid_panel_component_overlay_and_theme_token() {
        let mut registry = PackageUiRegistry::new();
        let package = package();
        let commands = vec!["markdown.togglePreview".to_string()];

        let panel = registry
            .register_panel(
                &package,
                &json!({
                    "id": "markdown.preview",
                    "slot": "right",
                    "kind": "fixed",
                    "defaultVisibility": "hidden",
                    "actionTargets": ["markdown.togglePreview"],
                    "component": {
                        "kind": "panel",
                        "id": "markdown.preview.root",
                        "title": "Preview",
                        "children": [{
                            "kind": "button",
                            "id": "markdown.preview.toggle",
                            "label": "Toggle",
                            "action": { "commandId": "markdown.togglePreview" }
                        }]
                    }
                }),
                &commands,
            )
            .unwrap();
        assert_eq!(panel.slot, "right");
        assert_eq!(panel.provenance.api_prefix, "markdown");

        let component = registry
            .register_component(
                &package,
                &json!({
                    "kind": "label",
                    "id": "markdown.preview.empty",
                    "text": "Preview unavailable"
                }),
                &commands,
            )
            .unwrap();
        assert_eq!(component.root_kind, "label");

        let overlay = registry
            .register_overlay(
                &package,
                &json!({
                    "id": "markdown.preview.quickOpen",
                    "anchor": "working-area",
                    "focusPolicy": "restore",
                    "dismissalPolicy": "escape",
                    "component": {
                        "kind": "panel",
                        "id": "markdown.preview.quickOpen.root",
                        "title": "Quick Open",
                        "children": []
                    }
                }),
                &commands,
            )
            .unwrap();
        assert_eq!(overlay.focus_policy, "restore");

        let token = registry
            .register_theme_token(
                &package,
                &json!({
                    "token": "markdown.preview.background",
                    "type": "color-role",
                    "fallback": "surface.panel",
                    "description": "Markdown preview background"
                }),
            )
            .unwrap();
        assert_eq!(token.token_type, "color-role");
        assert_eq!(token.resolved_core_token, "surface.panel");

        let styled_component = registry
            .register_component(
                &package,
                &json!({
                    "kind": "panel",
                    "id": "markdown.preview.styled",
                    "style": {
                        "background": "markdown.preview.background",
                        "padding": "spacing.panel",
                        "typography": "typography.body"
                    },
                    "children": []
                }),
                &commands,
            )
            .unwrap();
        assert_eq!(styled_component.style_variable_count, 3);
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.panels.len(), 1);
        let mut runtime = crate::shell::PackageUiRuntimeState::new();
        runtime
            .apply_update(snapshot.runtime_update(0))
            .expect("registered package UI contributions should compose into runtime state");
        assert!(runtime.has_fixed_panels());
        assert!(runtime.fixed_panel_for_slot(FixedSlotId::Right).is_some());
        assert_eq!(runtime.transient_overlay_count(), 1);
        assert!(
            package
                .clay
                .permissions
                .contains(&PackagePermission::CommandRegistration)
        );
    }

    #[test]
    fn ui_registry_rejects_invalid_prefix_unregistered_actions_raw_css_and_duplicate_ids() {
        let mut registry = PackageUiRegistry::new();
        let package = package();
        let commands = vec!["markdown.togglePreview".to_string()];

        let invalid_prefix = registry
            .register_component(
                &package,
                &json!({ "kind": "label", "id": "other.preview", "text": "bad" }),
                &commands,
            )
            .unwrap_err();
        assert_eq!(invalid_prefix.rule, UiContributionRule::InvalidId);

        let invalid_action = registry
            .register_panel(
                &package,
                &json!({
                    "id": "markdown.preview",
                    "slot": "right",
                    "component": { "kind": "button", "id": "markdown.preview.button", "label": "Run", "action": { "commandId": "markdown.missing" } }
                }),
                &commands,
            )
            .unwrap_err();
        assert_eq!(invalid_action.rule, UiContributionRule::InvalidActionTarget);

        let raw_css = registry
            .register_component(
                &package,
                &json!({ "kind": "label", "id": "markdown.preview.raw", "text": "bad", "style": "color: red" }),
                &commands,
            )
            .unwrap_err();
        assert_eq!(raw_css.rule, UiContributionRule::ProhibitedAuthority);

        registry
            .register_theme_token(
                &package,
                &json!({
                    "token": "markdown.preview.background",
                    "type": "color-role",
                    "fallback": "surface.panel",
                    "description": "Markdown preview background"
                }),
            )
            .unwrap();
        let duplicate = registry
            .register_theme_token(
                &package,
                &json!({
                    "token": "markdown.preview.background",
                    "type": "color-role",
                    "fallback": "surface.panel",
                    "description": "Duplicate"
                }),
            )
            .unwrap_err();
        assert_eq!(duplicate.rule, UiContributionRule::DuplicateId);
    }

    #[test]
    fn component_catalog_rejects_unknown_kinds_duplicate_ids_and_unregistered_actions() {
        let mut registry = PackageUiRegistry::new();
        let package = package();
        let commands = vec!["markdown.togglePreview".to_string()];

        let unknown_kind = registry
            .register_component(
                &package,
                &json!({ "kind": "table", "id": "markdown.preview.table" }),
                &commands,
            )
            .unwrap_err();
        assert_eq!(unknown_kind.rule, UiContributionRule::InvalidComponent);
        assert!(unknown_kind.message.contains("reserved for a later"));

        let duplicate_ids = registry
            .register_component(
                &package,
                &json!({
                    "kind": "panel",
                    "id": "markdown.preview.root",
                    "children": [
                        { "kind": "label", "id": "markdown.preview.duplicate", "text": "First" },
                        { "kind": "label", "id": "markdown.preview.duplicate", "text": "Second" }
                    ]
                }),
                &commands,
            )
            .unwrap_err();
        assert_eq!(duplicate_ids.rule, UiContributionRule::DuplicateId);

        let unregistered_action = registry
            .register_component(
                &package,
                &json!({
                    "kind": "button",
                    "id": "markdown.preview.run",
                    "label": "Run",
                    "action": { "commandId": "markdown.missing" }
                }),
                &commands,
            )
            .unwrap_err();
        assert_eq!(
            unregistered_action.rule,
            UiContributionRule::InvalidActionTarget
        );
    }

    #[test]
    fn input_contributions_accept_component_scoped_pointer_focus_and_actions() {
        let mut registry = PackageUiRegistry::new();
        let package = package();
        let commands = vec![
            "markdown.focusPreview".to_string(),
            "markdown.togglePreview".to_string(),
        ];

        let input = registry
            .register_input(
                &package,
                &json!({
                    "id": "markdown.preview.input",
                    "scope": "component",
                    "componentId": "markdown.preview.root",
                    "pointer": {
                        "click": "action",
                        "action": "markdown.focusPreview",
                        "drag": "select"
                    },
                    "focus": { "policy": "restore-editor" },
                    "selectionPolicy": "component-local",
                    "context": { "modes": ["markdown"] },
                    "actionTargets": ["markdown.togglePreview"]
                }),
                &commands,
            )
            .unwrap();

        assert_eq!(input.scope, "component");
        assert_eq!(
            input.pointer_action.as_deref(),
            Some("markdown.focusPreview")
        );
        assert_eq!(input.action_targets.len(), 2);
        let snapshot = registry.snapshot();
        let mut runtime = crate::shell::PackageUiRuntimeState::new();
        runtime
            .apply_update(snapshot.runtime_update(0))
            .expect("input routing should compose into inert runtime state");
        let routes: Vec<_> = runtime.input_routes().collect();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].focus_policy, "restore-editor");
    }

    #[test]
    fn input_contributions_reject_raw_callbacks_key_routing_and_unregistered_actions() {
        let mut registry = PackageUiRegistry::new();
        let package = package();
        let commands = vec!["markdown.focusPreview".to_string()];

        let raw_callback = registry
            .register_input(
                &package,
                &json!({
                    "id": "markdown.preview.input",
                    "scope": "component",
                    "componentId": "markdown.preview.root",
                    "pointer": { "click": "focus" },
                    "clientJavaScript": "window.alert(1)"
                }),
                &commands,
            )
            .unwrap_err();
        assert_eq!(raw_callback.rule, UiContributionRule::ProhibitedAuthority);

        let key_route = registry
            .register_input(
                &package,
                &json!({
                    "id": "markdown.preview.keys",
                    "scope": "component",
                    "componentId": "markdown.preview.root",
                    "keys": ["Enter"]
                }),
                &commands,
            )
            .unwrap_err();
        assert_eq!(key_route.rule, UiContributionRule::ProhibitedAuthority);

        let missing_action = registry
            .register_input(
                &package,
                &json!({
                    "id": "markdown.preview.missingAction",
                    "scope": "component",
                    "componentId": "markdown.preview.root",
                    "pointer": { "click": "action", "action": "markdown.missing" }
                }),
                &commands,
            )
            .unwrap_err();
        assert_eq!(missing_action.rule, UiContributionRule::InvalidActionTarget);
    }

    #[test]
    fn ui_state_scope_registration_accepts_supported_scopes_and_lifecycles() {
        let mut registry = PackageUiRegistry::new();
        let package = package();

        let scope = registry
            .register_ui_state_scope(
                &package,
                &json!({
                    "id": "markdown.preview.visibility",
                    "scope": "pane",
                    "targetId": "markdown.preview",
                    "owner": "shell",
                    "lifetime": "session",
                    "persistence": "client-local",
                    "implementationStatus": "implemented",
                    "valueSchema": { "kind": "enum", "values": ["visible", "hidden"] }
                }),
            )
            .unwrap();

        assert_eq!(scope.scope, "pane");
        assert_eq!(scope.persistence, "client-local");
        assert_eq!(scope.implementation_status, "implemented");
        assert_eq!(scope.value_schema_kind, "enum");
        assert_eq!(scope.provenance.api_prefix, "markdown");
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.ui_state_scopes.len(), 1);
        assert_eq!(
            snapshot.ui_state_scopes[0].target_id.as_deref(),
            Some("markdown.preview")
        );
    }

    #[test]
    fn ui_state_scope_registration_rejects_hidden_globals_unsupported_scopes_and_payloads() {
        let mut registry = PackageUiRegistry::new();
        let package = package();

        let hidden = registry
            .register_ui_state_scope(
                &package,
                &json!({
                    "id": "markdown._hidden",
                    "scope": "package-global",
                    "owner": "package",
                    "lifetime": "session",
                    "persistence": "none",
                    "valueSchema": { "kind": "boolean" }
                }),
            )
            .unwrap_err();
        assert_eq!(hidden.rule, UiContributionRule::InvalidId);

        let unsupported = registry
            .register_ui_state_scope(
                &package,
                &json!({
                    "id": "markdown.preview.unsupported",
                    "scope": "masonry-widget",
                    "owner": "shell",
                    "lifetime": "session",
                    "persistence": "client-local",
                    "valueSchema": { "kind": "boolean" }
                }),
            )
            .unwrap_err();
        assert_eq!(unsupported.rule, UiContributionRule::InvalidStateScope);

        let raw_value = registry
            .register_ui_state_scope(
                &package,
                &json!({
                    "id": "markdown.preview.raw",
                    "scope": "component",
                    "targetId": "markdown.preview.root",
                    "owner": "shell",
                    "lifetime": "session",
                    "persistence": "client-local",
                    "valueSchema": { "kind": "string", "defaultValue": "hidden" }
                }),
            )
            .unwrap_err();
        assert_eq!(raw_value.rule, UiContributionRule::ProhibitedAuthority);

        let raw_ops = registry
            .register_ui_state_scope(
                &package,
                &json!({
                    "id": "markdown.preview.ops",
                    "scope": "component",
                    "targetId": "markdown.preview.root",
                    "owner": "shell",
                    "lifetime": "session",
                    "persistence": "client-local",
                    "valueSchema": { "kind": "string", "code": "Deno.core.ops.op_clay_runtime_ping()" }
                }),
            )
            .unwrap_err();
        assert_eq!(raw_ops.rule, UiContributionRule::ProhibitedAuthority);
    }

    #[test]
    fn layout_override_applies_user_precedence_and_validates_theme_input_and_actions() {
        let mut registry = PackageUiRegistry::new();
        let package = package();
        let commands = vec!["markdown.togglePreview".to_string()];

        registry
            .register_panel(
                &package,
                &json!({
                    "id": "markdown.preview",
                    "slot": "right",
                    "component": {
                        "kind": "button",
                        "id": "markdown.preview.button",
                        "label": "Toggle",
                        "action": { "commandId": "markdown.togglePreview" }
                    }
                }),
                &commands,
            )
            .unwrap();
        registry
            .register_theme_token(
                &package,
                &json!({
                    "token": "markdown.preview.background",
                    "type": "color-role",
                    "fallback": "surface.panel",
                    "description": "Markdown preview background"
                }),
            )
            .unwrap();
        registry
            .register_input(
                &package,
                &json!({
                    "id": "markdown.preview.input",
                    "scope": "component",
                    "componentId": "markdown.preview.button",
                    "pointer": { "click": "action", "action": "markdown.togglePreview" }
                }),
                &commands,
            )
            .unwrap();

        let visibility = registry
            .set_layout_override(&json!({
                "targetId": "markdown.preview",
                "property": "visibility",
                "value": "hidden",
                "source": "user-config"
            }))
            .unwrap();
        assert_eq!(visibility.precedence_rank, 1);

        let token_remap = registry
            .set_layout_override(&json!({
                "targetId": "markdown.preview",
                "property": "themeToken",
                "value": { "token": "markdown.preview.background", "fallback": "surface.overlay" },
                "source": "user-config"
            }))
            .unwrap();
        assert_eq!(token_remap.property, "themeToken");

        let input_default = registry
            .set_layout_override(&json!({
                "targetId": "markdown.preview",
                "property": "inputDefault",
                "value": { "inputId": "markdown.preview.input" },
                "source": "active-major-mode"
            }))
            .unwrap();
        assert_eq!(input_default.precedence_rank, 2);

        let action_default = registry
            .set_layout_override(&json!({
                "targetId": "markdown.preview",
                "property": "actionDefault",
                "value": "markdown.togglePreview",
                "source": "package-default"
            }))
            .unwrap();
        assert_eq!(action_default.precedence_rank, 5);
        assert_eq!(registry.snapshot().layout_overrides.len(), 4);
    }

    #[test]
    fn layout_override_rejects_hidden_keys_unknown_tokens_raw_values_and_bad_slots() {
        let mut registry = PackageUiRegistry::new();

        let hidden_target = registry
            .set_layout_override(&json!({
                "targetId": "markdown._hidden",
                "property": "visibility",
                "value": "hidden"
            }))
            .unwrap_err();
        assert_eq!(hidden_target.rule, UiContributionRule::InvalidId);

        let bad_slot = registry
            .set_layout_override(&json!({
                "targetId": "markdown.preview",
                "property": "slot",
                "value": "main"
            }))
            .unwrap_err();
        assert_eq!(bad_slot.rule, UiContributionRule::InvalidSlot);

        let unknown_token = registry
            .set_layout_override(&json!({
                "targetId": "markdown.preview",
                "property": "themeToken",
                "value": { "token": "markdown.preview.background", "fallback": "surface.overlay" }
            }))
            .unwrap_err();
        assert_eq!(unknown_token.rule, UiContributionRule::InvalidThemeToken);

        let raw_value = registry
            .set_layout_override(&json!({
                "targetId": "markdown.preview",
                "property": "fallback",
                "value": { "rawOps": "Deno.core.ops.op_clay_runtime_ping" }
            }))
            .unwrap_err();
        assert_eq!(raw_value.rule, UiContributionRule::ProhibitedAuthority);
    }

    #[test]
    fn theme_token_registry_rejects_raw_css_raw_colors_and_type_mismatches() {
        let mut registry = PackageUiRegistry::new();
        let package = package();
        let commands = vec!["markdown.togglePreview".to_string()];

        let raw_color_token = registry
            .register_theme_token(
                &package,
                &json!({
                    "token": "markdown.preview.raw",
                    "type": "color-role",
                    "fallback": "surface.panel",
                    "description": "Raw color should be rejected",
                    "rawColor": "#ff00aa"
                }),
            )
            .unwrap_err();
        assert_eq!(
            raw_color_token.rule,
            UiContributionRule::ProhibitedAuthority
        );

        let type_mismatch = registry
            .register_theme_token(
                &package,
                &json!({
                    "token": "markdown.preview.padding",
                    "type": "spacing",
                    "fallback": "surface.panel",
                    "description": "Spacing cannot fall back to a color token"
                }),
            )
            .unwrap_err();
        assert_eq!(type_mismatch.rule, UiContributionRule::InvalidThemeToken);

        registry
            .register_theme_token(
                &package,
                &json!({
                    "token": "markdown.preview.background",
                    "type": "color-role",
                    "fallback": "surface.panel",
                    "description": "Markdown preview background"
                }),
            )
            .unwrap();
        let raw_component_color = registry
            .register_component(
                &package,
                &json!({
                    "kind": "label",
                    "id": "markdown.preview.rawColor",
                    "text": "bad",
                    "style": { "background": "#ff00aa" }
                }),
                &commands,
            )
            .unwrap_err();
        assert_eq!(
            raw_component_color.rule,
            UiContributionRule::ProhibitedAuthority
        );
    }
}
