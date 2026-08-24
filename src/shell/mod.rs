//! Renderer-neutral shell models used by server validation and Tauri state projection.

pub(crate) mod components;
pub(crate) mod file_browser;
pub(crate) mod fuzzy;
pub(crate) mod layout;
pub(crate) mod layout_persist;
pub(crate) mod package_ui;
pub(crate) mod path_browser;
pub mod theme;
#[doc(hidden)]
pub mod transient_menu;

pub(crate) use layout::FixedSlotId;
pub use layout::PaneId;
#[doc(hidden)]
pub use layout_persist::{
    PersistedTabLayout, PersistedTabState, PersistedWindowState, load_window_state,
    load_window_state_json, parse_window_state_json, save_window_state,
    save_window_state_from_json,
};
pub(crate) use package_ui::{
    FixedPackagePanel, PackageInputRouting, PackageOverlayAnchor, PackagePanelVisibility,
    PackageUiComponentTree, PackageUiRuntimeError, PackageUiRuntimeState, PackageUiRuntimeUpdate,
    TransientPackageOverlay,
};
pub use transient_menu::TransientMenuSession;
