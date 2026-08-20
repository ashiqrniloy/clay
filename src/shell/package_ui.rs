//! Clay-owned slot-aware package UI runtime state.
//!
//! The server validates package `clay:ui` declarations before they reach this
//! module. This runtime state composes accepted fixed panels and transient
//! overlays into shell-owned slot geometry without exposing Masonry widget IDs,
//! native handles, raw CSS, raw ops, renderer callbacks, or executable package
//! code to packages.

#![allow(
    dead_code,
    reason = "package UI runtime descriptors are validated and documented before all shell callsites are enabled"
)]

use std::collections::{BTreeMap, BTreeSet};

use masonry::kurbo::Rect;
use serde_json::{Map, Value};

use crate::{
    editor::typography::{TypographyRegistry, UiTextVariant},
    perf::budgets::{COMPLETION_MAX_VISIBLE_ROWS, COMPLETION_MAX_WIDTH_PX},
    protocol::{FontRole, PackageUiSnapshot},
};

use super::layout::{FixedSlotId, FixedSlotState, PaneSlotLayout};
use super::theme::{PanelDefaults, ResolvedUiTheme, SduiThemeStyle};
use super::transient_menu::{
    TransientMenuFocusPolicy, TransientMenuItem, TransientMenuOrigin, TransientMenuSession,
    TransientMenuStatus,
};

