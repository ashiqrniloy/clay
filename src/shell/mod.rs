pub(crate) mod components;
pub(crate) mod file_browser;
pub(crate) mod layout;
pub(crate) mod package_ui;
pub(crate) mod theme;
pub(crate) mod transient_menu;

pub(crate) use layout::{
    FixedSlotId, FixedSlotState, PaneSlotLayout, ShellComponentKind, WorkingAreaLayout,
    WorkingAreaLayoutObservation, WorkingAreaLayoutUpdate, WorkingAreaLayoutUpdateError,
};
pub(crate) use package_ui::{
    FixedPackagePanel, PackageInputRouting, PackageOverlayAnchor, PackagePanelVisibility,
    PackageUiComponentTree, PackageUiOverlayObservation, PackageUiPanelObservation,
    PackageUiRuntimeError, PackageUiRuntimeState, PackageUiRuntimeUpdate, TransientPackageOverlay,
};
pub(crate) use transient_menu::TransientMenuSession;
pub(crate) use transient_menu::{
    CompletionMenuAcceptAction, TransientMenuStatus, completion_result_to_menu_session,
    language_intelligence_result_to_menu_session,
};

#[cfg(test)]
pub(crate) use layout::{
    PaneId, PaneSlotId, PaneSlotLayoutAssignment, PaneSplitNode, PaneSplitTree,
    PaneTreeObservation, ShellComponentId, ShellLayoutVersion, SplitOrientation, SplitRatio,
    WorkingAreaId,
};
