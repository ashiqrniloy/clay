//! Runtime-backed package UI contribution registry and validators.
//!
//! The public package boundary is the `clay:ui` JavaScript facade.  This module
//! is deliberately crate-internal: it validates inert package UI declarations,
//! preserves package provenance, and stores accepted declarations for later
//! client publication without exposing Masonry widgets, raw ops, CSS, or
//! executable package code.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use self::UiContributionRule as Rule;

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

/// One row of [`CHOICE_FIELDS`]: `(key, allowed, rule, label)`.
///
/// `key` is the name [`validate_choice`] looks the row up by (`<family>.<field>`
/// for nested fields, `<layoutOverride>.<property>` for override values);
/// `label` is the diagnostic prefix that [`choice_message`] completes with the
/// allowed list, so a message can never drift from the values actually accepted.
type ChoiceRow = (
    &'static str,
    &'static [&'static str],
    UiContributionRule,
    &'static str,
);

/// Every closed string-choice field validated by [`validate_choice`]. Adding an
/// allowed value is one edit in the `VALID_*` slice the row points at: the
/// diagnostic text follows that slice.
// A data table: one closed-choice field per line.
#[rustfmt::skip]
const CHOICE_FIELDS: &[ChoiceRow] = &[
    ("slot", VALID_SLOTS, Rule::InvalidSlot, "panel slot must be one of"),
    ("defaultVisibility", VALID_VISIBILITY, Rule::InvalidPolicy, "panel defaultVisibility must be"),
    ("anchor", VALID_OVERLAY_ANCHORS, Rule::InvalidPolicy, "overlay anchor must be one of"),
    ("focusPolicy", VALID_FOCUS_POLICIES, Rule::InvalidPolicy, "overlay focusPolicy must be"),
    ("dismissalPolicy", VALID_DISMISSAL_POLICIES, Rule::InvalidPolicy, "overlay dismissalPolicy must be"),
    ("input.scope", VALID_INPUT_SCOPES, Rule::InvalidInputScope, "input scope must be"),
    ("pointer.click", VALID_POINTER_CLICK_POLICIES, Rule::InvalidPolicy, "pointer.click must be"),
    ("pointer.drag", VALID_POINTER_DRAG_POLICIES, Rule::InvalidPolicy, "pointer.drag must be"),
    ("focus.policy", VALID_COMPONENT_FOCUS_POLICIES, Rule::InvalidFocusPolicy, "focus.policy must be"),
    ("selectionPolicy", VALID_SELECTION_POLICIES, Rule::InvalidPolicy, "selectionPolicy must be"),
    ("state.scope", VALID_UI_STATE_SCOPES, Rule::InvalidStateScope, "UI state scope must be"),
    ("state.owner", VALID_UI_STATE_OWNERS, Rule::InvalidLifecycle, "UI state owner must be"),
    ("state.lifetime", VALID_UI_STATE_LIFETIMES, Rule::InvalidLifecycle, "UI state lifetime must be"),
    ("state.persistence", VALID_UI_STATE_PERSISTENCE, Rule::InvalidLifecycle, "UI state persistence must be"),
    ("implementationStatus", VALID_UI_STATE_STATUSES, Rule::InvalidLifecycle, "implementationStatus must be"),
    ("valueSchema.kind", VALID_UI_STATE_SCHEMA_KINDS, Rule::InvalidStateSchema, "valueSchema.kind must be"),
    ("layoutOverride.property", VALID_LAYOUT_OVERRIDE_PROPERTIES, Rule::InvalidLayoutOverride, "layout override property must be"),
    ("layoutOverride.source", VALID_LAYOUT_OVERRIDE_SOURCES, Rule::InvalidLayoutOverride, "layout override source must be"),
    ("layoutOverride.slot", VALID_SLOTS, Rule::InvalidSlot, "slot override value must be"),
    ("layoutOverride.visibility", VALID_VISIBILITY, Rule::InvalidPolicy, "visibility override value must be"),
    ("layoutOverride.fallback", VALID_FALLBACK_BEHAVIORS, Rule::InvalidLayoutOverride, "fallback override value must be"),
];