const MAX_FIXED_PANELS: usize = 4;
const MAX_TRANSIENT_OVERLAYS: usize = 16;
const MAX_INPUT_ROUTES: usize = 64;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PackageUiRuntimeState {
    version: u64,
    fixed_panels: BTreeMap<FixedSlotId, FixedPackagePanel>,
    transient_overlays: BTreeMap<String, TransientPackageOverlay>,
    input_routing: BTreeMap<String, PackageInputRouting>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageUiRuntimeUpdate {
    pub(crate) base_version: u64,
    pub(crate) fixed_panels: Vec<FixedPackagePanel>,
    pub(crate) transient_overlays: Vec<TransientPackageOverlay>,
    pub(crate) input_routing: Vec<PackageInputRouting>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixedPackagePanel {
    pub(crate) id: String,
    pub(crate) slot_id: FixedSlotId,
    pub(crate) visibility: PackagePanelVisibility,
    pub(crate) component: PackageUiComponentTree,
    pub(crate) action_targets: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackagePanelVisibility {
    Visible,
    Hidden,
    Collapsed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransientPackageOverlay {
    pub(crate) id: String,
    pub(crate) anchor: PackageOverlayAnchor,
    pub(crate) focus_policy: String,
    pub(crate) dismissal_policy: String,
    pub(crate) component: PackageUiComponentTree,
    pub(crate) action_targets: Vec<String>,
    /// Phase 20.5: z-level token name for overlay stacking order.
    /// One of `"z.overlay"`, `"z.modal"`, `"z.tooltip"`.
    pub(crate) z_level_token: &'static str,
    /// Plan 070 step 13f: hosted menu a11y payload. `Some` only for overlays built
    /// by [`TransientPackageOverlay::from_menu_session`]; the hosted
    /// `PackageRegionWidget` builds a `Menu`/`MenuItem`/`Status` a11y subtree
    /// from it instead of letting the generic `Group`/`ListItem` subtree flow.
    /// Item labels are finalized by the shared bounded accessibility helper
    /// before this payload reaches Masonry.
    /// `None` for package-declared overlays (they keep the generic subtree).
    pub(crate) menu_a11y: Option<MenuA11y>,
}

/// Plan 070 step 13f: a11y payload for a hosted transient menu — the hosted
/// `PackageRegionWidget` reports `Role::Menu` (prompt) > `Role::MenuItem`
/// (already bounded/sanitized rows, with a `" selected"` suffix on the active
/// item) + `Role::Status` (empty-state message), matching the legacy
/// `collect_active_menu_accessibility_entries` screen-reader contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MenuA11y {
    pub(crate) prompt: String,
    pub(crate) items: Vec<MenuA11yItem>,
    pub(crate) status: Option<String>,
    /// Phase 24.4: centered Command Centre surfaces expose one stable polite
    /// result-count status node, separate from empty-state detail text.
    pub(crate) result_count: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MenuA11yItem {
    /// Final bounded/sanitized row label, including the in-budget ` selected`
    /// suffix when active. Display/action data remains on the session item.
    pub(crate) label: String,
    pub(crate) selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackageOverlayAnchor {
    WorkingArea,
    ActivePane,
    Main,
    Pointer,
    Bottom,
    /// Clay-native completion surface anchored to the active caret.
    Completion,
    /// Phase 24.4: window-centered Command Centre surface. Clay-internal only:
    /// `parse` never produces it (packages keep the four documented anchors),
    /// and it is not part of `VALID_OVERLAY_ANCHORS` on the server.
    Centered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageInputRouting {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageUiComponentTree {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) font_role: FontRole,
    pub(crate) text_variant: Option<UiTextVariant>,
    pub(crate) title: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) label: Option<String>,
    pub(crate) action_command_id: Option<String>,
    pub(crate) items: Vec<PackageUiListItem>,
    pub(crate) children: Vec<PackageUiComponentTree>,
    /// Phase 20.4: component-level disabled flag. Disabled components render
    /// with the disabled state tokens and are gated out of `SduiVisibleAction`
    /// (their actions are not dispatchable).
    pub(crate) disabled: bool,
    /// Phase 20.5: text input validation state (`none`/`error`/`warning`/`success`).
    /// Parsed from `style.validationState`; `None` means no validation state declared.
    pub(crate) validation_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageUiListItem {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) detail: Option<String>,
    pub(crate) action_command_id: Option<String>,
    pub(crate) selected: bool,
    /// Phase 20.4: row-level disabled flag. Disabled rows render with the
    /// disabled state tokens and are gated out of `SduiVisibleAction`.
    pub(crate) disabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PackageUiPanelObservation {
    pub(crate) id: String,
    pub(crate) slot_id: FixedSlotId,
    pub(crate) rect: Rect,
    pub(crate) component_id: String,
    pub(crate) component_kind: String,
    pub(crate) title: Option<String>,
    pub(crate) visible: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PackageUiOverlayObservation {
    pub(crate) id: String,
    pub(crate) anchor: PackageOverlayAnchor,
    pub(crate) rect: Rect,
    pub(crate) component_id: String,
    pub(crate) component_kind: String,
    pub(crate) focus_policy: String,
    pub(crate) dismissal_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PackageUiRuntimeError {
    StaleVersion { expected: u64, actual: u64 },
    TooManyFixedPanels { count: usize, max: usize },
    TooManyTransientOverlays { count: usize, max: usize },
    DuplicateFixedSlot { slot_id: FixedSlotId },
    DuplicateContributionId { id: String },
    TooManyInputRoutes { count: usize, max: usize },
}

impl PackageUiRuntimeState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn version(&self) -> u64 {
        self.version
    }

    pub(crate) fn has_fixed_panels(&self) -> bool {
        !self.fixed_panels.is_empty()
    }

    pub(crate) fn fixed_panel_for_slot(&self, slot_id: FixedSlotId) -> Option<&FixedPackagePanel> {
        self.fixed_panels.get(&slot_id)
    }

    pub(crate) fn transient_overlay_count(&self) -> usize {
        self.transient_overlays.len()
    }

    pub(crate) fn apply_update(
        &mut self,
        update: PackageUiRuntimeUpdate,
    ) -> Result<(), PackageUiRuntimeError> {
        if update.base_version != self.version {
            return Err(PackageUiRuntimeError::StaleVersion {
                expected: self.version,
                actual: update.base_version,
            });
        }
        if update.fixed_panels.len() > MAX_FIXED_PANELS {
            return Err(PackageUiRuntimeError::TooManyFixedPanels {
                count: update.fixed_panels.len(),
                max: MAX_FIXED_PANELS,
            });
        }
        if update.transient_overlays.len() > MAX_TRANSIENT_OVERLAYS {
            return Err(PackageUiRuntimeError::TooManyTransientOverlays {
                count: update.transient_overlays.len(),
                max: MAX_TRANSIENT_OVERLAYS,
            });
        }
        if update.input_routing.len() > MAX_INPUT_ROUTES {
            return Err(PackageUiRuntimeError::TooManyInputRoutes {
                count: update.input_routing.len(),
                max: MAX_INPUT_ROUTES,
            });
        }

        let mut ids = BTreeSet::new();
        let mut fixed_panels = BTreeMap::new();
        for panel in update.fixed_panels {
            if !ids.insert(panel.id.clone()) {
                return Err(PackageUiRuntimeError::DuplicateContributionId { id: panel.id });
            }
            let slot_id = panel.slot_id;
            if fixed_panels.insert(slot_id, panel).is_some() {
                return Err(PackageUiRuntimeError::DuplicateFixedSlot { slot_id });
            }
        }

        let mut transient_overlays = BTreeMap::new();
        for overlay in update.transient_overlays {
            if !ids.insert(overlay.id.clone()) {
                return Err(PackageUiRuntimeError::DuplicateContributionId { id: overlay.id });
            }
            if transient_overlays
                .insert(overlay.id.clone(), overlay.clone())
                .is_some()
            {
                return Err(PackageUiRuntimeError::DuplicateContributionId { id: overlay.id });
            }
        }

        let mut input_routing = BTreeMap::new();
        for route in update.input_routing {
            if !ids.insert(route.id.clone()) {
                return Err(PackageUiRuntimeError::DuplicateContributionId { id: route.id });
            }
            if input_routing
                .insert(route.id.clone(), route.clone())
                .is_some()
            {
                return Err(PackageUiRuntimeError::DuplicateContributionId { id: route.id });
            }
        }

        self.version = self.version.saturating_add(1);
        self.fixed_panels = fixed_panels;
        self.transient_overlays = transient_overlays;
        self.input_routing = input_routing;
        Ok(())
    }

    /// Replace package UI for a runtime-generation snapshot.
    ///
    /// Contribution payloads are empty until package UI crosses IPC, so this
    /// clears previous panels/overlays/routes and advances the version to the
    /// snapshot generation under one install boundary.
    pub(crate) fn install_runtime_snapshot(&mut self, snapshot: &PackageUiSnapshot) {
        self.version = snapshot.version;
        self.fixed_panels.clear();
        self.transient_overlays.clear();
        self.input_routing.clear();
    }

    pub(crate) fn slot_layout(&self, defaults: &PanelDefaults) -> PaneSlotLayout {
        self.fixed_panels
            .values()
            .fold(PaneSlotLayout::main_only(), |layout, panel| {
                layout.with_fixed_slot(panel.fixed_slot_state(defaults))
            })
    }

    pub(crate) fn fixed_panel_observations(
        &self,
        working_area: Rect,
        defaults: &PanelDefaults,
    ) -> Vec<PackageUiPanelObservation> {
        let geometry = self.slot_layout(defaults).compute_geometry(working_area);
        self.fixed_panels
            .iter()
            .map(|(slot_id, panel)| {
                let rect = geometry
                    .fixed_slots
                    .iter()
                    .find(|slot| slot.slot_id == *slot_id)
                    .map_or(Rect::ZERO, |slot| slot.rect);
                PackageUiPanelObservation {
                    id: panel.id.clone(),
                    slot_id: *slot_id,
                    rect,
                    component_id: panel.component.id.clone(),
                    component_kind: panel.component.kind.clone(),
                    title: panel.component.title.clone(),
                    visible: panel.visibility == PackagePanelVisibility::Visible,
                }
            })
            .collect()
    }

    pub(crate) fn overlay_observations(
        &self,
        working_area: Rect,
        defaults: &PanelDefaults,
    ) -> Vec<PackageUiOverlayObservation> {
        let slot_geometry = self.slot_layout(defaults).compute_geometry(working_area);
        self.transient_overlays
            .values()
            .map(|overlay| PackageUiOverlayObservation {
                id: overlay.id.clone(),
                anchor: overlay.anchor,
                rect: overlay.anchor.rect(working_area, slot_geometry.main_rect),
                component_id: overlay.component.id.clone(),
                component_kind: overlay.component.kind.clone(),
                focus_policy: overlay.focus_policy.clone(),
                dismissal_policy: overlay.dismissal_policy.clone(),
            })
            .collect()
    }

    pub(crate) fn visible_fixed_panels(
        &self,
        working_area: Rect,
        defaults: &PanelDefaults,
    ) -> Vec<(Rect, &FixedPackagePanel)> {
        let geometry = self.slot_layout(defaults).compute_geometry(working_area);
        self.fixed_panels
            .iter()
            .filter_map(|(slot_id, panel)| {
                if panel.visibility != PackagePanelVisibility::Visible {
                    return None;
                }
                geometry
                    .fixed_slots
                    .iter()
                    .find(|slot| slot.slot_id == *slot_id)
                    .map(|slot| (slot.rect, panel))
            })
            .collect()
    }

    /// Visible fixed panels keyed by slot, without geometry (visibility is a
    /// pure field check). Used by the retained panel host to reconcile the panel
    /// set; rects are computed separately in layout from the working-area size.
    pub(crate) fn visible_fixed_panel_components(&self) -> Vec<(FixedSlotId, &FixedPackagePanel)> {
        self.fixed_panels
            .iter()
            .filter(|(_, panel)| panel.visibility == PackagePanelVisibility::Visible)
            .map(|(slot_id, panel)| (*slot_id, panel))
            .collect()
    }

    pub(crate) fn overlays(&self) -> impl Iterator<Item = &TransientPackageOverlay> {
        self.transient_overlays.values()
    }

    pub(crate) fn input_routes(&self) -> impl Iterator<Item = &PackageInputRouting> {
        self.input_routing.values()
    }
}

impl PackageInputRouting {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        id: impl Into<String>,
        scope: impl Into<String>,
        component_id: impl Into<String>,
        pointer_click: impl Into<String>,
        pointer_action: Option<String>,
        pointer_drag: impl Into<String>,
        focus_policy: impl Into<String>,
        selection_policy: impl Into<String>,
        context_modes: Vec<String>,
        action_targets: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            scope: scope.into(),
            component_id: component_id.into(),
            pointer_click: pointer_click.into(),
            pointer_action,
            pointer_drag: pointer_drag.into(),
            focus_policy: focus_policy.into(),
            selection_policy: selection_policy.into(),
            context_modes,
            action_targets,
        }
    }
}

impl FixedPackagePanel {
    pub(crate) fn new(
        id: impl Into<String>,
        slot_id: FixedSlotId,
        visibility: PackagePanelVisibility,
        component: PackageUiComponentTree,
        action_targets: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            slot_id,
            visibility,
            component,
            action_targets,
        }
    }

    fn fixed_slot_state(&self, defaults: &PanelDefaults) -> FixedSlotState {
        FixedSlotState {
            slot_id: self.slot_id,
            size: defaults.default_size(self.slot_id),
            min_size: defaults.min_size(self.slot_id),
            max_size: defaults.max_size(self.slot_id),
            visible: self.visibility == PackagePanelVisibility::Visible,
            collapsed: self.visibility == PackagePanelVisibility::Collapsed,
            resized_by_user: false,
        }
    }
}

impl TransientPackageOverlay {
    pub(crate) fn new(
        id: impl Into<String>,
        anchor: PackageOverlayAnchor,
        focus_policy: impl Into<String>,
        dismissal_policy: impl Into<String>,
        component: PackageUiComponentTree,
        action_targets: Vec<String>,
        z_level_token: &'static str,
    ) -> Self {
        Self {
            id: id.into(),
            anchor,
            focus_policy: focus_policy.into(),
            dismissal_policy: dismissal_policy.into(),
            component,
            action_targets,
            z_level_token,
            menu_a11y: None,
        }
    }

    /// Projects a generic `TransientMenuSession` onto a Clay-owned transient
    /// overlay anchored to the bottom of the main pane. The resulting component
    /// tree carries only inert command IDs and bounded JSON arguments; it does
    /// not embed callbacks, native handles, raw CSS, or executable code.
    pub(crate) fn from_menu_session(session: &TransientMenuSession) -> Self {
        // Plan 070 step 13f: hosted menu a11y payload (prompt, resolved item
        // labels + selected, empty-state status). Built once from the session;
        // the hosted `PackageRegionWidget` reports `Menu`/`MenuItem`/`Status`
        // from it regardless of which component-tree branch renders below.
        // Package-authored labels are normalized here, before Masonry sees the
        // payload; display/action data remains unchanged.
        let menu_a11y = MenuA11y {
            prompt: crate::editor::accessibility::sanitize_recovery_summary(session.prompt())
                .unwrap_or_else(|| "Transient menu".to_string()),
            items: session
                .items()
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let selected = index == session.selected_index();
                    MenuA11yItem {
                        label: crate::editor::accessibility::compose_menu_item_accessibility_label(
                            &item.accessibility_label,
                            &item.label,
                            selected,
                        ),
                        selected,
                    }
                })
                .collect(),
            status: match session.status() {
                TransientMenuStatus::Empty { message } => {
                    crate::editor::accessibility::sanitize_recovery_summary(message)
                }
                _ => None,
            },
            result_count: (session.origin() == TransientMenuOrigin::Centered).then(|| {
                crate::editor::accessibility::compose_menu_result_count(session.items().len())
            }),
        };
        // Phase 20.5: anchor selected by surface origin.
        let anchor = match session.origin() {
            TransientMenuOrigin::ContextMenu => PackageOverlayAnchor::Pointer,
            TransientMenuOrigin::MenuBar => PackageOverlayAnchor::Main,
            TransientMenuOrigin::CommandPalette => PackageOverlayAnchor::Bottom,
            TransientMenuOrigin::Completion => PackageOverlayAnchor::Completion,
            // Phase 24.4: command/path mode request the window-centered
            // surface; the host routes it to the window-level overlay layer.
            TransientMenuOrigin::Centered => PackageOverlayAnchor::Centered,
        };
        let prompt_id = format!("menu.{}.prompt", session.session_id().0);
        let query_id = format!("menu.{}.query", session.session_id().0);
        let list_id = format!("menu.{}.list", session.session_id().0);
        let status_id = format!("menu.{}.status", session.session_id().0);

        let mut children = vec![
            PackageUiComponentTree {
                id: prompt_id,
                disabled: false,
                kind: "label".to_string(),
                font_role: FontRole::Ui,
                text_variant: None,
                title: None,
                text: Some(session.prompt().to_string()),
                label: Some(session.prompt().to_string()),
                action_command_id: None,
                items: Vec::new(),
                children: Vec::new(),
                validation_state: None,
            },
            PackageUiComponentTree {
                id: query_id,
                disabled: false,
                kind: "label".to_string(),
                font_role: FontRole::Ui,
                text_variant: None,
                title: None,
                text: Some(session.query().to_string()),
                label: Some(session.query().to_string()),
                action_command_id: None,
                items: Vec::new(),
                children: Vec::new(),
                validation_state: None,
            },
        ];

        match session.status() {
            TransientMenuStatus::Empty { message } => {
                children.push(PackageUiComponentTree {
                    id: status_id,
                    disabled: false,
                    kind: "statusItem".to_string(),
                    font_role: FontRole::Ui,
                    text_variant: Some(UiTextVariant::Status),
                    title: None,
                    text: Some(message.clone()),
                    label: Some(message.clone()),
                    action_command_id: None,
                    items: Vec::new(),
                    children: Vec::new(),
                    validation_state: None,
                });
            }
            _ => {
                let selected_index = session.selected_index();
                let items: Vec<PackageUiListItem> = session
                    .items()
                    .iter()
                    .enumerate()
                    .map(|(index, item)| menu_item_to_list_item(index, item, selected_index))
                    .collect();
                let action_targets: Vec<String> = session
                    .items()
                    .iter()
                    .filter(|item| item.action.completion_accept.is_none())
                    .map(|item| item.action.command_id.clone())
                    .collect();
                let list = PackageUiComponentTree {
                    id: list_id,
                    disabled: false,
                    kind: "list".to_string(),
                    font_role: FontRole::Ui,
                    text_variant: None,
                    title: None,
                    text: None,
                    label: None,
                    action_command_id: None,
                    items,
                    children: Vec::new(),
                    validation_state: None,
                };
                children.push(PackageUiComponentTree {
                    id: format!("menu.{}.scroll", session.session_id().0),
                    disabled: false,
                    kind: "scroll".to_string(),
                    font_role: FontRole::Ui,
                    text_variant: None,
                    title: None,
                    text: None,
                    label: None,
                    action_command_id: None,
                    items: Vec::new(),
                    children: vec![list],
                    validation_state: None,
                });
                return Self {
                    id: format!("menu.{}", session.session_id().0),
                    anchor,
                    focus_policy: match session.focus_policy() {
                        TransientMenuFocusPolicy::Modal => "modal".to_string(),
                        TransientMenuFocusPolicy::Modeless => "modeless".to_string(),
                    },
                    dismissal_policy: "escape".to_string(),
                    component: PackageUiComponentTree {
                        id: format!("menu.{}.root", session.session_id().0),
                        disabled: false,
                        kind: "stack".to_string(),
                        font_role: FontRole::Ui,
                        text_variant: None,
                        title: Some(session.prompt().to_string()),
                        text: None,
                        label: Some(session.prompt().to_string()),
                        action_command_id: None,
                        items: Vec::new(),
                        children,
                        validation_state: None,
                    },
                    action_targets,
                    z_level_token: "z.overlay",
                    menu_a11y: Some(menu_a11y.clone()),
                };
            }
        }

        Self {
            id: format!("menu.{}", session.session_id().0),
            anchor,
            focus_policy: match session.focus_policy() {
                TransientMenuFocusPolicy::Modal => "modal".to_string(),
                TransientMenuFocusPolicy::Modeless => "modeless".to_string(),
            },
            dismissal_policy: "escape".to_string(),
            component: PackageUiComponentTree {
                id: format!("menu.{}.root", session.session_id().0),
                disabled: false,
                kind: "stack".to_string(),
                font_role: FontRole::Ui,
                text_variant: None,
                title: Some(session.prompt().to_string()),
                text: None,
                label: Some(session.prompt().to_string()),
                action_command_id: None,
                items: Vec::new(),
                children,
                validation_state: None,
            },
            action_targets: Vec::new(),
            z_level_token: "z.overlay",
            menu_a11y: Some(menu_a11y),
        }
    }
}

fn menu_item_to_list_item(
    index: usize,
    item: &TransientMenuItem,
    selected_index: usize,
) -> PackageUiListItem {
    PackageUiListItem {
        id: format!("item.{index}"),
        disabled: false,
        label: item.label.clone(),
        detail: item.detail.clone(),
        action_command_id: item
            .action
            .completion_accept
            .is_none()
            .then(|| item.action.command_id.clone()),
        selected: index == selected_index,
    }
}

impl PackagePanelVisibility {
    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "visible" => Self::Visible,
            "collapsed" => Self::Collapsed,
            _ => Self::Hidden,
        }
    }
}

