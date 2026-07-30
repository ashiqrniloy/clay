pub(crate) mod components;
pub(crate) mod file_browser;
pub(crate) mod layout;
pub(crate) mod layout_persist;
pub(crate) mod package_ui;
pub(crate) mod primitives;
pub(crate) mod theme;
pub(crate) mod transient_menu;

pub(crate) use layout::{
    FixedSlotId, FixedSlotState, PaneSlotLayout, ShellComponentKind, SlotDragState, SplitDragState,
    SplitOrientation, SplitRatio, WorkingAreaLayout, WorkingAreaLayoutObservation,
    WorkingAreaLayoutUpdate, WorkingAreaLayoutUpdateError, compute_drag_ratio,
    compute_slot_resize_size, hit_test_slot_handle, hit_test_split_divider, slot_handle_rect,
};
pub(crate) use package_ui::{
    FixedPackagePanel, PackageInputRouting, PackageOverlayAnchor, PackagePanelVisibility,
    PackageUiComponentTree, PackageUiOverlayObservation, PackageUiPanelObservation,
    PackageUiRuntimeError, PackageUiRuntimeState, PackageUiRuntimeUpdate, TransientPackageOverlay,
};
pub(crate) use primitives::{
    Axis, InteractionState, PanelChrome, component_state_color, disabled_text_color,
    list_row_fill_color, paint_divider, paint_focus_ring, paint_panel_chrome,
};
pub(crate) use theme::ResolvedUiTheme;
pub(crate) use transient_menu::TransientMenuSession;
pub(crate) use transient_menu::{
    CompletionMenuAcceptAction, TransientMenuAction, TransientMenuItem, TransientMenuSessionId,
    completion_result_to_menu_session, language_intelligence_result_to_menu_session,
};

#[cfg(test)]
pub(crate) use layout::{
    PaneId, PaneSlotId, PaneSlotLayoutAssignment, PaneSplitNode, PaneSplitTree,
    PaneTreeObservation, ShellComponentId, ShellLayoutVersion, WorkingAreaId,
};
