pub(crate) mod components;
pub(crate) mod file_browser;
pub(crate) mod layout;
pub(crate) mod layout_persist;
pub(crate) mod package_ui;
pub(crate) mod primitives;
pub(crate) mod theme;
#[doc(hidden)]
pub mod transient_menu;

pub use layout::PaneId;
pub(crate) use layout::{
    FixedSlotId, FixedSlotState, KEYBOARD_RESIZE_STEP, PaneResizeDirection, PaneSlotLayout,
    PaneSplitTree, SlotDragState, SplitChild, SplitDragState, SplitOrientation, SplitRatio,
    WorkingAreaLayout, compute_drag_ratio, compute_slot_resize_size, hit_test_slot_handle,
    hit_test_split_divider, slot_handle_rect,
};
#[doc(hidden)]
pub use layout_persist::{
    PersistedTabLayout, PersistedTabState, PersistedWindowState, load_window_state,
    save_window_state,
};
pub(crate) use package_ui::{
    FixedPackagePanel, PackageInputRouting, PackageOverlayAnchor, PackagePanelVisibility,
    PackageUiComponentTree, PackageUiRuntimeError, PackageUiRuntimeState, PackageUiRuntimeUpdate,
    TransientPackageOverlay,
};
#[cfg(test)]
pub(crate) use package_ui::{PackageUiOverlayObservation, PackageUiPanelObservation};
pub(crate) use primitives::{
    Axis, InteractionState, PanelChrome, component_state_color, disabled_text_color,
    list_row_fill_color, paint_divider, paint_focus_ring, paint_panel_chrome, tab_card_chrome,
};
pub(crate) use theme::ResolvedUiTheme;
pub use transient_menu::TransientMenuSession;
pub(crate) use transient_menu::{
    CompletionMenuAcceptAction, TransientMenuAction, TransientMenuItem, TransientMenuSessionId,
    completion_result_to_menu_session, language_intelligence_result_to_menu_session,
};

/// Phase 22.4: the driver-owned tab-close confirm menu (Save all and close /
/// Discard and close / Cancel). Built here so the app driver can host the
/// session without widening the transient-menu item/action types. Every
/// action carries `clientId`, which the pane view reads to hand the selection
/// back to the driver (`EditorAction::TabCloseMenuAction`); the action IDs
/// are driver-local orchestration — never declared commands, never
/// server-routed — so tab-confirm and per-view save-conflict sessions cannot
/// cross-route.
pub fn tab_close_confirm_session(
    session_id: u64,
    prompt: String,
    client_id: u64,
) -> TransientMenuSession {
    let arguments = serde_json::json!({ "clientId": client_id });
    let items = vec![
        TransientMenuItem::new(
            "tabclose.saveall",
            "Save all and close",
            TransientMenuAction::new("clay.shell.clientTabCloseSaveAll")
                .with_arguments(arguments.clone()),
        )
        .with_accessibility_label("Save all unsaved documents in this tab, then close the tab"),
        TransientMenuItem::new(
            "tabclose.discard",
            "Discard and close",
            TransientMenuAction::new("clay.shell.clientTabCloseDiscard")
                .with_arguments(arguments.clone()),
        )
        .with_accessibility_label("Discard all unsaved edits in this tab, then close the tab"),
        TransientMenuItem::new(
            "tabclose.cancel",
            "Cancel",
            TransientMenuAction::new("clay.shell.clientTabCloseCancel").with_arguments(arguments),
        )
        .with_accessibility_label("Keep the tab open and cancel closing"),
    ];
    TransientMenuSession::new(TransientMenuSessionId(session_id), prompt).with_items(items)
}

#[cfg(test)]
pub(crate) use layout::{
    PaneSlotId, PaneSlotLayoutAssignment, PaneSplitNode, PaneTreeObservation, ShellComponentId,
    ShellComponentKind, ShellLayoutVersion, WorkingAreaId, WorkingAreaLayoutObservation,
    WorkingAreaLayoutUpdate, WorkingAreaLayoutUpdateError,
};