impl PackageOverlayAnchor {
    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "active-pane" => Self::ActivePane,
            "main" => Self::Main,
            "pointer" => Self::Pointer,
            "bottom" => Self::Bottom,
            _ => Self::WorkingArea,
        }
    }

    pub(crate) fn rect(self, working_area: Rect, main_rect: Rect) -> Rect {
        self.rect_with_centered_width(working_area, main_rect, 640.0)
    }

    /// Resolve geometry with the cached centered-surface width. The centered
    /// anchor uses window bounds; other anchors preserve their existing
    /// pane-local geometry.
    pub(crate) fn rect_with_centered_width(
        self,
        working_area: Rect,
        main_rect: Rect,
        centered_width: f64,
    ) -> Rect {
        match self {
            Self::Main => main_rect,
            Self::Pointer => centered_rect(main_rect, 320.0, 220.0),
            Self::Bottom => bottom_rect(main_rect),
            Self::Completion => main_rect,
            Self::WorkingArea | Self::ActivePane => working_area,
            Self::Centered => centered_rect(working_area, centered_width, 220.0),
        }
    }
}

pub(crate) fn completion_overlay_rect(
    main_rect: Rect,
    caret: Option<Rect>,
    item_count: usize,
    typography: &TypographyRegistry,
    ui_theme: &ResolvedUiTheme,
) -> Rect {
    let style = SduiThemeStyle::from_ui_theme(ui_theme);
    let body = typography.ui_text_metrics(FontRole::Ui, style.body_text);
    let detail = typography.ui_text_metrics(FontRole::Ui, UiTextVariant::Detail);
    let row_height = body.list_height(detail);
    let visible_rows = item_count.clamp(1, COMPLETION_MAX_VISIBLE_ROWS);
    let padding = ui_theme.scalar_f64("spacing.panel").unwrap_or(16.0);
    let height = (padding + body.row_height * 2.0 + row_height * visible_rows as f64)
        .min(main_rect.height().max(0.0));
    let width = main_rect.width().clamp(0.0, COMPLETION_MAX_WIDTH_PX);
    let max_x = (main_rect.x1 - width).max(main_rect.x0);
    let caret = caret.unwrap_or(Rect::new(
        main_rect.x0,
        main_rect.y0,
        main_rect.x0,
        main_rect.y0,
    ));
    let x = caret.x0.clamp(main_rect.x0, max_x);
    let gap = ui_theme.scalar_f64("spacing.inline").unwrap_or(6.0);
    let y = if height >= main_rect.height().max(0.0) {
        main_rect.y0
    } else {
        let below = caret.y1 + gap;
        let above = caret.y0 - gap - height;
        if below + height <= main_rect.y1 {
            below
        } else if above >= main_rect.y0 {
            above
        } else {
            below.clamp(main_rect.y0, main_rect.y1 - height)
        }
    };
    Rect::new(x, y, x + width, y + height)
}