fn choice_field(key: &str) -> &'static ChoiceRow {
    CHOICE_FIELDS
        .iter()
        .find(|(row_key, ..)| *row_key == key)
        .unwrap_or_else(|| panic!("choice field `{key}` is not declared in CHOICE_FIELDS"))
}

/// Builds an out-of-set diagnostic: the row's label plus the allowed values
/// (`a, b, or c`).
fn choice_message(allowed: &[&str], label: &str) -> String {
    let Some((last, head)) = allowed.split_last() else {
        return label.to_owned();
    };
    match head {
        [] => format!("{label} {last}"),
        [only] => format!("{label} {only} or {last}"),
        _ => format!("{label} {}, or {last}", head.join(", ")),
    }
}

/// The one validator behind every closed-choice field: allowed values,
/// diagnostic rule, and message all come from the [`CHOICE_FIELDS`] row.
fn validate_choice(
    key: &str,
    value: &str,
    id: &str,
    context: &UiDiagnosticContext,
) -> Result<(), UiContributionDiagnostic> {
    let (_, allowed, rule, label) = choice_field(key);
    if allowed.contains(&value) {
        return Ok(());
    }
    Err(context.error(rule.clone(), Some(id), choice_message(allowed, label)))
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PackageUiRegistry {
    panels: BTreeMap<String, RegisteredPanelContribution>,
    components: BTreeMap<String, RegisteredComponentContribution>,
    overlays: BTreeMap<String, RegisteredTransientOverlayContribution>,
    theme_tokens: BTreeMap<String, RegisteredPackageThemeTokenDeclaration>,
    input_contributions: BTreeMap<String, RegisteredPackageInputContribution>,
    ui_state_scopes: BTreeMap<String, RegisteredPackageUiStateScope>,
    layout_overrides: BTreeMap<String, RegisteredPackageLayoutOverride>,
    layout_intents: BTreeMap<String, RegisteredLayoutIntent>,
    pane_contents: BTreeMap<String, RegisteredPaneContentContribution>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PackageUiRegistrySnapshot {
    pub(crate) panels: Vec<RegisteredPanelContribution>,
    pub(crate) components: Vec<RegisteredComponentContribution>,
    pub(crate) overlays: Vec<RegisteredTransientOverlayContribution>,
    pub(crate) theme_tokens: Vec<RegisteredPackageThemeTokenDeclaration>,
    pub(crate) input_contributions: Vec<RegisteredPackageInputContribution>,
    pub(crate) ui_state_scopes: Vec<RegisteredPackageUiStateScope>,
    pub(crate) layout_overrides: Vec<RegisteredPackageLayoutOverride>,
    pub(crate) layout_intents: Vec<RegisteredLayoutIntent>,
    pub(crate) pane_contents: Vec<RegisteredPaneContentContribution>,
}

impl PackageUiRegistrySnapshot {
    pub(crate) fn validate(&self) -> Result<(), crate::shell::PackageUiRuntimeError> {
        let mut state = crate::shell::PackageUiRuntimeState::new();
        state.apply_update(self.runtime_update(0))
    }

    #[allow(
        dead_code,
        reason = "package UI runtime updates are produced once dynamic package UI publication is wired"
    )]
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
                        "z.overlay",
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

    /// One empty-tab winner, or `Err` of sorted contribution IDs on conflict.
    #[cfg(test)]
    pub(crate) fn empty_tab(
        &self,
    ) -> Result<Option<crate::protocol::EmptyTabContent>, Vec<String>> {
        let mut candidates: Vec<&RegisteredPaneContentContribution> = self
            .pane_contents
            .iter()
            .filter(|entry| entry.activation == "empty-tab")
            .collect();
        candidates.sort_by(|left, right| left.id.cmp(&right.id));
        match candidates.as_slice() {
            [] => Ok(None),
            [winner] => Ok(Some(
                winner.to_wire(crate::protocol::PackageUiTrustDomain::ThirdParty),
            )),
            many => Err(many.iter().map(|entry| entry.id.clone()).collect()),
        }
    }

    pub(crate) fn wire_snapshot(
        &self,
        version: u64,
        trust_domain: impl Fn(&UiContributionProvenance) -> crate::protocol::PackageUiTrustDomain,
    ) -> Result<crate::protocol::PackageUiSnapshot, Vec<String>> {
        let empty_tab = {
            let mut candidates: Vec<_> = self
                .pane_contents
                .iter()
                .filter(|entry| entry.activation == "empty-tab")
                .collect();
            candidates.sort_by(|left, right| left.id.cmp(&right.id));
            match candidates.as_slice() {
                [] => None,
                [winner] => Some(winner.to_wire(trust_domain(&winner.provenance))),
                many => return Err(many.iter().map(|entry| entry.id.clone()).collect()),
            }
        };
        Ok(crate::protocol::PackageUiSnapshot {
            version,
            empty_tab,
            surfaces: {
                let mut surfaces: Vec<_> = self
                    .pane_contents
                    .iter()
                    .filter(|entry| entry.activation == "pane")
                    .collect();
                surfaces.sort_by(|left, right| left.id.cmp(&right.id));
                surfaces
                    .into_iter()
                    .map(|entry| entry.to_wire(trust_domain(&entry.provenance)))
                    .collect()
            },
            panels: self
                .panels
                .iter()
                .map(|panel| panel.to_wire(trust_domain(&panel.provenance)))
                .collect(),
            overlays: self
                .overlays
                .iter()
                .map(|overlay| overlay.to_wire(trust_domain(&overlay.provenance)))
                .collect(),
            components: self
                .components
                .iter()
                .map(|component| component.to_wire(trust_domain(&component.provenance)))
                .collect(),
            input_routes: self
                .input_contributions
                .iter()
                .map(|input| input.to_wire(trust_domain(&input.provenance)))
                .collect(),
        })
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
    pub(crate) component_json: String,
    pub(crate) component_tree: PackageUiComponentTree,
    pub(crate) action_targets: Vec<String>,
    pub(crate) provenance: UiContributionProvenance,
    pub(crate) estimated_payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredPaneContentContribution {
    pub(crate) id: String,
    pub(crate) activation: String,
    pub(crate) component_id: String,
    pub(crate) component_tree: PackageUiComponentTree,
    pub(crate) component_json: String,
    pub(crate) action_targets: Vec<String>,
    pub(crate) provenance: UiContributionProvenance,
    pub(crate) estimated_payload_bytes: usize,
}

impl UiContributionProvenance {
    fn to_wire(
        &self,
        trust_domain: crate::protocol::PackageUiTrustDomain,
    ) -> crate::protocol::PackageUiProvenance {
        crate::protocol::PackageUiProvenance {
            package_name: self.package_name.clone(),
            package_version: self.package_version.clone(),
            api_prefix: self.api_prefix.clone(),
            trust_domain,
        }
    }
}

impl RegisteredPanelContribution {
    fn to_wire(
        &self,
        trust_domain: crate::protocol::PackageUiTrustDomain,
    ) -> crate::protocol::PackagePanelContent {
        crate::protocol::PackagePanelContent {
            id: self.id.clone(),
            slot: self.slot.clone(),
            visibility: self.default_visibility.clone(),
            component_json: self.component_json.clone(),
            action_targets: self.action_targets.clone(),
            provenance: self.provenance.to_wire(trust_domain),
        }
    }
}

impl RegisteredPaneContentContribution {
    fn to_wire(
        &self,
        trust_domain: crate::protocol::PackageUiTrustDomain,
    ) -> crate::protocol::EmptyTabContent {
        crate::protocol::EmptyTabContent {
            id: self.id.clone(),
            package_name: self.provenance.package_name.clone(),
            component_json: self.component_json.clone(),
            action_targets: self.action_targets.clone(),
            provenance: self.provenance.to_wire(trust_domain),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegisteredComponentContribution {
    pub(crate) id: String,
    pub(crate) component_json: String,
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
    pub(crate) component_json: String,
    pub(crate) component_tree: PackageUiComponentTree,
    pub(crate) action_targets: Vec<String>,
    pub(crate) provenance: UiContributionProvenance,
    pub(crate) estimated_payload_bytes: usize,
}

impl RegisteredComponentContribution {
    fn to_wire(
        &self,
        trust_domain: crate::protocol::PackageUiTrustDomain,
    ) -> crate::protocol::PackageComponentContent {
        crate::protocol::PackageComponentContent {
            id: self.id.clone(),
            component_json: self.component_json.clone(),
            action_targets: self.action_targets.clone(),
            provenance: self.provenance.to_wire(trust_domain),
        }
    }
}

impl RegisteredTransientOverlayContribution {
    fn to_wire(
        &self,
        trust_domain: crate::protocol::PackageUiTrustDomain,
    ) -> crate::protocol::PackageOverlayContent {
        crate::protocol::PackageOverlayContent {
            id: self.id.clone(),
            anchor: self.anchor.clone(),
            focus_policy: self.focus_policy.clone(),
            dismissal_policy: self.dismissal_policy.clone(),
            component_json: self.component_json.clone(),
            action_targets: self.action_targets.clone(),
            provenance: self.provenance.to_wire(trust_domain),
        }
    }
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

impl RegisteredPackageInputContribution {
    fn to_wire(
        &self,
        trust_domain: crate::protocol::PackageUiTrustDomain,
    ) -> crate::protocol::PackageInputRouteContent {
        crate::protocol::PackageInputRouteContent {
            id: self.id.clone(),
            scope: self.scope.clone(),
            component_id: self.component_id.clone(),
            pointer_click: self.pointer_click.clone(),
            pointer_action: self.pointer_action.clone(),
            pointer_drag: self.pointer_drag.clone(),
            focus_policy: self.focus_policy.clone(),
            selection_policy: self.selection_policy.clone(),
            context_modes: self.context_modes.clone(),
            action_targets: self.action_targets.clone(),
            provenance: self.provenance.to_wire(trust_domain),
        }
    }
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

/// Phase 20.3: Inert versioned layout intent submitted by a package.
///
/// Packages request pane splits through this advisory struct. Clay validates
/// and stores the intent; composition into `WorkingAreaLayoutUpdate` happens
/// at Clay's discretion. Packages cannot mutate native layout directly.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RegisteredLayoutIntent {
    pub(crate) id: String,
    pub(crate) target_pane: String,
    pub(crate) orientation: String,
    pub(crate) ratio: f64,
    pub(crate) position: String,
    pub(crate) source: String,
    pub(crate) estimated_payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UiContributionDiagnostic {
    pub(crate) package_name: Option<Box<str>>,
    pub(crate) package_version: Option<Box<str>>,
    pub(crate) api_prefix: Option<Box<str>>,
    pub(crate) contribution_id: Option<Box<str>>,
    pub(crate) rule: UiContributionRule,
    pub(crate) message: Box<str>,
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
    InvalidLayoutIntent,
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
            layout_intents: self.layout_intents.values().cloned().collect(),
            pane_contents: self.pane_contents.values().cloned().collect(),
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
        validate_choice("slot", slot, &id, &context)?;
        let default_visibility = optional_str(object, "defaultVisibility").unwrap_or("hidden");
        validate_choice("defaultVisibility", default_visibility, &id, &context)?;
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
            component_json: component_value.to_string(),
            component_tree,
            action_targets,
            provenance: UiContributionProvenance::from(package),
            estimated_payload_bytes: size,
        };
        self.panels.insert(id, registered.clone());
        Ok(registered)
    }

    pub(crate) fn register_pane_content(
        &mut self,
        package: &ClayPackageManifest,
        declaration: &Value,
        registered_command_ids: &[String],
    ) -> Result<RegisteredPaneContentContribution, UiContributionDiagnostic> {
        let context = UiDiagnosticContext::from_package(package, None);
        validate_provenance(package, &context)?;
        let size = payload_size(declaration);
        if size > SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES {
            return Err(context.error(
                UiContributionRule::PayloadTooLarge,
                None,
                format!(
                    "pane-content contribution payload ({size} bytes) exceeds SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES ({SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_prohibited_authority(declaration, &context)?;
        let object = declaration.as_object().ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidComponent,
                None,
                "pane-content contribution declaration must be an object",
            )
        })?;
        let id = package_owned_string(object, "id", package, UiContributionRule::InvalidId)?;
        let context = UiDiagnosticContext::from_package(package, Some(id.clone()));
        if self.pane_contents.contains_key(&id) {
            return Err(context.error(
                UiContributionRule::DuplicateId,
                Some(&id),
                "pane-content contribution IDs must be unique",
            ));
        }
        let activation = required_str(
            object,
            "activation",
            UiContributionRule::InvalidPolicy,
            &context,
        )?;
        // Phase 2 (plan 108 task 8): generic activations. `empty-tab` keeps the
        // single-winner landing election; `pane` is a named pane surface any
        // app-like package may present in a working-area pane. The vocabulary
        // is closed so clients can stay deny-by-default.
        if activation != "empty-tab" && activation != "pane" {
            return Err(context.error(
                UiContributionRule::InvalidPolicy,
                Some(&id),
                "pane-content activation must be empty-tab or pane",
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

        let registered = RegisteredPaneContentContribution {
            id: id.clone(),
            activation: activation.to_string(),
            component_id,
            component_json: component_value.to_string(),
            component_tree,
            action_targets,
            provenance: UiContributionProvenance::from(package),
            estimated_payload_bytes: size,
        };
        self.pane_contents.insert(id, registered.clone());
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
            component_json: declaration.to_string(),
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
        validate_choice("anchor", anchor, &id, &context)?;
        let focus_policy = optional_str(object, "focusPolicy").unwrap_or("restore");
        validate_choice("focusPolicy", focus_policy, &id, &context)?;
        let dismissal_policy = optional_str(object, "dismissalPolicy").unwrap_or("escape");
        validate_choice("dismissalPolicy", dismissal_policy, &id, &context)?;
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
            component_json: component_value.to_string(),
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
        validate_choice("input.scope", scope, &id, &context)?;
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
        validate_choice("pointer.click", pointer_click, &id, &context)?;
        let pointer_drag = pointer
            .and_then(|pointer| optional_str(pointer, "drag"))
            .unwrap_or("none");
        validate_choice("pointer.drag", pointer_drag, &id, &context)?;
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
        validate_choice("focus.policy", focus_policy, &id, &context)?;
        let selection_policy = optional_str(object, "selectionPolicy").unwrap_or("preserve-editor");
        validate_choice("selectionPolicy", selection_policy, &id, &context)?;
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
        validate_choice("state.scope", scope, &id, &context)?;
        let owner = required_str(
            object,
            "owner",
            UiContributionRule::InvalidLifecycle,
            &context,
        )?;
        validate_choice("state.owner", owner, &id, &context)?;
        let lifetime = required_str(
            object,
            "lifetime",
            UiContributionRule::InvalidLifecycle,
            &context,
        )?;
        validate_choice("state.lifetime", lifetime, &id, &context)?;
        let persistence = required_str(
            object,
            "persistence",
            UiContributionRule::InvalidLifecycle,
            &context,
        )?;
        validate_choice("state.persistence", persistence, &id, &context)?;
        let implementation_status =
            optional_str(object, "implementationStatus").unwrap_or("deferred");
        validate_choice("implementationStatus", implementation_status, &id, &context)?;
        let target_id = optional_str(object, "targetId").map(ToOwned::to_owned);
        if matches!(scope, "pane" | "component" | "transient-overlay") && target_id.is_none() {
            return Err(context.error(
                UiContributionRule::InvalidStateScope,
                Some(&id),
                "pane, component, and transient-overlay state scopes require a package-prefixed targetId",
            ));
        }
        if let Some(target_id) = &target_id
            && !target_id.starts_with(&format!("{}.", package.clay.api_prefix))
        {
            return Err(context.error(
                UiContributionRule::InvalidId,
                Some(target_id),
                "state scope targetId must use the package apiPrefix",
            ));
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
        validate_choice("valueSchema.kind", value_schema_kind, &id, &context)?;
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
        validate_choice("layoutOverride.property", property, property, &context)?;
        let source = optional_str(object, "source").unwrap_or("user-config");
        validate_choice("layoutOverride.source", source, source, &context)?;
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

    /// Phase 20.3: Validate and store an inert layout intent from a package.
    pub(crate) fn request_layout_intent(
        &mut self,
        package: &ClayPackageManifest,
        declaration: &Value,
    ) -> Result<RegisteredLayoutIntent, UiContributionDiagnostic> {
        let context = UiDiagnosticContext::from_package(package, None);
        validate_provenance(package, &context)?;
        let size = payload_size(declaration);
        if size > SDUI_UPDATE_PAYLOAD_BUDGET_BYTES {
            return Err(context.error(
                UiContributionRule::PayloadTooLarge,
                None,
                format!(
                    "layout intent payload ({size} bytes) exceeds SDUI_UPDATE_PAYLOAD_BUDGET_BYTES ({SDUI_UPDATE_PAYLOAD_BUDGET_BYTES} bytes)"
                ),
            ));
        }
        reject_prohibited_authority(declaration, &context)?;
        let object = declaration.as_object().ok_or_else(|| {
            context.error(
                UiContributionRule::InvalidLayoutIntent,
                None,
                "layout intent declaration must be an object",
            )
        })?;
        let id = package_owned_string(object, "id", package, UiContributionRule::InvalidId)?;
        if self.layout_intents.contains_key(&id) {
            return Err(context.error(
                UiContributionRule::DuplicateId,
                Some(&id),
                "layout intent id already registered",
            ));
        }
        let target_pane = required_str(
            object,
            "targetPane",
            UiContributionRule::InvalidLayoutIntent,
            &context,
        )?;
        let orientation = required_str(
            object,
            "orientation",
            UiContributionRule::InvalidLayoutIntent,
            &context,
        )?;
        if !matches!(orientation, "horizontal" | "vertical") {
            return Err(context.error(
                UiContributionRule::InvalidLayoutIntent,
                Some(orientation),
                "layout intent orientation must be horizontal or vertical",
            ));
        }
        let ratio = object
            .get("ratio")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidLayoutIntent,
                    Some(&id),
                    "layout intent requires a numeric ratio",
                )
            })?;
        if !ratio.is_finite() || !(0.05..=0.95).contains(&ratio) {
            return Err(context.error(
                UiContributionRule::InvalidLayoutIntent,
                Some(&id),
                format!("layout intent ratio {ratio} must be between 0.05 and 0.95"),
            ));
        }
        let position = optional_str(object, "position").unwrap_or("second");
        if !matches!(position, "first" | "second") {
            return Err(context.error(
                UiContributionRule::InvalidLayoutIntent,
                Some(position),
                "layout intent position must be first or second",
            ));
        }
        let registered = RegisteredLayoutIntent {
            id: id.clone(),
            target_pane: target_pane.to_string(),
            orientation: orientation.to_string(),
            ratio,
            position: position.to_string(),
            source: package.clay.api_prefix.clone(),
            estimated_payload_bytes: size,
        };
        self.layout_intents.insert(id, registered.clone());
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
                format!(
                    "theme token type must be one of: {}",
                    ThemeTokenType::all_as_str().join(", ")
                ),
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

/// Validate an optional semantic icon reference on a package component or
/// component list item (Plan 112 task 3). Only button, label, list, and
/// statusItem kinds accept icons; references must be core keys or the
/// declaring package's apiPrefix namespace.
fn validate_component_icon(
    value: Option<&Value>,
    kind: crate::shell::components::ComponentKind,
    api_prefix: &str,
) -> Result<Option<String>, String> {
    let Some(reference) = value else {
        return Ok(None);
    };
    if reference.is_null() {
        return Ok(None);
    }
    let Value::String(reference) = reference else {
        return Err("component icon references must be strings".to_string());
    };
    let icon_kind_allowed = matches!(
        kind,
        crate::shell::components::ComponentKind::Button
            | crate::shell::components::ComponentKind::Label
            | crate::shell::components::ComponentKind::List
            | crate::shell::components::ComponentKind::StatusItem
    );
    if !icon_kind_allowed {
        return Err(format!(
            "semantic icons are only supported by button, label, list, and statusItem components (got kind `{}`)",
            kind.as_str()
        ));
    }
    crate::shell::icons::validate_icon_reference(reference, api_prefix)
        .map_err(|error| error.message)?;
    Ok(Some(reference.clone()))
}

struct ComponentValidationContext<'a> {
    package: &'a ClayPackageManifest,
    registered_command_ids: &'a [String],
    theme_resolver: &'a ThemeTokenResolver,
    seen_ids: BTreeSet<String>,
    action_targets: Vec<String>,
    icon_references: Vec<String>,
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
            icon_references: Vec::new(),
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
        let component_kind = validate_component_kind(kind).map_err(|error| {
            context.error(
                UiContributionRule::InvalidComponent,
                Some(&error.field),
                error.message,
            )
        })?;
        let id = package_owned_string(object, "id", self.package, UiContributionRule::InvalidId)?;
        let context = UiDiagnosticContext::from_package(self.package, Some(id.clone()));
        if let Some(icon) = validate_component_icon(
            object.get("icon"),
            component_kind,
            self.package.clay.api_prefix.as_str(),
        )
        .map_err(|message| {
            context.error(UiContributionRule::InvalidComponent, Some(&id), message)
        })? {
            // Record validated references so conflicts/diagnostics can identify
            // icon consumers without re-parsing declarations.
            self.icon_references.push(icon);
        }
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
        if style_variables
            .iter()
            .any(|variable| variable.name == "fontRole")
            && !component_kind.supports_text_font_role()
        {
            return Err(context.error(
                UiContributionRule::InvalidComponent,
                Some(&id),
                "style.fontRole is only supported by text-bearing panel, label, button, list, and statusItem components",
            ));
        }
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
                if let Some(icon) = validate_component_icon(
                    item_object.get("icon"),
                    component_kind,
                    self.package.clay.api_prefix.as_str(),
                )
                .map_err(|message| {
                    context.error(UiContributionRule::InvalidComponent, Some(&id), message)
                })? {
                    self.icon_references.push(icon);
                }
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
        message: impl Into<Box<str>>,
    ) -> UiContributionDiagnostic {
        UiContributionDiagnostic {
            package_name: self.package_name.clone().map(String::into_boxed_str),
            package_version: self.package_version.clone().map(String::into_boxed_str),
            api_prefix: self.api_prefix.clone().map(String::into_boxed_str),
            contribution_id: contribution_id
                .map(|id| id.to_string().into_boxed_str())
                .or_else(|| self.contribution_id.clone().map(String::into_boxed_str)),
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

const CLIENT_DIALOG_ACTIONS: &[&str] = &[
    "documents.clientOpenFileDialog",
    "workspace.clientOpenFolderDialog",
    "agent.clientOpenAgentPicker",
    "agent.clientOpenProviderPicker",
    "agent.clientOpenModelPicker",
    "agent.clientOpenProviderSetup",
    "agent.clientOpenSessionPicker",
    "agent.clientOpenSessionSearchPicker",
];

fn validate_registered_actions(
    action_targets: &[String],
    registered_command_ids: &[String],
    context: &UiDiagnosticContext,
) -> Result<(), UiContributionDiagnostic> {
    for command_id in action_targets {
        if CLIENT_DIALOG_ACTIONS.contains(&command_id.as_str()) {
            continue;
        }
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
            validate_choice("layoutOverride.slot", slot, slot, context)?;
        }
        "visibility" => {
            let visibility = value.as_str().ok_or_else(|| {
                context.error(
                    UiContributionRule::InvalidLayoutOverride,
                    Some(target_id),
                    "visibility override value must be a string",
                )
            })?;
            validate_choice("layoutOverride.visibility", visibility, visibility, context)?;
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
            validate_choice("layoutOverride.fallback", fallback, fallback, context)?;
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

#[allow(
    dead_code,
    reason = "slot parsing helper belongs to package UI validation path and is kept with related code"
)]
fn fixed_slot_id(slot: &str) -> FixedSlotId {
    match slot {
        "right" => FixedSlotId::Right,
        "top" => FixedSlotId::Top,
        "bottom" => FixedSlotId::Bottom,
        _ => FixedSlotId::Left,
    }
}

#[cfg(test)]
mod tests;