impl PackageUiComponentTree {
    pub(crate) fn from_declaration(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| "package UI component must be an object".to_string())?;
        Self::from_object(object)
    }

    fn from_object(object: &Map<String, Value>) -> Result<Self, String> {
        let id = required_text(object, "id")?.to_string();
        let kind = required_text(object, "kind")?.to_string();
        let font_role = object
            .get("style")
            .and_then(Value::as_object)
            .and_then(|style| style.get("fontRole"))
            .and_then(Value::as_str)
            .and_then(FontRole::from_name)
            .unwrap_or(FontRole::Ui);
        let text_variant = object
            .get("style")
            .and_then(Value::as_object)
            .and_then(|style| style.get("typography"))
            .and_then(Value::as_str)
            .map(UiTextVariant::from_typography_token);
        let title = optional_text(object, "title").map(ToOwned::to_owned);
        let text = optional_text(object, "text").map(ToOwned::to_owned);
        let label = optional_text(object, "label").map(ToOwned::to_owned);
        let action_command_id = object
            .get("action")
            .and_then(Value::as_object)
            .and_then(|action| optional_text(action, "commandId"))
            .map(ToOwned::to_owned);
        let items = object
            .get("items")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(PackageUiListItem::from_value)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        let children = object
            .get("children")
            .and_then(Value::as_array)
            .map(|children| {
                children
                    .iter()
                    .map(Self::from_declaration)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        let disabled = object
            .get("disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let validation_state = object
            .get("style")
            .and_then(Value::as_object)
            .and_then(|style| style.get("validationState"))
            .and_then(Value::as_str)
            .filter(|s| matches!(*s, "none" | "error" | "warning" | "success"))
            .map(ToOwned::to_owned);
        Ok(Self {
            id,
            kind,
            font_role,
            text_variant,
            title,
            text,
            label,
            action_command_id,
            items,
            children,
            disabled,
            validation_state,
        })
    }
}

impl PackageUiListItem {
    pub(crate) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        action_command_id: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            action_command_id,
            selected: false,
            disabled: false,
        }
    }

    pub(crate) fn with_selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn from_value(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| "package UI list items must be objects".to_string())?;
        Ok(Self {
            id: required_text(object, "id")?.to_string(),
            label: required_text(object, "label")?.to_string(),
            detail: optional_text(object, "detail").map(ToOwned::to_owned),
            action_command_id: object
                .get("action")
                .and_then(Value::as_object)
                .and_then(|action| optional_text(action, "commandId"))
                .map(ToOwned::to_owned),
            selected: object
                .get("selected")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            disabled: object
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }
}

fn required_text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    optional_text(object, key).ok_or_else(|| format!("package UI component `{key}` must be text"))
}

fn optional_text<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

pub(crate) fn centered_rect(bounds: Rect, width: f64, height: f64) -> Rect {
    let width = width.min(bounds.width()).max(0.0);
    let height = height.min(bounds.height()).max(0.0);
    let x0 = bounds.x0 + (bounds.width() - width) / 2.0;
    let y0 = bounds.y0 + (bounds.height() - height) / 2.0;
    Rect::new(x0, y0, x0 + width, y0 + height)
}

fn bottom_rect(main_rect: Rect) -> Rect {
    let available = main_rect.height().max(0.0);
    let height = if available < 120.0 {
        available
    } else {
        (available * 0.35).clamp(120.0, 240.0).min(available)
    };
    Rect::new(
        main_rect.x0,
        main_rect.y1 - height,
        main_rect.x1,
        main_rect.y1,
    )
}

#[cfg(test)]
mod tests {
    use masonry::kurbo::Rect;
    use serde_json::json;

    use super::*;
    use crate::shell::transient_menu::{
        TransientMenuAction, TransientMenuItem, TransientMenuOrigin, TransientMenuSession,
        TransientMenuSessionId,
    };

    fn component(id: &str) -> PackageUiComponentTree {
        PackageUiComponentTree::from_declaration(&json!({
            "kind": "panel",
            "id": id,
            "title": "Preview",
            "children": [{
                "kind": "button",
                "id": format!("{id}.toggle"),
                "label": "Toggle",
                "action": { "commandId": "markdown.togglePreview" }
            }]
        }))
        .unwrap()
    }

    fn find_list(tree: &PackageUiComponentTree) -> Option<&PackageUiComponentTree> {
        if tree.kind == "list" {
            return Some(tree);
        }
        tree.children.iter().find_map(find_list)
    }

    #[test]
    fn package_components_default_to_ui_typography() {
        let component = PackageUiComponentTree::from_declaration(&json!({
            "kind": "panel",
            "id": "markdown.preview.root",
            "title": "Preview",
            "children": [{
                "kind": "label",
                "id": "markdown.preview.label",
                "text": "Ready"
            }]
        }))
        .unwrap();

        assert_eq!(component.font_role, FontRole::Ui);
        assert_eq!(component.children[0].font_role, FontRole::Ui);
        assert_eq!(component.children[0].text_variant, None);
    }

    #[test]
    fn slot_panel_contribution_places_panel_in_requested_slot_and_preserves_main_editor() {
        let mut runtime = PackageUiRuntimeState::new();
        runtime
            .apply_update(PackageUiRuntimeUpdate {
                base_version: 0,
                fixed_panels: vec![FixedPackagePanel::new(
                    "markdown.preview",
                    FixedSlotId::Right,
                    PackagePanelVisibility::Visible,
                    component("markdown.preview.root"),
                    vec!["markdown.togglePreview".to_string()],
                )],
                transient_overlays: Vec::new(),
                input_routing: Vec::new(),
            })
            .unwrap();

        let defaults = crate::shell::theme::ResolvedUiTheme::default().panel_defaults();
        let geometry = runtime
            .slot_layout(&defaults)
            .compute_geometry(Rect::new(0.0, 0.0, 900.0, 600.0));
        assert_eq!(geometry.main_rect, Rect::new(0.0, 0.0, 660.0, 600.0));
        assert_eq!(geometry.fixed_slots[0].slot_id, FixedSlotId::Right);
        assert_eq!(
            geometry.fixed_slots[0].rect,
            Rect::new(660.0, 0.0, 900.0, 600.0)
        );
    }

    #[test]
    fn slot_panel_contribution_rejects_duplicate_exclusive_slot_claims() {
        let mut runtime = PackageUiRuntimeState::new();
        let error = runtime
            .apply_update(PackageUiRuntimeUpdate {
                base_version: 0,
                fixed_panels: vec![
                    FixedPackagePanel::new(
                        "markdown.preview",
                        FixedSlotId::Left,
                        PackagePanelVisibility::Visible,
                        component("markdown.preview.root"),
                        Vec::new(),
                    ),
                    FixedPackagePanel::new(
                        "outline.preview",
                        FixedSlotId::Left,
                        PackagePanelVisibility::Visible,
                        component("outline.preview.root"),
                        Vec::new(),
                    ),
                ],
                transient_overlays: Vec::new(),
                input_routing: Vec::new(),
            })
            .unwrap_err();
        assert_eq!(
            error,
            PackageUiRuntimeError::DuplicateFixedSlot {
                slot_id: FixedSlotId::Left
            }
        );
    }

    #[test]
    fn transient_overlay_renders_without_consuming_fixed_slot_geometry() {
        let mut with_overlay = PackageUiRuntimeState::new();
        with_overlay
            .apply_update(PackageUiRuntimeUpdate {
                base_version: 0,
                fixed_panels: vec![FixedPackagePanel::new(
                    "markdown.preview",
                    FixedSlotId::Bottom,
                    PackagePanelVisibility::Visible,
                    component("markdown.preview.root"),
                    Vec::new(),
                )],
                transient_overlays: vec![TransientPackageOverlay::new(
                    "markdown.preview.quickOpen",
                    PackageOverlayAnchor::Main,
                    "restore",
                    "escape",
                    component("markdown.preview.quickOpen.root"),
                    Vec::new(),
                    "z.overlay",
                )],
                input_routing: Vec::new(),
            })
            .unwrap();

        let defaults = crate::shell::theme::ResolvedUiTheme::default().panel_defaults();
        let geometry = with_overlay
            .slot_layout(&defaults)
            .compute_geometry(Rect::new(0.0, 0.0, 900.0, 600.0));
        let overlay =
            with_overlay.overlay_observations(Rect::new(0.0, 0.0, 900.0, 600.0), &defaults);
        assert_eq!(geometry.main_rect, Rect::new(0.0, 0.0, 900.0, 480.0));
        assert_eq!(overlay[0].rect, geometry.main_rect);
    }

    #[test]
    fn menu_session_projects_to_bottom_transient_overlay() {
        use crate::shell::transient_menu::TransientMenuAction;

        let session = TransientMenuSession::new(TransientMenuSessionId(7), "Control Center")
            .with_items(vec![
                TransientMenuItem::new(
                    "cmd.a",
                    "Alpha Command",
                    TransientMenuAction::new("clay.alpha"),
                )
                .with_detail("does alpha"),
                TransientMenuItem::new(
                    "cmd.b",
                    "Beta Command",
                    TransientMenuAction::new("clay.beta"),
                ),
            ]);
        let overlay = TransientPackageOverlay::from_menu_session(&session);

        assert_eq!(overlay.id, "menu.7");
        assert_eq!(overlay.anchor, PackageOverlayAnchor::Bottom);
        assert_eq!(overlay.focus_policy, "modal");
        assert_eq!(overlay.dismissal_policy, "escape");
        assert_eq!(overlay.component.kind, "stack");
        assert_eq!(overlay.action_targets, vec!["clay.alpha", "clay.beta"]);

        let list_component = find_list(&overlay.component).expect("menu overlay contains list");
        assert_eq!(list_component.items.len(), 2);
        assert_eq!(list_component.items[0].label, "Alpha Command");
        assert_eq!(
            list_component.items[0].detail.as_deref(),
            Some("does alpha")
        );
        assert_eq!(
            list_component.items[0].action_command_id,
            Some("clay.alpha".to_string())
        );
        assert!(list_component.items[0].selected);
        assert!(!list_component.items[1].selected);
    }

    #[test]
    fn centered_menu_projection_uses_window_geometry_and_stays_internal() {
        let session = TransientMenuSession::new(TransientMenuSessionId(8), "Control Center")
            .with_origin(TransientMenuOrigin::Centered);
        let overlay = TransientPackageOverlay::from_menu_session(&session);
        assert_eq!(overlay.anchor, PackageOverlayAnchor::Centered);
        assert_eq!(
            PackageOverlayAnchor::parse("centered"),
            PackageOverlayAnchor::WorkingArea,
            "package parsing cannot request the internal centered anchor"
        );

        let window = Rect::new(0.0, 0.0, 900.0, 600.0);
        assert_eq!(
            overlay
                .anchor
                .rect_with_centered_width(window, Rect::ZERO, 640.0),
            Rect::new(130.0, 190.0, 770.0, 410.0)
        );
        assert_eq!(
            overlay.anchor.rect_with_centered_width(
                Rect::new(0.0, 0.0, 300.0, 200.0),
                Rect::ZERO,
                640.0
            ),
            Rect::new(0.0, 0.0, 300.0, 200.0)
        );
    }

    #[test]
    fn bottom_overlay_stays_inside_short_main_regions() {
        for height in [48.0, 119.0, 120.0, 200.0] {
            let main = Rect::new(0.0, 10.0, 300.0, 10.0 + height);
            let overlay = bottom_rect(main);
            assert!(overlay.x0 >= main.x0 && overlay.x1 <= main.x1);
            assert!(overlay.y0 >= main.y0 && overlay.y1 <= main.y1);
            assert!(overlay.height() <= main.height());
        }
        assert_eq!(bottom_rect(Rect::new(0.0, 0.0, 300.0, 80.0)).height(), 80.0);
    }

    #[test]
    fn bottom_menu_overlay_does_not_consume_fixed_slot_geometry() {
        let session = TransientMenuSession::new(TransientMenuSessionId(8), "Control Center")
            .with_items(vec![TransientMenuItem::new(
                "cmd.a",
                "Alpha",
                TransientMenuAction::new("clay.alpha"),
            )]);
        let overlay = TransientPackageOverlay::from_menu_session(&session);

        let mut runtime = PackageUiRuntimeState::new();
        runtime
            .apply_update(PackageUiRuntimeUpdate {
                base_version: 0,
                fixed_panels: vec![FixedPackagePanel::new(
                    "markdown.preview",
                    FixedSlotId::Bottom,
                    PackagePanelVisibility::Visible,
                    component("markdown.preview.root"),
                    Vec::new(),
                )],
                transient_overlays: vec![overlay],
                input_routing: Vec::new(),
            })
            .unwrap();

        let defaults = crate::shell::theme::ResolvedUiTheme::default().panel_defaults();
        let geometry = runtime
            .slot_layout(&defaults)
            .compute_geometry(Rect::new(0.0, 0.0, 900.0, 600.0));
        // Bottom fixed panel consumes the fixed slot; the transient menu overlay
        // is projected inside the remaining main rect and does not alter it.
        assert_eq!(geometry.main_rect, Rect::new(0.0, 0.0, 900.0, 480.0));

        let overlay_rect =
            runtime.overlay_observations(Rect::new(0.0, 0.0, 900.0, 600.0), &defaults)[0].rect;
        assert!(overlay_rect.y0 >= geometry.main_rect.y0);
        assert_eq!(overlay_rect.y1, geometry.main_rect.y1);
        assert_eq!(overlay_rect.x0, geometry.main_rect.x0);
        assert_eq!(overlay_rect.x1, geometry.main_rect.x1);
        assert!(overlay_rect.height() <= 240.0);
    }

    #[test]
    fn completion_menu_projection_has_no_command_action_targets() {
        let result = crate::protocol::CompletionResultSet {
            request_id: 10,
            client_id: 1,
            document_id: 2,
            document_version: 3,
            behavior_version: 4,
            provider_generation: 1,
            replacement_range: crate::protocol::CompletionReplacementRange::new(0, 1),
            status: crate::protocol::CompletionStatus::Ok,
            items: vec![crate::protocol::CompletionItem::new(
                "alpha",
                "alpha",
                crate::protocol::CompletionProvenance::builtin_core(),
            )],
            provenance: crate::protocol::CompletionProvenance::builtin_core(),
        };
        let session = crate::shell::completion_result_to_menu_session(&result);
        let overlay = TransientPackageOverlay::from_menu_session(&session);

        assert!(overlay.action_targets.is_empty());
        assert_eq!(overlay.anchor, PackageOverlayAnchor::Completion);
        assert!(
            overlay
                .component
                .children
                .iter()
                .any(|child| child.kind == "scroll")
        );
        let list_component =
            find_list(&overlay.component).expect("completion menu overlay contains list");
        assert_eq!(list_component.items[0].label, "alpha");
        assert!(list_component.items[0].action_command_id.is_none());
        assert!(list_component.items[0].selected);
    }

    #[test]
    fn completion_overlay_clamps_above_or_below_caret_inside_main_rect() {
        let typography = TypographyRegistry::default();
        let theme = ResolvedUiTheme::default();
        let main = Rect::new(0.0, 20.0, 900.0, 820.0);
        let below = completion_overlay_rect(
            main,
            Some(Rect::new(180.0, 100.0, 181.0, 120.0)),
            COMPLETION_MAX_VISIBLE_ROWS + 4,
            &typography,
            &theme,
        );
        assert!(below.x0 >= main.x0 && below.x1 <= main.x1);
        assert_eq!(below.width(), COMPLETION_MAX_WIDTH_PX);
        assert!(below.y0 >= 120.0 && below.y1 <= main.y1);

        let above = completion_overlay_rect(
            main,
            Some(Rect::new(180.0, 760.0, 181.0, 780.0)),
            COMPLETION_MAX_VISIBLE_ROWS + 4,
            &typography,
            &theme,
        );
        assert!(above.x0 >= main.x0 && above.x1 <= main.x1);
        assert!(above.y0 >= main.y0 && above.y1 <= 760.0);
    }

    #[test]
    fn completion_overlay_height_uses_visible_row_cap() {
        let typography = TypographyRegistry::default();
        let theme = ResolvedUiTheme::default();
        let main = Rect::new(0.0, 0.0, 900.0, 600.0);
        let caret = Some(Rect::new(120.0, 100.0, 121.0, 120.0));
        let one = completion_overlay_rect(main, caret, 1, &typography, &theme);
        let eight = completion_overlay_rect(
            main,
            caret,
            COMPLETION_MAX_VISIBLE_ROWS,
            &typography,
            &theme,
        );
        let many = completion_overlay_rect(main, caret, usize::MAX, &typography, &theme);

        assert!(one.height() < eight.height());
        assert_eq!(many.height(), eight.height());
        assert!(many.height() <= main.height());
        assert_eq!(many.width(), COMPLETION_MAX_WIDTH_PX);
    }

    #[test]
    fn empty_menu_session_shows_status_without_action_targets() {
        let session = TransientMenuSession::new(TransientMenuSessionId(9), "Control Center");
        let overlay = TransientPackageOverlay::from_menu_session(&session);

        assert!(overlay.action_targets.is_empty());
        let status_component = overlay
            .component
            .children
            .iter()
            .find(|child| child.kind == "statusItem")
            .expect("empty menu overlay contains status component");
        assert_eq!(status_component.text.as_deref(), Some("No results"));
    }
}
