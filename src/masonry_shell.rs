use std::collections::{BTreeMap, BTreeSet};

use crate::perf::budgets::TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS;
use masonry::accesskit::{Live, Node, NodeId, Role};
use masonry::core::keyboard::{Key, KeyState, NamedKey};
use masonry::core::{
    AccessCtx, BoxConstraints, ChildrenIds, EventCtx, LayoutCtx, MutateCtx, NewWidget, PaintCtx,
    PointerButton, PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, TextEvent, Update,
    UpdateCtx, Widget, WidgetId, WidgetPod,
};
use masonry::kurbo::{Point, Rect, Size};
use masonry::vello::Scene;

use crate::editor::typography::{TypographyRegistry, UiTextVariant};
use crate::masonry_editor::{EditorAction, EditorWidget};
use crate::masonry_pane_host::PaneContentHost;
use crate::masonry_sdui::paint_sdui_text;
use crate::protocol::{ClientId, FontRole};
use crate::shell::{
    Axis, FixedSlotId, InteractionState, KEYBOARD_RESIZE_STEP, PaneId, PaneResizeDirection,
    PaneSplitTree, PanelChrome, PersistedTabLayout, PersistedTabState, ResolvedUiTheme,
    SlotDragState, SplitChild, SplitDragState, SplitOrientation, SplitRatio, WorkingAreaLayout,
    compute_slot_resize_size, hit_test_slot_handle, hit_test_split_divider, paint_divider,
    paint_focus_ring, paint_panel_chrome, slot_handle_rect,
};

// Doc-hidden pass-through for the native `clay` binary's menu-sync routing.
pub use crate::shell::TransientMenuSession;

#[cfg(test)]
use crate::shell::{
    FixedSlotState, PaneSlotId, PaneSlotLayout, PaneSlotLayoutAssignment, PaneSplitNode,
    PaneTreeObservation, ShellComponentId, ShellComponentKind, ShellLayoutVersion, WorkingAreaId,
    WorkingAreaLayoutObservation, WorkingAreaLayoutUpdate, WorkingAreaLayoutUpdateError,
};

/// Internal structural shell snapshot for tests and agent inspection.
///
/// The snapshot deliberately omits Masonry/native widget IDs, document text,
/// source snippets, raw action payload authority, raw filesystem paths, raw CSS,
/// raw ops, and executable package code. Public shell APIs must be introduced
/// separately through Clay JS facade/op/reference-doc coverage.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ShellObservableSnapshot {
    pub(crate) layout: WorkingAreaLayoutObservation,
    pub(crate) editor_component_bound: bool,
    pub(crate) sdui_state_present: bool,
    pub(crate) status_present: bool,
}

/// Clay-owned native shell root for a window working area.
///
/// The shell is a Masonry container that keeps the editor as a child component
/// instead of making `EditorWidget` the top-level application layout. It is an
/// internal native implementation detail; packages never receive Masonry widget
/// IDs, widget handles, raw callbacks, or layout mutation authority. The type is
/// Rust-public only so the package's `clay` binary target can construct the
/// library-owned widget; it is not a Clay JS API and has no facade/op/registry
/// entry.
#[doc(hidden)]
/// Phase 22.1: how panes are activated by the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneFocusPolicy {
    /// A pointer-down inside an inactive pane activates it.
    #[default]
    ClickToFocus,
    /// Pointer motion over a pane activates it.
    FollowsCursor,
}

impl PaneFocusPolicy {
    /// Map a configuration string (`"click"` or `"cursor"`) to the enum.
    /// Unknown values fall back to the default (`ClickToFocus`).
    pub fn from_config_str(value: &str) -> Self {
        match value {
            "cursor" => Self::FollowsCursor,
            _ => Self::ClickToFocus,
        }
    }
}

/// Phase 22.1: client-routed shell pane-management commands.
///
/// Mapped from `shell.client*` command IDs by [`Self::from_command_id`].
/// Vim-style naming: "vertical" = side by side (vsplit), "horizontal" = stacked.
/// Phase 22.4 adds tab management: `TabActivate(u32)`/`TabMoveTo(u32)` carry
/// the 1-based tab position from the numbered `clientTabActivate.N`/
/// `clientTabMoveTo.N` command IDs (N in 1..=9 only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellClientCommand {
    SplitPaneVertical,
    SplitPaneHorizontal,
    AddEqualPane,
    ClosePane,
    FocusPaneNext,
    FocusPanePrev,
    ResizePaneLeft,
    ResizePaneRight,
    ResizePaneUp,
    ResizePaneDown,
    MovePaneNext,
    MovePanePrev,
    TabNext,
    TabPrev,
    TabNew,
    TabClose,
    TabMoveLeft,
    TabMoveRight,
    /// 1-based position in the current tab order (card order).
    TabActivate(u32),
    /// 1-based target position in the current tab order (card order).
    TabMoveTo(u32),
}

impl ShellClientCommand {
    /// Maps an allowlisted `shell.client*` command ID to its shell command.
    /// `None` for IDs outside the allowlisted surface.
    pub fn from_command_id(command_id: &str) -> Option<Self> {
        match command_id {
            "shell.clientSplitPaneVertical" => Some(Self::SplitPaneVertical),
            "shell.clientSplitPaneHorizontal" => Some(Self::SplitPaneHorizontal),
            // Phase 22.7 (F3): direction-named aliases for the Vim-style
            // canonical IDs ("vertical" = side by side). The canonical names
            // stay for backwards compatibility with existing configs and
            // docs; the aliases add the direction vocabulary without
            // duplicating handlers.
            "shell.clientSplitPaneRight" => Some(Self::SplitPaneVertical), // alias: new pane beside
            "shell.clientSplitPaneDown" => Some(Self::SplitPaneHorizontal), // alias: new pane below
            "shell.clientAddEqualPane" => Some(Self::AddEqualPane),
            "shell.clientClosePane" => Some(Self::ClosePane),
            "shell.clientFocusPaneNext" => Some(Self::FocusPaneNext),
            "shell.clientFocusPanePrev" => Some(Self::FocusPanePrev),
            "shell.clientResizePaneLeft" => Some(Self::ResizePaneLeft),
            "shell.clientResizePaneRight" => Some(Self::ResizePaneRight),
            "shell.clientResizePaneUp" => Some(Self::ResizePaneUp),
            "shell.clientResizePaneDown" => Some(Self::ResizePaneDown),
            "shell.clientMovePaneNext" => Some(Self::MovePaneNext),
            "shell.clientMovePanePrev" => Some(Self::MovePanePrev),
            // Phase 22.4: tab management. Numbered families parse N in 1..=9
            // only (the command surface declares no "beyond 9" IDs).
            "shell.clientTabNext" => Some(Self::TabNext),
            "shell.clientTabPrev" => Some(Self::TabPrev),
            "shell.clientTabNew" => Some(Self::TabNew),
            "shell.clientTabClose" => Some(Self::TabClose),
            "shell.clientTabMoveLeft" => Some(Self::TabMoveLeft),
            "shell.clientTabMoveRight" => Some(Self::TabMoveRight),
            command_id
                if let Some(n) = command_id
                    .strip_prefix("shell.clientTabActivate.")
                    .and_then(|suffix| suffix.parse::<u32>().ok())
                    .filter(|n| (1..=9).contains(n)) =>
            {
                Some(Self::TabActivate(n))
            }
            command_id
                if let Some(n) = command_id
                    .strip_prefix("shell.clientTabMoveTo.")
                    .and_then(|suffix| suffix.parse::<u32>().ok())
                    .filter(|n| (1..=9).contains(n)) =>
            {
                Some(Self::TabMoveTo(n))
            }
            _ => None,
        }
    }
}

/// Complete server-visible shell command surface. Each ID is checked by the
/// client parser before execution; aliases remain listed so the menu mirrors
/// the same deny-by-default surface as keybinding validation.
pub(crate) const SHELL_CLIENT_COMMAND_CATALOGUE: &[(&str, &str)] = &[
    ("shell.clientSplitPaneVertical", "Split Pane Vertical"),
    ("shell.clientSplitPaneHorizontal", "Split Pane Horizontal"),
    ("shell.clientSplitPaneRight", "Split Pane Right"),
    ("shell.clientSplitPaneDown", "Split Pane Down"),
    ("shell.clientAddEqualPane", "Add Equal Pane"),
    ("shell.clientClosePane", "Close Pane"),
    ("shell.clientFocusPaneNext", "Focus Next Pane"),
    ("shell.clientFocusPanePrev", "Focus Previous Pane"),
    ("shell.clientResizePaneLeft", "Resize Pane Left"),
    ("shell.clientResizePaneRight", "Resize Pane Right"),
    ("shell.clientResizePaneUp", "Resize Pane Up"),
    ("shell.clientResizePaneDown", "Resize Pane Down"),
    ("shell.clientMovePaneNext", "Move Pane Next"),
    ("shell.clientMovePanePrev", "Move Pane Previous"),
    ("shell.clientTabNext", "Next Tab"),
    ("shell.clientTabPrev", "Previous Tab"),
    ("shell.clientTabNew", "New Tab"),
    ("shell.clientTabClose", "Close Tab"),
    ("shell.clientTabMoveLeft", "Move Tab Left"),
    ("shell.clientTabMoveRight", "Move Tab Right"),
    ("shell.clientTabActivate.1", "Activate Tab 1"),
    ("shell.clientTabActivate.2", "Activate Tab 2"),
    ("shell.clientTabActivate.3", "Activate Tab 3"),
    ("shell.clientTabActivate.4", "Activate Tab 4"),
    ("shell.clientTabActivate.5", "Activate Tab 5"),
    ("shell.clientTabActivate.6", "Activate Tab 6"),
    ("shell.clientTabActivate.7", "Activate Tab 7"),
    ("shell.clientTabActivate.8", "Activate Tab 8"),
    ("shell.clientTabActivate.9", "Activate Tab 9"),
    ("shell.clientTabMoveTo.1", "Move Tab to Position 1"),
    ("shell.clientTabMoveTo.2", "Move Tab to Position 2"),
    ("shell.clientTabMoveTo.3", "Move Tab to Position 3"),
    ("shell.clientTabMoveTo.4", "Move Tab to Position 4"),
    ("shell.clientTabMoveTo.5", "Move Tab to Position 5"),
    ("shell.clientTabMoveTo.6", "Move Tab to Position 6"),
    ("shell.clientTabMoveTo.7", "Move Tab to Position 7"),
    ("shell.clientTabMoveTo.8", "Move Tab to Position 8"),
    ("shell.clientTabMoveTo.9", "Move Tab to Position 9"),
];

/// Phase 22.3: tab bar card chrome is token-driven (Phase 20.4 state tokens:
/// `surface.list`/`surface.selected` rests, `surface.hover`, `surface.active`,
/// `accent.primary` focus ring, `surface.disabled` × `opacity.disabled`).
/// The bar is a shell-owned window-level row above the working area; it never
/// consumes a package-contributable fixed slot.
pub(crate) const TAB_BAR_HEIGHT: f64 = 30.0;
pub(crate) const TAB_BAR_CARD_WIDTH: f64 = 180.0;
/// Phase 22.7 (D6/F5): floor on card width; once the strip cannot hold all
/// cards at this width, cards stop shrinking and the strip scrolls instead.
pub(crate) const TAB_BAR_CARD_MIN_WIDTH: f64 = 100.0;
pub(crate) const TAB_BAR_CARD_GAP: f64 = 4.0;
pub(crate) const TAB_BAR_CARD_PADDING: f64 = 8.0;
pub(crate) const TAB_BAR_CLOSE_SIZE: f64 = 14.0;
pub(crate) const TAB_BAR_NEW_TAB_SIZE: f64 = 28.0;
pub(crate) const TAB_BAR_NEW_TAB_GAP: f64 = 4.0;
/// Phase 22.7: wheel line-delta multiplier for the tab strip scroll.
pub(crate) const TAB_BAR_SCROLL_STEP: f64 = 24.0;

/// Phase 22.3: one tab bar card, pushed by the app driver from the
/// server-authoritative registry snapshot (names/order) plus mounted tabs
/// awaiting their registry entry (transient, close disabled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabCard {
    /// The mounted tab's connection `ClientId` (the shell's tab key).
    pub client_id: ClientId,
    /// Workspace display name for the card label (root display path's final
    /// segment).
    pub name: String,
    /// False while the tab has no server-assigned `TabId` yet (its registry
    /// entry has not arrived): the close button renders disabled and clicks
    /// are no-ops.
    pub closable: bool,
}

/// Phase 22.3: computed tab bar geometry for one window size.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TabBarGeometry {
    pub(crate) bar: Rect,
    pub(crate) cards: Vec<TabCardGeometry>,
    /// The "new tab" affordance slot at the bar's right edge (present while
    /// the bar is visible). Cards clamp before it.
    pub(crate) new_tab_rect: Rect,
    /// Phase 22.7: the effective strip scroll used to shift card positions
    /// (clamped to `[0, scroll_max]`).
    pub(crate) scroll: f64,
    /// Phase 22.7: the largest scroll that brings the last card's right edge
    /// to the "+" slot boundary (`0` when the strip fits — no overflow).
    pub(crate) scroll_max: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TabCardGeometry {
    pub(crate) rect: Rect,
    pub(crate) label_rect: Rect,
    pub(crate) close_rect: Rect,
}

/// Phase 22.3: one tab's chrome state: its own split tree, retained pane
/// content hosts, routing targets, and pane-activation policy. The shell
/// hosts every tab's hosts (stable `WidgetId`s); only the active tab's hosts
/// are laid out at their pane rects — inactive tabs are laid out at zero size
/// (the Phase 22.1 `pending_orphans` protocol) so their widgets stay in the
/// tree and keep receiving connection events without painting or hit-testing.
///
/// CONTRACT (connection owner): the chrome `EditorWidget` is the tab's
/// connection owner — the widget connection events (theme, SDUI snapshot,
/// runtime state, editor commands) apply through, mounted at `editor_pane_id`
/// (pane 1 today). Closing that pane orphans the owner at zero size; it must
/// remain in the Masonry tree (`chrome_orphans`) and keep receiving events
/// — event routing NEVER assumes the owner is visible or mounted in a pane.
pub struct TabChrome {
    layout: WorkingAreaLayout,
    /// Phase 22.1: one retained content host per pane leaf, keyed by pane ID.
    pane_hosts: BTreeMap<PaneId, WidgetPod<PaneContentHost>>,
    /// `WidgetId` of the hosted `EditorWidget` (mounted once in Phase 22.1).
    editor_widget_id: WidgetId,
    /// The pane that mounts the connection owner (`PaneContent::Editor`).
    /// Fixed at construction; closing this pane orphans the owner rather than
    /// detaching it (see `chrome_orphans`).
    editor_pane_id: PaneId,
    /// The connection owner's host after its pane closed (zero-size orphan).
    /// Unlike `pending_orphans`, these are NEVER detached: the owner must stay
    /// in the tree so `editor_widget_id` remains editable and connection
    /// events keep applying. Registered and laid out at zero size forever.
    chrome_orphans: Vec<WidgetPod<PaneContentHost>>,
    /// Phase 22.2: pane → content widget id for keyboard/event routing. Pane 1
    /// maps to the chrome (`editor_widget_id`); document panes map to their
    /// `PaneDocumentView` (registered by the app driver when mounting).
    pane_targets: BTreeMap<PaneId, WidgetId>,
    /// Hosts removed from the tree without a `MutateCtx` available. Detached by
    /// the next [`Self::reconcile_pane_hosts`] call; laid out at zero size until
    /// then so Masonry's canonical children list stays consistent.
    pending_orphans: Vec<WidgetPod<PaneContentHost>>,
    /// Phase 22.1: pane activation policy (click vs focus-follows-cursor).
    pane_focus_policy: PaneFocusPolicy,
    /// Phase 22.6: pane hosts already inserted in the Masonry tree by a
    /// register pass. Newly synced hosts are absent until the next register
    /// pass, and `MutateCtx::get_mut` panics on them, so accessibility count
    /// updates skip them (they receive the count at creation instead).
    registered_panes: BTreeSet<PaneId>,
}

impl TabChrome {
    /// Build one tab's chrome state: a single-editor pane tree hosting
    /// `editor`. `restore_persisted` applies the Phase 20.3 `layout.json`
    /// restore (first tab only; per-tab layout persistence is 22.5).
    #[doc(hidden)]
    pub fn single_editor(editor: EditorWidget, restore_persisted: bool) -> Self {
        let mut layout = WorkingAreaLayout::single_editor();
        if restore_persisted {
            // Phase 20.3: restore persisted layout state at startup.
            if let Some(state) = crate::shell::layout_persist::load_layout() {
                crate::shell::layout_persist::apply_persisted_state(&mut layout, &state);
            }
        }
        Self::with_layout(editor, layout)
    }

    /// Build one tab's chrome state from an explicit layout (restore path
    /// and tests).
    pub(crate) fn with_layout(mut editor: EditorWidget, layout: WorkingAreaLayout) -> Self {
        let editor_pane_id = layout.editor_component().pane_id;
        // Phase 22.2: the chrome must know which pane it hosts for pane-focus
        // actions; set before the chrome is moved into the host.
        editor.set_pane_id(editor_pane_id);
        // Seed pane hosts with the editor's installed theme so restored and
        // placeholder panes follow the active theme from the first paint.
        let ui_theme = editor.ui_theme().clone();
        let editor = NewWidget::new(editor);
        let editor_widget_id = editor.id();
        let mut editor = Some(editor);
        let mut pane_targets = BTreeMap::new();
        pane_targets.insert(editor_pane_id, editor_widget_id);
        let pane_count = layout.pane_tree().pane_ids().len();
        let pane_hosts = layout
            .pane_tree()
            .pane_ids()
            .into_iter()
            .map(|pane_id| {
                let host = if pane_id == editor_pane_id {
                    PaneContentHost::with_editor(
                        pane_id,
                        editor
                            .take()
                            .expect("editor pane is a member of the pane tree"),
                    )
                } else {
                    PaneContentHost::placeholder(pane_id)
                };
                (
                    pane_id,
                    NewWidget::new(
                        host.with_pane_count(pane_count)
                            .with_ui_theme(ui_theme.clone()),
                    )
                    .to_pod(),
                )
            })
            .collect();
        Self {
            layout,
            pane_hosts,
            editor_widget_id,
            editor_pane_id,
            chrome_orphans: Vec::new(),
            pane_targets,
            pending_orphans: Vec::new(),
            pane_focus_policy: PaneFocusPolicy::default(),
            registered_panes: BTreeSet::new(),
        }
    }

    /// The chrome's widget id (the tab's event-bridge routing tag).
    #[doc(hidden)]
    pub fn editor_widget_id(&self) -> WidgetId {
        self.editor_widget_id
    }
}

pub struct ClayShellWidget {
    /// Phase 22.3: one chrome state per tab, keyed by the tab's connection
    /// `ClientId` (the client-known identity at mount time; the server-assigned
    /// `TabId` arrives asynchronously via the registry snapshot and is tracked
    /// by the app driver). The active tab's hosts are laid out at their pane
    /// rects; inactive tabs' hosts are retained at zero size.
    tabs: BTreeMap<ClientId, TabChrome>,
    /// The mounted (active) tab. Invariant: `active_tab` names a mounted tab
    /// OR `tabs` is empty (removing the last tab leaves this field at the
    /// removed value; every public entry point early-returns on the empty
    /// map, so the stale value is never dereferenced).
    active_tab: ClientId,
    /// Phase 20.3: split divider drag session state.
    split_drag: SplitDragState,
    /// Phase 20.3: fixed slot resize drag session state.
    slot_drag: SlotDragState,
    /// Phase 20.3: double-click detection for slot collapse toggle.
    last_slot_click: Option<(std::time::Instant, FixedSlotId)>,
    /// Phase 20.3: debounced layout persistence timestamp.
    last_persist: Option<std::time::Instant>,
    /// Phase 22.3: tab bar cards (registry-driven; empty → the bar is hidden
    /// and working-area geometry is the pre-22.3 shape). Shown only when more
    /// than one tab is mounted.
    tab_cards: Vec<TabCard>,
    /// Phase 22.3: hovered card index for state paint (None when the pointer
    /// is not over a card).
    tab_bar_hover: Option<usize>,
    /// Hover state for the pinned new-tab affordance.
    tab_bar_new_tab_hover: bool,
    /// Phase 22.7: horizontal strip scroll offset (one `f64`; clamped to
    /// `[0, max_scroll]` by `tab_bar_geometry` and every mutation path).
    /// `0` when the strip fits, so non-overflowing bars never shift.
    tab_bar_scroll: f64,
    /// Phase 22.3: shell text paint uses the UI typography profile (default
    /// registry; syncing the active tab's typography is not wired in 22.3).
    typography: TypographyRegistry,
    /// Phase 22.6 (task 4): pending polite live-region announcement text.
    /// `None`/empty until the first window-model action; replaced (never
    /// appended) per action.
    announcement: Option<String>,
    /// Resolved active-theme UI tokens for shell chrome paint (tab bar, split
    /// dividers, focus rings) and new placeholder panes. Defaults to the Clay
    /// theme until the first `set_active_theme`.
    ui_theme: ResolvedUiTheme,
}

impl ClayShellWidget {
    pub fn single_editor(client_id: ClientId, editor: EditorWidget) -> Self {
        let ui_theme = editor.ui_theme().clone();
        Self::from_chrome(client_id, TabChrome::single_editor(editor, true), ui_theme)
    }

    /// Phase 22.5: build the shell for a restored window — the bootstrap tab
    /// mounts with its persisted split tree (never the 20.3 legacy apply;
    /// legacy files keep the `single_editor` bootstrap path).
    pub fn restored_single_editor(
        client_id: ClientId,
        editor: EditorWidget,
        persisted: &PersistedTabState,
    ) -> Self {
        let ui_theme = editor.ui_theme().clone();
        Self::from_chrome(
            client_id,
            TabChrome::with_layout(
                editor,
                crate::shell::layout_persist::layout_from_persisted_tab(persisted),
            ),
            ui_theme,
        )
    }

    fn from_chrome(client_id: ClientId, chrome: TabChrome, ui_theme: ResolvedUiTheme) -> Self {
        let mut tabs = BTreeMap::new();
        tabs.insert(client_id, chrome);
        Self {
            tabs,
            active_tab: client_id,
            split_drag: SplitDragState::Idle,
            slot_drag: SlotDragState::Idle,
            last_slot_click: None,
            last_persist: None,
            tab_cards: Vec::new(),
            tab_bar_hover: None,
            tab_bar_new_tab_hover: false,
            tab_bar_scroll: 0.0,
            typography: TypographyRegistry::default(),
            announcement: None,
            ui_theme,
        }
    }

    #[cfg(test)]
    pub(crate) fn single_editor_with_layout(
        editor: EditorWidget,
        layout: WorkingAreaLayout,
    ) -> Self {
        let mut tabs = BTreeMap::new();
        tabs.insert(0, TabChrome::with_layout(editor, layout));
        Self {
            tabs,
            active_tab: 0,
            split_drag: SplitDragState::Idle,
            slot_drag: SlotDragState::Idle,
            last_slot_click: None,
            last_persist: None,
            tab_cards: Vec::new(),
            tab_bar_hover: None,
            tab_bar_new_tab_hover: false,
            tab_bar_scroll: 0.0,
            typography: TypographyRegistry::default(),
            announcement: None,
            ui_theme: ResolvedUiTheme::default(),
        }
    }

    /// The active tab's chrome state.
    fn active(&self) -> &TabChrome {
        debug_assert!(
            self.tabs.contains_key(&self.active_tab),
            "invariant: active_tab names a mounted tab (tabs empty means no active() call)"
        );
        &self.tabs[&self.active_tab]
    }

    /// The active tab's chrome state (mutable).
    fn active_mut(&mut self) -> &mut TabChrome {
        debug_assert!(
            self.tabs.contains_key(&self.active_tab),
            "invariant: active_tab names a mounted tab (tabs empty means no active_mut() call)"
        );
        self.tabs
            .get_mut(&self.active_tab)
            .expect("active tab is always present")
    }

    pub fn editor_widget_id(&self) -> WidgetId {
        self.active().editor_widget_id
    }

    /// Phase 22.2: the active pane's content widget (keyboard routing target).
    pub fn focus_fallback_widget_id(&self) -> WidgetId {
        self.active_pane_target()
            .unwrap_or_else(|| self.editor_widget_id())
    }

    // -- Phase 22.2: pane content routing --

    /// Register/update the content widget target for one pane of the active
    /// tab (the app driver calls this after mounting a document view).
    pub fn set_pane_target(&mut self, pane_id: PaneId, widget_id: WidgetId) {
        self.set_pane_target_for(self.active_tab, pane_id, widget_id);
    }

    /// The content widget target for one pane of the active tab (chrome for
    /// the editor pane, the mounted `PaneDocumentView` for document panes).
    pub fn pane_target(&self, pane_id: PaneId) -> Option<WidgetId> {
        self.pane_target_for(self.active_tab, pane_id)
    }

    /// All pane content targets of the active tab (driver routing).
    pub fn pane_targets(&self) -> Vec<(PaneId, WidgetId)> {
        self.pane_targets_for(self.active_tab)
    }

    pub fn active_pane_id(&self) -> PaneId {
        self.active_pane_id_for(self.active_tab)
    }

    /// The active tab's active pane content widget target, if any.
    pub fn active_pane_target(&self) -> Option<WidgetId> {
        self.active_pane_target_for(self.active_tab)
    }

    /// The content host widget id for one pane of the active tab (driver
    /// mounts document views).
    pub fn pane_host_id(&self, pane_id: PaneId) -> Option<WidgetId> {
        self.pane_host_id_for(self.active_tab, pane_id)
    }

    // -- Phase 22.3: per-tab routing (the app driver routes each tab's
    // connection events to that tab's chrome and panes) --

    /// The tab owning `widget_id` (its chrome id), if any.
    pub fn tab_for_chrome(&self, widget_id: WidgetId) -> Option<ClientId> {
        self.tabs
            .iter()
            .find_map(|(client_id, tab)| (tab.editor_widget_id == widget_id).then_some(*client_id))
    }

    /// Mount a tab's chrome state (open-tab path). The first mounted tab
    /// becomes active; later tabs are retained at zero size until switched in.
    pub fn install_tab(&mut self, ctx: &mut MutateCtx<'_>, client_id: ClientId, chrome: TabChrome) {
        let first = self.tabs.is_empty();
        self.tabs.insert(client_id, chrome);
        if first {
            self.active_tab = client_id;
        }
        ctx.children_changed();
    }

    /// Switch the mounted tab. Returns false when `client_id` is unknown or
    /// already active. Resets in-flight drag sessions (a drag across a switch
    /// is meaningless).
    pub fn set_active_tab(&mut self, ctx: &mut MutateCtx<'_>, client_id: ClientId) -> bool {
        if client_id == self.active_tab || !self.tabs.contains_key(&client_id) {
            return false;
        }
        self.active_tab = client_id;
        self.split_drag = SplitDragState::Idle;
        self.slot_drag = SlotDragState::Idle;
        self.last_slot_click = None;
        // Phase 22.7: an off-screen active card must become reachable.
        self.scroll_active_card_into_view(ctx.size());
        ctx.children_changed();
        ctx.request_layout();
        true
    }

    /// Phase 22.5: mount a restored tab with its persisted split tree.
    /// Returns the chrome's widget id (the event-bridge routing tag). Does
    /// not switch the active tab — restore activates the persisted active
    /// tab after every mount confirms.
    pub fn install_restored_tab(
        &mut self,
        ctx: &mut MutateCtx<'_>,
        client_id: ClientId,
        editor: EditorWidget,
        persisted: &PersistedTabState,
    ) -> WidgetId {
        let layout = crate::shell::layout_persist::layout_from_persisted_tab(persisted);
        let chrome = TabChrome::with_layout(editor, layout);
        let chrome_id = chrome.editor_widget_id();
        self.install_tab(ctx, client_id, chrome);
        chrome_id
    }

    #[cfg(test)]
    pub(crate) fn tab_bar_hover_index(&self) -> Option<usize> {
        self.tab_bar_hover
    }

    /// Phase 22.3: remove a mounted tab: its hosts and orphaned pods leave
    /// the Masonry tree, and the map entry drops. When the removed tab was
    /// active, the first remaining tab becomes active (the shell invariant is
    /// that `active_tab` always names a mounted tab; the driver moves focus).
    pub fn remove_tab(&mut self, ctx: &mut MutateCtx<'_>, client_id: ClientId) {
        let Some(mut tab) = self.tabs.remove(&client_id) else {
            return;
        };
        let hosts = std::mem::take(&mut tab.pane_hosts);
        for (_, host) in hosts {
            ctx.remove_child(host);
        }
        for orphan in tab.pending_orphans.drain(..) {
            ctx.remove_child(orphan);
        }
        // Phase 22.7: the connection owner's orphan host leaves with the tab.
        for orphan in tab.chrome_orphans.drain(..) {
            ctx.remove_child(orphan);
        }
        if self.active_tab == client_id
            && let Some((next, _)) = self.tabs.iter().next()
        {
            self.active_tab = *next;
        }
        // Removing at the two-card boundary can hide the bar before replacement cards arrive; clear stale hover.
        if self.tab_cards.len() <= 2 {
            self.tab_bar_hover = None;
            self.tab_bar_new_tab_hover = false;
        }
        // Phase 22.6 (task 4): announce the close (name/position from the
        // registry cards; remaining count from the tabs map after removal).
        // A connection-drop removal announces too — the tab visibly
        // disappears, so the window-model change reaches screen-reader
        // users either way.
        let position = self
            .tab_cards
            .iter()
            .position(|card| card.client_id == client_id)
            .unwrap_or(0)
            + 1;
        let name = self
            .tab_cards
            .iter()
            .find(|card| card.client_id == client_id)
            .map(|card| card.name.as_str());
        self.announce(
            ctx,
            compose_announcement(AnnouncementKind::TabClosed, name, position, self.tabs.len()),
        );
        ctx.children_changed();
    }

    /// Phase 22.3: re-key a mounted tab from `old` to `new` after a reconnect
    /// (`Reclaim` rebinds the registry entry to the new connection's
    /// `ClientId`). The `TabChrome` — widgets, split tree, pane targets,
    /// focus policy — moves wholesale, so every widget id stays stable; only
    /// the map key and the card's client id change.
    pub fn rekey_tab(&mut self, ctx: &mut MutateCtx<'_>, old: ClientId, new: ClientId) -> bool {
        let Some(tab) = self.tabs.remove(&old) else {
            return false;
        };
        self.tabs.insert(new, tab);
        if self.active_tab == old {
            self.active_tab = new;
        }
        for card in &mut self.tab_cards {
            if card.client_id == old {
                card.client_id = new;
            }
        }
        ctx.request_render();
        true
    }

    /// Phase 22.3: install the tab bar cards (registry-driven names/order).
    /// The bar is painted only while more than one card is present, so
    /// single-tab working-area geometry stays the pre-22.3 shape.
    pub fn set_tab_cards(&mut self, ctx: &mut MutateCtx<'_>, cards: Vec<TabCard>) {
        let geometry_changed = (cards.len() >= 2) != (self.tab_cards.len() >= 2);
        self.tab_cards = cards;
        if self.tab_cards.len() < 2 {
            self.tab_bar_hover = None;
            self.tab_bar_new_tab_hover = false;
            self.tab_bar_scroll = 0.0;
        }
        // Phase 22.7: registry-driven order/names may move the active card;
        // keep it visible.
        self.scroll_active_card_into_view(ctx.size());
        if geometry_changed {
            ctx.request_layout();
        }
        ctx.request_render();
        ctx.request_accessibility_update();
    }

    /// Phase 22.6: set a pane's document display name from the raw document
    /// `path` (the app driver calls this when a document open/reload lands
    /// in the pane); the name is sanitized here, at the accessibility
    /// boundary, so pane labels never announce host paths. `None` clears it.
    pub fn set_pane_document_name(
        &mut self,
        ctx: &mut MutateCtx<'_>,
        client_id: ClientId,
        pane_id: PaneId,
        path: Option<&str>,
    ) {
        let Some(host) = self
            .tabs
            .get_mut(&client_id)
            .and_then(|tab| tab.pane_hosts.get_mut(&pane_id))
        else {
            return;
        };
        let name = path.map(crate::editor::accessibility::sanitize_document_display_name);
        let mut host = ctx.get_mut(host);
        host.widget.set_document_display_name(&mut host.ctx, name);
    }

    /// Phase 22.3: computed tab bar geometry for a window `size`. `None` when
    /// the bar is hidden (fewer than two cards): working-area geometry is then
    /// exactly the pre-22.3 shape.
    pub(crate) fn tab_bar_geometry(&self, size: Size) -> Option<TabBarGeometry> {
        if self.tab_cards.len() < 2 {
            return None;
        }
        let bar = Rect::new(0.0, 0.0, size.width, TAB_BAR_HEIGHT);
        let new_tab_rect = Rect::new(
            bar.x1 - TAB_BAR_NEW_TAB_SIZE - TAB_BAR_NEW_TAB_GAP,
            bar.y0 + (bar.height() - TAB_BAR_NEW_TAB_SIZE) / 2.0,
            bar.x1 - TAB_BAR_NEW_TAB_GAP,
            bar.y0 + (bar.height() + TAB_BAR_NEW_TAB_SIZE) / 2.0,
        );
        // Phase 22.7 (D6/F5): cards shrink-to-fit until the minimum width
        // binds, then the strip overflows and scrolls. Widths come from the
        // UNSCROLLED positions so every card keeps the same shape; positions
        // shift by the clamped scroll afterwards.
        let mut cards = Vec::with_capacity(self.tab_cards.len());
        let mut x = TAB_BAR_CARD_GAP;
        for _ in &self.tab_cards {
            let width = (TAB_BAR_CARD_WIDTH)
                .min((new_tab_rect.x0 - x - TAB_BAR_CARD_GAP).max(TAB_BAR_CARD_MIN_WIDTH));
            let rect = Rect::new(x, TAB_BAR_CARD_GAP, x + width, bar.y1 - TAB_BAR_CARD_GAP);
            cards.push(TabCardGeometry {
                rect,
                label_rect: Rect::new(
                    rect.x0 + TAB_BAR_CARD_PADDING,
                    rect.y0,
                    (rect.x1 - TAB_BAR_CARD_PADDING - TAB_BAR_CLOSE_SIZE - TAB_BAR_CARD_PADDING)
                        .max(rect.x0 + TAB_BAR_CARD_PADDING),
                    rect.y1,
                ),
                close_rect: Rect::new(
                    rect.x1 - TAB_BAR_CARD_PADDING - TAB_BAR_CLOSE_SIZE,
                    rect.y0 + (rect.height() - TAB_BAR_CLOSE_SIZE) / 2.0,
                    rect.x1 - TAB_BAR_CARD_PADDING,
                    rect.y0 + (rect.height() - TAB_BAR_CLOSE_SIZE) / 2.0 + TAB_BAR_CLOSE_SIZE,
                ),
            });
            x = rect.x1 + TAB_BAR_CARD_GAP;
        }
        // Last card's right edge can reach the "+" slot boundary.
        let scroll_max = (x - TAB_BAR_CARD_GAP - new_tab_rect.x0).max(0.0);
        let scroll = self.tab_bar_scroll.clamp(0.0, scroll_max);
        if scroll > 0.0 {
            for card in &mut cards {
                card.rect = Rect::new(
                    card.rect.x0 - scroll,
                    card.rect.y0,
                    card.rect.x1 - scroll,
                    card.rect.y1,
                );
                card.label_rect = Rect::new(
                    card.label_rect.x0 - scroll,
                    card.label_rect.y0,
                    card.label_rect.x1 - scroll,
                    card.label_rect.y1,
                );
                card.close_rect = Rect::new(
                    card.close_rect.x0 - scroll,
                    card.close_rect.y0,
                    card.close_rect.x1 - scroll,
                    card.close_rect.y1,
                );
            }
        }
        Some(TabBarGeometry {
            bar,
            cards,
            new_tab_rect,
            scroll,
            scroll_max,
        })
    }

    /// Hit-test a tab bar point: `(card_index, hit_close_glyph)`. Close wins
    /// inside the card (the glyph is the rightmost affordance).
    pub(crate) fn tab_bar_hit_test(
        &self,
        geometry: &TabBarGeometry,
        point: Point,
    ) -> Option<(usize, bool)> {
        for (index, card) in geometry.cards.iter().enumerate() {
            if card.close_rect.contains(point) {
                return Some((index, true));
            }
            if card.rect.contains(point) {
                return Some((index, false));
            }
        }
        None
    }

    /// Phase 22.7 (D6/F5): bring the active card's visible edge into the
    /// strip. Left edge wins when the card is wider than the strip. No-op
    /// when the bar is hidden or the card is already visible.
    fn scroll_active_card_into_view(&mut self, size: Size) {
        let Some(geometry) = self.tab_bar_geometry(size) else {
            return;
        };
        let Some(index) = self
            .tab_cards
            .iter()
            .position(|card| card.client_id == self.active_tab)
        else {
            return;
        };
        let rect = geometry.cards[index].rect;
        let mut target = geometry.scroll;
        if rect.x0 < 0.0 {
            target += rect.x0;
        } else if rect.x1 > geometry.new_tab_rect.x0 {
            target += rect.x1 - geometry.new_tab_rect.x0;
        }
        self.tab_bar_scroll = target.clamp(0.0, geometry.scroll_max);
    }

    /// Phase 22.7: horizontal strip scroll read (for tests and diagnostics).
    #[cfg(test)]
    pub(crate) fn tab_bar_scroll(&self) -> f64 {
        self.tab_bar_scroll
    }

    pub fn pane_target_for(&self, client_id: ClientId, pane_id: PaneId) -> Option<WidgetId> {
        self.tabs
            .get(&client_id)
            .and_then(|tab| tab.pane_targets.get(&pane_id).copied())
    }

    pub fn pane_targets_for(&self, client_id: ClientId) -> Vec<(PaneId, WidgetId)> {
        self.tabs
            .get(&client_id)
            .map(|tab| {
                tab.pane_targets
                    .iter()
                    .map(|(pane, widget)| (*pane, *widget))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Phase 22.5: every mounted tab's layout snapshot (active pane, split
    /// tree, user-modified slots) in client-id order, for whole-window
    /// persistence collection. The driver adds per-pane document identity.
    pub fn tab_layout_data(&self) -> Vec<(ClientId, PersistedTabLayout)> {
        self.tabs
            .iter()
            .map(|(client_id, tab)| {
                (
                    *client_id,
                    PersistedTabLayout {
                        active_pane: tab.layout.active_pane_id(),
                        tree: tab.layout.pane_tree().root_node().clone(),
                        slots: crate::shell::layout_persist::collect_slot_entries(&tab.layout),
                    },
                )
            })
            .collect()
    }

    pub fn active_pane_id_for(&self, client_id: ClientId) -> PaneId {
        self.tabs
            .get(&client_id)
            .map(|tab| tab.layout.active_pane_id())
            .unwrap_or(PaneId(1))
    }

    pub fn active_pane_target_for(&self, client_id: ClientId) -> Option<WidgetId> {
        self.pane_target_for(client_id, self.active_pane_id_for(client_id))
    }

    pub fn pane_host_id_for(&self, client_id: ClientId, pane_id: PaneId) -> Option<WidgetId> {
        self.tabs
            .get(&client_id)
            .and_then(|tab| tab.pane_hosts.get(&pane_id).map(|host| host.id()))
    }

    pub fn editor_widget_id_for(&self, client_id: ClientId) -> Option<WidgetId> {
        self.tabs.get(&client_id).map(|tab| tab.editor_widget_id)
    }

    pub fn set_pane_target_for(
        &mut self,
        client_id: ClientId,
        pane_id: PaneId,
        widget_id: WidgetId,
    ) {
        if let Some(tab) = self.tabs.get_mut(&client_id) {
            tab.pane_targets.insert(pane_id, widget_id);
        }
    }

    pub fn set_active_pane_for(&mut self, client_id: ClientId, pane_id: PaneId) {
        if let Some(tab) = self.tabs.get_mut(&client_id) {
            let _ = tab.layout.set_focus_pane(pane_id);
        }
    }

    pub fn set_pane_focus_policy_for(&mut self, client_id: ClientId, policy: PaneFocusPolicy) {
        if let Some(tab) = self.tabs.get_mut(&client_id) {
            tab.pane_focus_policy = policy;
        }
    }

    pub fn pane_focus_policy_for(&self, client_id: ClientId) -> PaneFocusPolicy {
        self.tabs
            .get(&client_id)
            .map(|tab| tab.pane_focus_policy)
            .unwrap_or_default()
    }

    /// Submit the pane-activation action so the driver can sync Masonry focus
    /// to the pane's content widget.
    fn submit_pane_focused(&self, ctx: &mut EventCtx<'_>, pane_id: PaneId) {
        ctx.submit_action::<EditorAction>(EditorAction::PaneFocused(pane_id));
    }

    #[cfg(test)]
    pub(crate) fn apply_layout_update(
        &mut self,
        update: WorkingAreaLayoutUpdate,
    ) -> Result<(), WorkingAreaLayoutUpdateError> {
        self.active_mut().layout.apply_update(update)?;
        self.sync_pane_hosts_state();
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn observable_snapshot(&self, size: Size) -> ShellObservableSnapshot {
        let active = self.active();
        ShellObservableSnapshot {
            layout: active
                .layout
                .observable_snapshot(Rect::new(0.0, 0.0, size.width, size.height)),
            editor_component_bound: active.layout.editor_component().kind
                == ShellComponentKind::Editor,
            sdui_state_present: true,
            status_present: true,
        }
    }

    #[cfg(test)]
    pub(crate) fn working_area_layout(&self) -> &WorkingAreaLayout {
        &self.active().layout
    }

    /// Phase 22.5 (tests): any tab's layout, so composition guards can prove
    /// inactive tabs stay untouched.
    #[cfg(test)]
    pub(crate) fn working_area_layout_for(
        &self,
        client_id: ClientId,
    ) -> Option<&WorkingAreaLayout> {
        self.tabs.get(&client_id).map(|tab| &tab.layout)
    }

    #[cfg(test)]
    pub(crate) fn editor_component_rect_for_size(&self, size: Size) -> Rect {
        self.editor_component_rect(size)
    }

    fn layout_size(bc: &BoxConstraints) -> Size {
        if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(900.0, 600.0))
        }
    }

    #[cfg(test)]
    fn editor_component_rect(&self, size: Size) -> Rect {
        self.active()
            .layout
            .editor_component_rect(Rect::new(0.0, 0.0, size.width, size.height))
    }

    /// Phase 20.3 + 22.5: debounced persistence signal (≥500ms between
    /// emissions). The driver owns the whole-window save (it assembles
    /// per-tab layouts + per-pane document identity); the shell only reports
    /// that a layout mutation committed. The 22.3 single-tab guard is gone —
    /// mutations persist at any tab count.
    fn persist_debounced(&mut self, ctx: &mut EventCtx<'_>) {
        if self.mark_persistence_due() {
            ctx.submit_action::<EditorAction>(EditorAction::PersistenceDue);
        }
    }

    /// Phase 22.5: debounce gate shared by every shell mutation path
    /// (pointer drags, keyboard topology changes, keyboard resize).
    fn mark_persistence_due(&mut self) -> bool {
        let now = std::time::Instant::now();
        if self
            .last_persist
            .is_some_and(|t| now.duration_since(t).as_millis() < 500)
        {
            return false;
        }
        self.last_persist = Some(now);
        true
    }

    // -- Phase 22.1: multi-pane hosting --

    pub fn pane_focus_policy(&self) -> PaneFocusPolicy {
        self.active().pane_focus_policy
    }

    /// Phase 22.1: set pane activation policy (wired by `ShellPreferences`
    /// transport from `setPaneFocusPolicy` in `init.js`).
    pub fn set_pane_focus_policy(&mut self, policy: PaneFocusPolicy) {
        self.set_pane_focus_policy_for(self.active_tab, policy);
    }

    /// Phase 22.6 (task 4): replace the polite live-region announcement text
    /// and invalidate the accessibility tree. The tree rebuilds only on this
    /// request (or an explicit `request_accessibility_update`), so repaints
    /// alone never re-announce.
    pub fn announce(&mut self, ctx: &mut MutateCtx<'_>, message: String) {
        self.announcement = Some(message);
        ctx.request_accessibility_update();
    }

    /// Phase 22.6 (task 4): announce a user-initiated tab switch. The driver
    /// calls this only from `activate_tab` after a successful switch;
    /// restore and registry-reconcile switches are model changes and stay
    /// silent.
    pub fn announce_tab_activated(&mut self, ctx: &mut MutateCtx<'_>, client_id: ClientId) {
        let position = self
            .tab_cards
            .iter()
            .position(|card| card.client_id == client_id)
            .unwrap_or(0)
            + 1;
        let name = self
            .tab_cards
            .iter()
            .find(|card| card.client_id == client_id)
            .map(|card| card.name.as_str());
        self.announce(
            ctx,
            compose_announcement(AnnouncementKind::TabActivated, name, position, 0),
        );
    }

    /// Phase 22.6 (task 4): announce a user-initiated new tab. The driver
    /// calls this from `mount_tab` after installing the chrome; restore
    /// mounts (`install_restored_tab`) are model changes and stay silent.
    pub fn announce_tab_created(&mut self, ctx: &mut MutateCtx<'_>, name: &str) {
        let position = self.tabs.len();
        self.announce(
            ctx,
            compose_announcement(AnnouncementKind::TabCreated, Some(name), position, 0),
        );
    }

    /// Phase 22.6 (task 4): announce a completed pane-tree change with the
    /// post-change pane count.
    fn announce_pane_change(&mut self, ctx: &mut MutateCtx<'_>, kind: AnnouncementKind) {
        let count = self.active().layout.pane_tree().pane_ids().len();
        self.announce(ctx, compose_announcement(kind, None, 0, count));
    }

    /// Phase 22.1: execute a client-routed shell pane command.
    ///
    /// No-ops at bounds (single pane close, cap reached, move at ends) without
    /// errors. Topology mutations (split/close/add-equal/move) reconcile pane
    /// hosts; focus/resize update only the active pane or a single divider ratio.
    pub fn apply_shell_client_command(
        &mut self,
        ctx: &mut MutateCtx<'_>,
        command: ShellClientCommand,
    ) {
        if self.tabs.is_empty() {
            return; // zero-tab shell: no pane state to command (invariant guard).
        }
        let active = self.active().layout.active_pane_id();
        match command {
            // Vim-style: "vertical" = side by side (vertical divider) →
            // SplitOrientation::Horizontal; "horizontal" = stacked → Vertical.
            ShellClientCommand::SplitPaneVertical => {
                let new_id = self.active().layout.pane_tree().next_pane_id();
                let tree = self.active().layout.pane_tree().split_pane(
                    active,
                    new_id,
                    SplitOrientation::Horizontal,
                    SplitRatio::balanced(),
                    SplitChild::Second,
                );
                if self.apply_tree_change(ctx, tree) {
                    self.announce_pane_change(ctx, AnnouncementKind::SplitPaneVertical);
                }
            }
            ShellClientCommand::SplitPaneHorizontal => {
                let new_id = self.active().layout.pane_tree().next_pane_id();
                let tree = self.active().layout.pane_tree().split_pane(
                    active,
                    new_id,
                    SplitOrientation::Vertical,
                    SplitRatio::balanced(),
                    SplitChild::Second,
                );
                if self.apply_tree_change(ctx, tree) {
                    self.announce_pane_change(ctx, AnnouncementKind::SplitPaneHorizontal);
                }
            }
            ShellClientCommand::AddEqualPane => {
                if self.apply_tree_change(ctx, self.active().layout.pane_tree().add_equal_pane()) {
                    self.announce_pane_change(ctx, AnnouncementKind::PaneAdded);
                }
            }
            ShellClientCommand::ClosePane => {
                if self.apply_tree_change(ctx, self.active().layout.pane_tree().close_pane(active))
                {
                    self.announce_pane_change(ctx, AnnouncementKind::PaneClosed);
                }
            }
            ShellClientCommand::MovePaneNext => {
                if self.apply_tree_change(
                    ctx,
                    self.active()
                        .layout
                        .pane_tree()
                        .move_pane(active, SplitChild::Second),
                ) {
                    self.announce_pane_change(ctx, AnnouncementKind::PaneMovedForward);
                }
            }
            ShellClientCommand::MovePanePrev => {
                if self.apply_tree_change(
                    ctx,
                    self.active()
                        .layout
                        .pane_tree()
                        .move_pane(active, SplitChild::First),
                ) {
                    self.announce_pane_change(ctx, AnnouncementKind::PaneMovedBackward);
                }
            }
            ShellClientCommand::FocusPaneNext => {
                let next = self.active().layout.pane_tree().next_pane();
                if next != active {
                    let _ = self.active_mut().layout.set_focus_pane(next);
                    ctx.request_render();
                }
            }
            ShellClientCommand::FocusPanePrev => {
                let prev = self.active().layout.pane_tree().prev_pane();
                if prev != active {
                    let _ = self.active_mut().layout.set_focus_pane(prev);
                    ctx.request_render();
                }
            }
            ShellClientCommand::ResizePaneLeft => {
                self.apply_keyboard_resize(ctx, PaneResizeDirection::Left);
            }
            ShellClientCommand::ResizePaneRight => {
                self.apply_keyboard_resize(ctx, PaneResizeDirection::Right);
            }
            ShellClientCommand::ResizePaneUp => {
                self.apply_keyboard_resize(ctx, PaneResizeDirection::Up);
            }
            ShellClientCommand::ResizePaneDown => {
                self.apply_keyboard_resize(ctx, PaneResizeDirection::Down);
            }
            // Phase 22.4: tab commands are driver-routed — they act on the
            // driver's tab state (active tab, connections, registry snapshots),
            // not the shell widget's pane tree. The driver intercepts them
            // before this call; the arms are inert here so a chord in the
            // interim wiring is a no-op, never a crash.
            ShellClientCommand::TabNext
            | ShellClientCommand::TabPrev
            | ShellClientCommand::TabNew
            | ShellClientCommand::TabClose
            | ShellClientCommand::TabMoveLeft
            | ShellClientCommand::TabMoveRight
            | ShellClientCommand::TabActivate(_)
            | ShellClientCommand::TabMoveTo(_) => {}
        }
    }

    /// Apply a topology-changing tree operation (split/close/add-equal/move).
    /// Returns whether the change happened (bounds no-ops return false), so
    /// callers announce only real changes.
    fn apply_tree_change(
        &mut self,
        ctx: &mut MutateCtx<'_>,
        new_tree: Option<PaneSplitTree>,
    ) -> bool {
        if let Some(new_tree) = new_tree {
            self.active_mut().layout.replace_pane_tree(new_tree);
            self.reconcile_pane_hosts(ctx);
            ctx.request_layout();
            if self.mark_persistence_due() {
                ctx.submit_action::<EditorAction>(EditorAction::PersistenceDue);
            }
            true
        } else {
            false
        }
    }

    /// Apply one keyboard resize step to the divider bordering the active pane.
    fn apply_keyboard_resize(&mut self, ctx: &mut MutateCtx<'_>, direction: PaneResizeDirection) {
        let active = self.active().layout.active_pane_id();
        if let Some((path, ratio)) = self.active().layout.pane_tree().keyboard_resize(
            active,
            direction,
            KEYBOARD_RESIZE_STEP,
        ) && self.active_mut().layout.commit_split_drag(&path, ratio)
        {
            ctx.request_layout();
            if self.mark_persistence_due() {
                ctx.submit_action::<EditorAction>(EditorAction::PersistenceDue);
            }
        }
    }

    /// Phase 22.2: activate a pane by id (driver-side pane-focus sync).
    pub fn set_active_pane(&mut self, pane_id: PaneId) {
        self.set_active_pane_for(self.active_tab, pane_id);
    }

    /// Phase 22.1: reconcile retained pane hosts with the layout's pane tree.
    ///
    /// New leaves get placeholder hosts, hosts of removed leaves are detached
    /// via `ctx.remove_child`, and surviving hosts keep their `WidgetId`s
    /// untouched. Call after any tree mutation applied outside construction.
    /// Always re-runs the register pass: `apply_layout_update` may have synced
    /// hosts (new pods) before a context was available.
    pub(crate) fn reconcile_pane_hosts(&mut self, ctx: &mut MutateCtx<'_>) {
        for orphan in self.active_mut().pending_orphans.drain(..) {
            ctx.remove_child(orphan);
        }
        self.sync_pane_hosts_state();
        // Phase 22.6: keep the "Pane N of M" accessibility count current on
        // every registered host. Hosts created by the sync (this call or an
        // earlier `apply_layout_update`) are not in the Masonry tree until
        // the next register pass — `get_mut` would panic on them — and they
        // received the count at creation instead.
        let count = self.active().layout.pane_tree().pane_ids().len();
        let registered = self.active().registered_panes.clone();
        for (pane_id, host) in self.active_mut().pane_hosts.iter_mut() {
            if !registered.contains(pane_id) {
                continue;
            }
            let mut host = ctx.get_mut(host);
            host.widget.set_pane_count(&mut host.ctx, count);
        }
        ctx.children_changed();
    }

    /// Install an inert `ActiveTheme` snapshot for shell chrome: resolve the
    /// design tokens over the theme's editor base palette (mirrors the editor
    /// surface install) and stamp every registered placeholder host so split
    /// panes follow the theme. Unregistered hosts were stamped at creation.
    pub fn set_active_theme(
        &mut self,
        ctx: &mut MutateCtx<'_>,
        theme: &crate::protocol::ActiveTheme,
    ) {
        let registry = crate::editor::theme::StyleRegistry::from_active_theme(theme);
        let base = registry.base;
        let Ok(resolved) = ResolvedUiTheme::from_active_theme(&theme.design_tokens) else {
            return;
        };
        self.ui_theme = resolved.with_base_ui(&base);
        let ui_theme = self.ui_theme.clone();
        for tab in self.tabs.values_mut() {
            let registered = tab.registered_panes.clone();
            for (pane_id, host) in tab.pane_hosts.iter_mut() {
                if registered.contains(pane_id) {
                    ctx.get_mut(host).widget.set_ui_theme(ui_theme.clone());
                }
            }
        }
    }

    /// State-level host sync against the pane tree (no Masonry context).
    ///
    /// Adds placeholder hosts for new leaves and stashes removed hosts in
    /// `pending_orphans` (a later `reconcile_pane_hosts` detaches them).
    fn sync_pane_hosts_state(&mut self) {
        let ui_theme = self.ui_theme.clone();
        let chrome = self.active_mut();
        let leaves: BTreeSet<PaneId> = chrome.layout.pane_tree().pane_ids().into_iter().collect();
        for pane_id in chrome.pane_hosts.keys().copied().collect::<Vec<_>>() {
            if !leaves.contains(&pane_id)
                && let Some(host) = chrome.pane_hosts.remove(&pane_id)
            {
                // Phase 22.2: drop routing targets of removed panes too.
                chrome.pane_targets.remove(&pane_id);
                chrome.registered_panes.remove(&pane_id);
                if pane_id == chrome.editor_pane_id {
                    // CONTRACT (connection owner): the editor host is NEVER
                    // detached — it becomes a permanent zero-size orphan so
                    // `editor_widget_id` stays editable and connection events
                    // keep applying (theme/SDUI/runtime).
                    chrome.chrome_orphans.push(host);
                } else {
                    chrome.pending_orphans.push(host);
                }
            }
        }
        let leaf_count = leaves.len();
        for pane_id in leaves {
            if let std::collections::btree_map::Entry::Vacant(slot) =
                chrome.pane_hosts.entry(pane_id)
            {
                slot.insert(
                    NewWidget::new(
                        PaneContentHost::placeholder(pane_id)
                            .with_pane_count(leaf_count)
                            .with_ui_theme(ui_theme.clone()),
                    )
                    .to_pod(),
                );
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn pane_host_ids(&self) -> Vec<(PaneId, WidgetId)> {
        self.active()
            .pane_hosts
            .iter()
            .map(|(pane_id, host)| (*pane_id, host.id()))
            .collect()
    }

    /// The pane-layout frame: full widget size minus the shell-owned tab bar
    /// row. Every consumer of pane geometry (host placement, chrome painting,
    /// hit-testing) must agree on this frame.
    fn working_area(&self, size: Size) -> Rect {
        let mut area = Rect::new(0.0, 0.0, size.width, size.height);
        if let Some(bar) = self.tab_bar_geometry(size) {
            area.y0 = bar.bar.y1;
        }
        area
    }

    /// Phase 22.1: host placement rects used by `layout()` (test accessor).
    #[cfg(test)]
    pub(crate) fn pane_host_rects(&self, size: Size) -> Vec<(PaneId, Rect)> {
        // Phase 22.3: mirrors `layout()`'s tab bar carve so observations match
        // actual host placement.
        let area = self.working_area(size);
        self.active()
            .pane_hosts
            .keys()
            .filter_map(|pane_id| {
                self.active()
                    .layout
                    .pane_slot_geometry(*pane_id, area)
                    .map(|geometry| (*pane_id, geometry.main_rect))
            })
            .collect()
    }
}

impl ClayShellWidget {
    /// Phase 22.3: paint the tab bar: panel-toned bar with a bottom hairline,
    /// token-state cards (selected fill + primary text for the active tab,
    /// list rest + muted text otherwise, hover overrides), the close glyph,
    /// and a focus ring for the (22.4-keyboard-only) focus state. The label
    /// uses the UI `Status` typography variant and is clipped to the card.
    fn paint_tab_bar(
        &mut self,
        ctx: &mut PaintCtx<'_>,
        scene: &mut Scene,
        geometry: &TabBarGeometry,
        theme: &ResolvedUiTheme,
    ) {
        let hairline = theme.dimension("dimension.border.hairline").unwrap_or(1.0);
        let bg = theme
            .color("surface.panel")
            .unwrap_or(masonry::peniko::Color::TRANSPARENT);
        scene.fill(
            masonry::vello::peniko::Fill::NonZero,
            masonry::kurbo::Affine::IDENTITY,
            bg,
            None,
            &geometry.bar,
        );
        // Bottom hairline separates the bar from the working area.
        let border = theme
            .color("border.subtle")
            .unwrap_or(masonry::peniko::Color::TRANSPARENT);
        let hairline_rect = Rect::new(
            geometry.bar.x0,
            geometry.bar.y1 - hairline,
            geometry.bar.x1,
            geometry.bar.y1,
        );
        scene.fill(
            masonry::vello::peniko::Fill::NonZero,
            masonry::kurbo::Affine::IDENTITY,
            border,
            None,
            &hairline_rect,
        );

        let radius = theme.scalar_f64("radius.xs").unwrap_or(2.0);
        let metrics = self
            .typography
            .ui_text_metrics(FontRole::Ui, UiTextVariant::Status);
        // Phase 22.7: scrolled cards must not paint into the "+" slot region
        // or off the bar's left edge — clip the card strip to the bar.
        let strip = Rect::new(
            geometry.bar.x0,
            geometry.bar.y0,
            geometry.new_tab_rect.x0,
            geometry.bar.y1,
        );
        scene.push_clip_layer(masonry::kurbo::Affine::IDENTITY, &strip);
        for (index, (card, card_geometry)) in self.tab_cards.iter().zip(&geometry.cards).enumerate()
        {
            let selected = card.client_id == self.active_tab;
            let state = if self.tab_bar_hover == Some(index) {
                InteractionState::Hover
            } else {
                InteractionState::Rest
            };
            let chrome = crate::shell::tab_card_chrome(theme, state, selected);

            let rounded = masonry::kurbo::RoundedRect::from_rect(card_geometry.rect, radius);
            scene.fill(
                masonry::vello::peniko::Fill::NonZero,
                masonry::kurbo::Affine::IDENTITY,
                chrome.fill,
                None,
                &rounded,
            );

            // Card label, clipped to the label rect.
            scene.push_clip_layer(masonry::kurbo::Affine::IDENTITY, &card_geometry.label_rect);
            paint_sdui_text(
                &self.typography,
                0.0,
                ctx,
                scene,
                &card.name,
                0,
                card_geometry.label_rect.y0
                    + (card_geometry.label_rect.height() - metrics.line_height) / 2.0,
                card_geometry.label_rect.width(),
                card_geometry.label_rect.x0,
                FontRole::Ui,
                metrics,
                chrome.text,
            );
            scene.pop_layer();

            // Close glyph (rightmost affordance; disabled when the tab has no
            // server `TabId` yet).
            if card.closable {
                Self::paint_close_glyph(scene, card_geometry.close_rect, chrome.close, hairline);
            }

            // Focus ring paints for the focus state (keyboard tab focus on the
            // bar is the 22.4 keybinding task; the resolver is state-complete).
            if chrome.focus_ring {
                paint_focus_ring(scene, card_geometry.rect, theme);
            }
        }
        scene.pop_layer();

        // New-tab affordance: reuse the same state palette as cards so the
        // pinned action has a visible hover state without relying on color
        // alone for its accessible name/action.
        let new_tab_state = if self.tab_bar_new_tab_hover {
            InteractionState::Hover
        } else {
            InteractionState::Rest
        };
        let new_tab_fill =
            crate::shell::component_state_color(theme, "surface.control", new_tab_state);
        let new_tab = masonry::kurbo::RoundedRect::from_rect(geometry.new_tab_rect, radius);
        scene.fill(
            masonry::vello::peniko::Fill::NonZero,
            masonry::kurbo::Affine::IDENTITY,
            new_tab_fill,
            None,
            &new_tab,
        );
        let new_tab_color = match new_tab_state {
            InteractionState::Rest => theme
                .color("text.muted")
                .unwrap_or(masonry::peniko::Color::TRANSPARENT),
            _ => theme
                .color("text.primary")
                .unwrap_or(masonry::peniko::Color::TRANSPARENT),
        };
        Self::paint_plus_glyph(scene, geometry.new_tab_rect, new_tab_color, hairline);
    }

    /// Phase 22.3: token-colored "+" glyph (two strokes) centered in `rect`.
    fn paint_plus_glyph(
        scene: &mut Scene,
        rect: Rect,
        color: masonry::peniko::Color,
        stroke_width: f64,
    ) {
        let inset = rect.width() * 0.3;
        let stroke = masonry::kurbo::Stroke::new(stroke_width.max(1.0));
        let affine = masonry::kurbo::Affine::IDENTITY;
        let center = rect.center();
        scene.stroke(
            &stroke,
            affine,
            color,
            None,
            &masonry::kurbo::Line::new(
                Point::new(rect.x0 + inset, center.y),
                Point::new(rect.x1 - inset, center.y),
            ),
        );
        scene.stroke(
            &stroke,
            affine,
            color,
            None,
            &masonry::kurbo::Line::new(
                Point::new(center.x, rect.y0 + inset),
                Point::new(center.x, rect.y1 - inset),
            ),
        );
    }

    /// Phase 22.3: token-colored close glyph (two crossing strokes) centered
    /// in `rect`.
    fn paint_close_glyph(
        scene: &mut Scene,
        rect: Rect,
        color: masonry::peniko::Color,
        stroke_width: f64,
    ) {
        let inset = rect.width() * 0.3;
        let stroke = masonry::kurbo::Stroke::new(stroke_width.max(1.0));
        let affine = masonry::kurbo::Affine::IDENTITY;
        let a = Point::new(rect.x0 + inset, rect.y0 + inset);
        let b = Point::new(rect.x1 - inset, rect.y1 - inset);
        scene.stroke(
            &stroke,
            affine,
            color,
            None,
            &masonry::kurbo::Line::new(a, b),
        );
        scene.stroke(
            &stroke,
            affine,
            color,
            None,
            &masonry::kurbo::Line::new(Point::new(b.x, a.y), Point::new(a.x, b.y)),
        );
    }
}

/// Phase 22.6: the shell root accessibility node's own bounds as a window
/// size, so the accessibility pass can reuse the painted tab bar geometry.
fn node_window_size(node: &Node) -> Size {
    match node.bounds() {
        Some(bounds) => Size::new(bounds.width().max(0.0), bounds.height().max(0.0)),
        None => Size::ZERO,
    }
}

/// Phase 22.6: kurbo → AccessKit rect for virtual accessibility nodes.
fn accesskit_rect(rect: Rect) -> masonry::accesskit::Rect {
    masonry::accesskit::Rect {
        x0: rect.x0,
        y0: rect.y0,
        x1: rect.x1,
        y1: rect.y1,
    }
}

/// Phase 22.6 (task 4): window-model actions that produce exactly one
/// polite live-region announcement each. One variant per user action keeps
/// the announcement strings in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnnouncementKind {
    TabActivated,
    TabCreated,
    TabClosed,
    SplitPaneVertical,
    SplitPaneHorizontal,
    PaneAdded,
    PaneClosed,
    PaneMovedForward,
    PaneMovedBackward,
}

/// Announcement length cap — the same budget constant menu labels use
/// (`src/perf/budgets.rs::TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS`).
pub(crate) const ANNOUNCEMENT_MAX_CHARS: usize = TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS;

/// Shared announcement builder: O(1) (fixed-size inputs — a display name of
/// at most 64 chars plus two counts) and sanitized — the name passes
/// `sanitize_document_display_name`, so no host path, separator, or control
/// character can reach the live region.
pub(crate) fn compose_announcement(
    kind: AnnouncementKind,
    name: Option<&str>,
    position: usize,
    count: usize,
) -> String {
    let name = name
        .map(crate::editor::accessibility::sanitize_document_display_name)
        .unwrap_or_default();
    let text = match kind {
        AnnouncementKind::TabActivated => format!("Switched to tab {position}: {name}"),
        AnnouncementKind::TabCreated => format!("Opened tab {position}: {name}"),
        AnnouncementKind::TabClosed => {
            let tabs = if count == 1 { "tab" } else { "tabs" };
            format!("Closed tab {position}: {name}; {count} {tabs} open")
        }
        AnnouncementKind::SplitPaneVertical => "Split pane vertically".to_string(),
        AnnouncementKind::SplitPaneHorizontal => "Split pane horizontally".to_string(),
        AnnouncementKind::PaneAdded => "Added pane".to_string(),
        AnnouncementKind::PaneClosed => {
            let (pane, verb) = if count == 1 {
                ("pane", "remains")
            } else {
                ("panes", "remain")
            };
            format!("Closed pane; {count} {pane} {verb}")
        }
        AnnouncementKind::PaneMovedForward => "Moved pane forward".to_string(),
        AnnouncementKind::PaneMovedBackward => "Moved pane backward".to_string(),
    };
    if text.chars().count() > ANNOUNCEMENT_MAX_CHARS {
        text.chars().take(ANNOUNCEMENT_MAX_CHARS).collect()
    } else {
        text
    }
}

impl Widget for ClayShellWidget {
    // Phase 22.2: pane-activation notifications (`PaneFocused`) flow to the
    // app driver so Masonry focus can follow pane focus.
    type Action = EditorAction;
    fn register_children(&mut self, ctx: &mut RegisterCtx<'_>) {
        for tab in self.tabs.values_mut() {
            let pane_ids: Vec<PaneId> = tab.pane_hosts.keys().copied().collect();
            for host in tab.pane_hosts.values_mut() {
                ctx.register_child(host);
            }
            // Phase 22.6: every listed host is in the Masonry tree after
            // this pass (see `registered_panes`).
            tab.registered_panes.extend(pane_ids);
            for orphan in &mut tab.pending_orphans {
                ctx.register_child(orphan);
            }
            for orphan in &mut tab.chrome_orphans {
                ctx.register_child(orphan);
            }
        }
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let size = Self::layout_size(bc);
        // Phase 22.3: the shell-owned tab bar row carves the top of the
        // working area (window-level row; it never consumes a
        // package-contributable fixed slot, which live inside each pane).
        let area = self.working_area(size);
        // Phase 22.1: place each pane host at its pane's main-slot rect; fixed
        // slots keep their Phase 20.3 geometry inside each pane.
        // Phase 22.3 + plan 086 task 3: the ACTIVE tab's hosts get their pane
        // rects; inactive tabs' hosts and pending orphans are STASHED — not
        // laid out, painted, or emitted by the accessibility walk (Masonry
        // propagates the stash through their subtrees), so the consumer never
        // sees an unattached node. Stashing is the only lever that keeps
        // inactive tabs out of the reachable a11y tree while they stay in
        // `children_ids` (which `register_children` requires). Unstashing on
        // tab activation requests layout + accessibility automatically.
        for (client_id, tab) in self.tabs.iter_mut() {
            let active = *client_id == self.active_tab;
            for (pane_id, host) in tab.pane_hosts.iter_mut() {
                if active {
                    ctx.set_stashed(host, false);
                    let Some(host_rect) = tab
                        .layout
                        .pane_slot_geometry(*pane_id, area)
                        .map(|geometry| geometry.main_rect)
                    else {
                        continue;
                    };
                    let constraints =
                        BoxConstraints::tight(Size::new(host_rect.width(), host_rect.height()));
                    ctx.run_layout(host, &constraints);
                    ctx.place_child(host, Point::new(host_rect.x0, host_rect.y0));
                } else {
                    ctx.set_stashed(host, true);
                }
            }
            for orphan in &mut tab.pending_orphans {
                ctx.set_stashed(orphan, true);
            }
            for orphan in &mut tab.chrome_orphans {
                ctx.run_layout(orphan, &BoxConstraints::tight(Size::ZERO));
                ctx.place_child(orphan, Point::ZERO);
            }
        }
        size
    }

    fn paint(&mut self, _ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        if self.tabs.is_empty() {
            return; // zero-tab shell paints nothing (invariant guard).
        }
        let area = self.working_area(_ctx.size());
        let theme = self.ui_theme.clone();

        // Phase 22.3: the shell-owned tab bar row (painted behind the working
        // area; the pane hosts are laid out below the bar so nothing overlaps).
        if let Some(geometry) = self.tab_bar_geometry(_ctx.size()) {
            self.paint_tab_bar(_ctx, scene, &geometry, &theme);
        }

        let active = self.active();
        // Split dividers paint above the pane hosts in `post_paint` (child
        // hosts repaint over anything drawn here).

        // Phase 20.3: paint fixed slot resize handles via paint_panel_chrome.
        let pane_id = active.layout.active_pane_id();
        if let Some(geometry) = active.layout.pane_slot_geometry(pane_id, area) {
            for slot in &geometry.fixed_slots {
                let handle = slot_handle_rect(slot.slot_id, slot.rect);
                let resizing = matches!(
                    &self.slot_drag,
                    SlotDragState::Resizing { slot_id, .. } if *slot_id == slot.slot_id
                );
                let chrome = PanelChrome {
                    title: None,
                    collapse: InteractionState::Rest,
                    resize: if resizing {
                        InteractionState::Active
                    } else {
                        InteractionState::Rest
                    },
                };
                paint_panel_chrome(scene, handle, &chrome, &theme);
            }
        }
    }

    fn post_paint(
        &mut self,
        _ctx: &mut PaintCtx<'_>,
        _props: &PropertiesRef<'_>,
        scene: &mut Scene,
    ) {
        if self.tabs.is_empty() {
            return; // zero-tab shell paints nothing (invariant guard).
        }
        // Pane hosts are children and repaint over `paint()` output, so the
        // split boundaries must be drawn here to stay visible — same
        // hairline treatment for side-by-side and stacked splits.
        let area = self.working_area(_ctx.size());
        let theme = self.ui_theme.clone();
        let active = self.active();
        // Paint focus after pane hosts so their backgrounds cannot cover the
        // active-pane ring. The ring is the non-color-only active/inactive
        // distinction for split panes.
        if active.layout.pane_tree().pane_count() > 1
            && let Some(focus_rect) = active.layout.focused_pane_rect(area)
        {
            paint_focus_ring(scene, focus_rect, &theme);
        }
        for divider in active.layout.pane_tree().divider_rects(area) {
            let axis = match divider.orientation {
                crate::shell::SplitOrientation::Horizontal => Axis::Vertical,
                crate::shell::SplitOrientation::Vertical => Axis::Horizontal,
            };
            paint_divider(scene, divider.line_rect, axis, &theme);
        }
    }

    fn on_text_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &TextEvent,
    ) {
        if self.tabs.is_empty() {
            return; // zero-tab shell ignores text events (invariant guard).
        }
        // Phase 20.3: Tab/Shift+Tab focus navigation across panes.
        if let TextEvent::Keyboard(keyboard) = event
            && keyboard.state == KeyState::Down
            && keyboard.key == Key::Named(NamedKey::Tab)
            && self.active().layout.pane_tree().pane_count() > 1
        {
            let pane_id = if keyboard.modifiers.shift() {
                self.active_mut().layout.focus_prev_pane()
            } else {
                self.active_mut().layout.focus_next_pane()
            };
            // Phase 22.2: the driver moves Masonry focus to the new active
            // pane's content widget (keyboard routing follows pane focus).
            self.submit_pane_focused(ctx, pane_id);
            ctx.request_layout();
            ctx.request_paint_only();
        }
    }

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        if self.tabs.is_empty() {
            return; // zero-tab shell ignores pointer events (invariant guard).
        }
        let area = self.working_area(ctx.size());
        let pane_id = self.active().layout.active_pane_id();

        match event {
            PointerEvent::Down(button_event)
                if button_event.button == Some(PointerButton::Primary) =>
            {
                let point = ctx.local_position(button_event.state.position);

                // Phase 22.3: tab bar clicks (the bar sits above the working
                // area, so it wins first). A card click activates that tab
                // (the driver switches optimistically; the server registry is
                // the reconciling authority). A close-glyph click submits the
                // close action when the tab is closable; the driver guards
                // close-on-last-tab. Clicks on the active card or a disabled
                // close are consumed as no-ops.
                if let Some(geometry) = self.tab_bar_geometry(ctx.size()) {
                    // New-tab affordance (right edge of the bar).
                    if geometry.new_tab_rect.contains(point) {
                        ctx.submit_action::<EditorAction>(EditorAction::TabBar(
                            crate::masonry_editor::TabBarAction::NewTab,
                        ));
                        return;
                    }
                    let Some((index, hit_close)) = self.tab_bar_hit_test(&geometry, point) else {
                        return;
                    };
                    let card = self.tab_cards[index].clone();
                    if hit_close {
                        if card.closable {
                            ctx.submit_action::<EditorAction>(EditorAction::TabBar(
                                crate::masonry_editor::TabBarAction::Close {
                                    client_id: card.client_id,
                                },
                            ));
                        }
                    } else if card.client_id != self.active_tab {
                        ctx.submit_action::<EditorAction>(EditorAction::TabBar(
                            crate::masonry_editor::TabBarAction::Activate {
                                client_id: card.client_id,
                            },
                        ));
                    }
                    return;
                }

                // Check slot handles first (they render on top of split dividers).
                if let Some(slot_id) = self
                    .active()
                    .layout
                    .pane_slot_geometry(pane_id, area)
                    .as_ref()
                    .and_then(|g| hit_test_slot_handle(g, point))
                {
                    let now = std::time::Instant::now();
                    // Double-click detection: same slot within 300ms → toggle collapse.
                    let is_double = self.last_slot_click.is_some_and(|(t, id)| {
                        id == slot_id && now.duration_since(t).as_millis() < 300
                    });
                    self.last_slot_click = Some((now, slot_id));

                    if is_double {
                        self.active_mut()
                            .layout
                            .toggle_slot_collapse(pane_id, slot_id);
                        self.last_slot_click = None;
                        self.persist_debounced(ctx);
                        ctx.request_layout();
                        return;
                    }

                    // Start resize drag.
                    let original_size = self
                        .active_mut()
                        .layout
                        .slot_layout_mut(pane_id)
                        .and_then(|l| l.fixed_slot_mut(slot_id))
                        .map(|s| s.size)
                        .unwrap_or(0.0);
                    self.slot_drag = SlotDragState::Resizing {
                        slot_id,
                        pane_id,
                        original_size,
                    };
                    ctx.capture_pointer();
                    return;
                }

                // Check split dividers.
                if let Some(hit) =
                    hit_test_split_divider(self.active().layout.pane_tree(), area, point)
                {
                    let original_ratio = self
                        .active()
                        .layout
                        .pane_tree()
                        .split_ratio_at_path(&hit.path)
                        .unwrap_or_else(crate::shell::SplitRatio::balanced);
                    self.split_drag = SplitDragState::Dragging {
                        path: hit.path,
                        orientation: hit.orientation,
                        parent_rect: hit.parent_rect,
                        original_ratio,
                    };
                    ctx.capture_pointer();
                } else if self.pane_focus_policy() == PaneFocusPolicy::ClickToFocus
                    && self.active().layout.pane_tree().pane_count() > 1
                    && let Some(pane_id) = self
                        .active()
                        .layout
                        .pane_tree()
                        .compute_geometry(area)
                        .iter()
                        .find(|pane| pane.rect.contains(point))
                        .map(|pane| pane.pane_id)
                    && pane_id != self.active().layout.active_pane_id()
                {
                    // Phase 22.1: click-to-focus. Placeholder hosts do not consume
                    // pointer-down, so clicks inside an inactive pane bubble here.
                    // (Editor panes activate via `Update::FocusChanged` actions.)
                    let _ = self.active_mut().layout.set_focus_pane(pane_id);
                    self.submit_pane_focused(ctx, pane_id);
                    ctx.request_render();
                }
            }
            PointerEvent::Move(pointer_update) => {
                let point = ctx.local_position(pointer_update.current.position);

                // Slot resize live preview.
                if let SlotDragState::Resizing {
                    slot_id, pane_id, ..
                } = &self.slot_drag
                {
                    let slot_id = *slot_id;
                    let pane_id = *pane_id;
                    if let Some(pane_rect) =
                        self.active().layout.pane_tree().pane_rect(pane_id, area)
                    {
                        let new_size = compute_slot_resize_size(slot_id, pane_rect, point);
                        self.active_mut()
                            .layout
                            .resize_slot_live(pane_id, slot_id, new_size);
                        ctx.request_layout();
                    }
                    return;
                }

                // Split divider drag live preview.
                if let SplitDragState::Dragging {
                    path,
                    orientation,
                    parent_rect,
                    ..
                } = &self.split_drag
                {
                    let path = path.clone();
                    let ratio = crate::shell::compute_drag_ratio(*orientation, *parent_rect, point);
                    self.active_mut()
                        .layout
                        .pane_tree_mut()
                        .update_split_ratio(&path, ratio);
                    ctx.request_layout();
                }

                // Phase 22.3: tab bar hover tracking (state paint). The bar is
                // above the working area, so a hovered card and a hovered pane
                // are mutually exclusive.
                let geometry = self.tab_bar_geometry(ctx.size());
                let hover = geometry
                    .as_ref()
                    .and_then(|geometry| self.tab_bar_hit_test(geometry, point))
                    .map(|(index, _)| index);
                let new_tab_hover = geometry
                    .as_ref()
                    .is_some_and(|geometry| geometry.new_tab_rect.contains(point));
                if hover != self.tab_bar_hover || new_tab_hover != self.tab_bar_new_tab_hover {
                    self.tab_bar_hover = hover;
                    self.tab_bar_new_tab_hover = new_tab_hover;
                    ctx.request_paint_only();
                }

                // Phase 22.1: focus follows cursor. Skipped during divider/slot
                // drags to avoid focus churn mid-gesture.
                if self.pane_focus_policy() == PaneFocusPolicy::FollowsCursor
                    && self.split_drag == SplitDragState::Idle
                    && self.slot_drag == SlotDragState::Idle
                    && self.active().layout.pane_tree().pane_count() > 1
                    && let Some(hover_pane) = self
                        .active()
                        .layout
                        .pane_tree()
                        .compute_geometry(area)
                        .iter()
                        .find(|pane| pane.rect.contains(point))
                        .map(|pane| pane.pane_id)
                    && hover_pane != self.active().layout.active_pane_id()
                {
                    let _ = self.active_mut().layout.set_focus_pane(hover_pane);
                    self.submit_pane_focused(ctx, hover_pane);
                    ctx.request_render();
                }
            }
            PointerEvent::Scroll(scroll_event) => {
                // Phase 22.7 (D6/F5): wheel over the tab bar scrolls the
                // strip horizontally (vertical wheel scrolls too, matching
                // tab-strip convention in browsers). No-op when the bar is
                // hidden or the strip fits.
                let point = ctx.local_position(scroll_event.state.position);
                let Some(geometry) = self.tab_bar_geometry(ctx.size()) else {
                    return;
                };
                if !geometry.bar.contains(point) {
                    return;
                }
                let (x, y) = match scroll_event.delta {
                    masonry::core::ScrollDelta::LineDelta(x, y) => (
                        (x as f64) * TAB_BAR_SCROLL_STEP,
                        (y as f64) * TAB_BAR_SCROLL_STEP,
                    ),
                    // Pixel deltas are already in pixels — no line multiplier.
                    masonry::core::ScrollDelta::PixelDelta(p) => (p.x, p.y),
                    // Synthetic page deltas (scrollbar wells) are not wheel input.
                    masonry::core::ScrollDelta::PageDelta(..) => return,
                };
                self.tab_bar_scroll = (self.tab_bar_scroll + x + y).clamp(0.0, geometry.scroll_max);
                ctx.request_paint_only();
                ctx.request_accessibility_update();
            }
            PointerEvent::Up(_) => {
                // Commit slot resize.
                if let SlotDragState::Resizing {
                    slot_id, pane_id, ..
                } = &self.slot_drag
                {
                    let slot_id = *slot_id;
                    let pane_id = *pane_id;
                    self.active_mut()
                        .layout
                        .commit_slot_resize(pane_id, slot_id);
                    self.slot_drag = SlotDragState::Idle;
                    self.persist_debounced(ctx);
                    ctx.release_pointer();
                    ctx.request_layout();
                    return;
                }

                // Commit split drag.
                if let SplitDragState::Dragging { path, .. } = &self.split_drag {
                    let path = path.clone();
                    if let Some(ratio) = self.active().layout.pane_tree().split_ratio_at_path(&path)
                    {
                        self.active_mut().layout.commit_split_drag(&path, ratio);
                    }
                    self.split_drag = SplitDragState::Idle;
                    self.persist_debounced(ctx);
                    ctx.release_pointer();
                    ctx.request_layout();
                }
            }
            PointerEvent::Cancel(_) => {
                // Cancel slot resize.
                if let SlotDragState::Resizing {
                    slot_id,
                    pane_id,
                    original_size,
                } = &self.slot_drag
                {
                    let slot_id = *slot_id;
                    let pane_id = *pane_id;
                    let original_size = *original_size;
                    self.active_mut()
                        .layout
                        .cancel_slot_resize(pane_id, slot_id, original_size);
                    self.slot_drag = SlotDragState::Idle;
                    ctx.release_pointer();
                    ctx.request_layout();
                    return;
                }

                // Cancel split drag.
                if let SplitDragState::Dragging {
                    path,
                    original_ratio,
                    ..
                } = &self.split_drag
                {
                    let path = path.clone();
                    let original_ratio = *original_ratio;
                    self.active_mut()
                        .layout
                        .cancel_split_drag(&path, original_ratio);
                    self.split_drag = SplitDragState::Idle;
                    ctx.release_pointer();
                    ctx.request_layout();
                }
            }
            _ => {}
        }
    }

    fn update(
        &mut self,
        _ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _event: &Update,
    ) {
        // Phase 22.2: pane activation follows Masonry focus via `PaneFocused`
        // actions submitted by the chrome (pane 1) and the pane views, plus the
        // shell's own programmatic focus moves above. The old `ChildFocusChanged`
        // hack is gone: it could not attribute focus to a specific pane.
    }

    fn accessibility_role(&self) -> Role {
        Role::Group
    }

    fn accessibility(
        &mut self,
        ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        node: &mut Node,
    ) {
        let mut children: Vec<NodeId> = if self.tabs.is_empty() {
            // Zero-tab shell: no active pane to name; the polite
            // announcement node still registers below.
            node.set_label("Clay working area shell. No mounted tabs.");
            Vec::new()
        } else {
            node.set_label(format!(
                "Clay working area shell. Active pane {}.",
                self.active().layout.active_pane_id().0
            ));
            self.active()
                .pane_hosts
                .values()
                .map(|host| host.id().into())
                .collect()
        };
        // Phase 22.6: the accessibility tree exposes only the mounted tab's
        // panes. Inactive tabs' hosts stay in the arena at zero size (the
        // `children_ids` contract below) for reconnect continuity but are
        // never walked, painted, or announced. The tab bar (visible with 2+
        // tabs) is exposed as a TabList of Tab nodes with sanitized workspace
        // names and the active card selected; tab operations stay
        // keyboard-command-driven (Phase 22.4), so the tab nodes are
        // informational, matching the status-line precedent.
        if self.tab_cards.len() >= 2
            && let Some(geometry) = self.tab_bar_geometry(node_window_size(node))
        {
            let list_id = crate::editor::accessibility::virtual_a11y_node_id(
                ctx.widget_id(),
                crate::editor::accessibility::virtual_a11y_slots::SHELL_TAB_LIST,
            );
            let mut list = Node::new(Role::TabList);
            list.set_label("Workspace tabs");
            list.set_bounds(accesskit_rect(geometry.bar));
            let mut tab_ids = Vec::with_capacity(geometry.cards.len());
            for (card, card_geometry) in self.tab_cards.iter().zip(&geometry.cards) {
                // Slot derives from the connection id so a card keeps its ID
                // across reorders and selection changes; client ids are
                // bounded by `MAX_ACTIVE_CONNECTIONS` (64), far below the
                // 9-bit slot space.
                let tab_id = crate::editor::accessibility::virtual_a11y_node_id(
                    ctx.widget_id(),
                    crate::editor::accessibility::virtual_a11y_slots::SHELL_TAB_BASE
                        + u16::try_from(card.client_id)
                            .expect("tab client id exceeds u16 slot space"),
                );
                let mut tab = Node::new(Role::Tab);
                tab.set_label(
                    crate::editor::accessibility::sanitize_document_display_name(&card.name),
                );
                tab.set_selected(card.client_id == self.active_tab);
                tab.set_bounds(accesskit_rect(card_geometry.rect));
                ctx.tree_update().nodes.push((tab_id, tab));
                tab_ids.push(tab_id);
            }
            list.set_children(tab_ids);
            ctx.tree_update().nodes.push((list_id, list));
            children.insert(0, list_id);
        }
        // Phase 22.6 (task 4): the polite live-region announcement node is
        // always present so assistive technologies can register it; only
        // its label changes per window-model action. Empty until the first
        // action. `ponytail:` ceiling — an AT may skip an announcement whose
        // label equals the previous one; a two-phase clear+set is the
        // upgrade path if repeated identical actions must re-announce.
        let mut announce = Node::new(Role::Status);
        announce.set_live(Live::Polite);
        if let Some(text) = &self.announcement {
            announce.set_label(text.as_str());
        }
        let announce_id = crate::editor::accessibility::virtual_a11y_node_id(
            ctx.widget_id(),
            crate::editor::accessibility::virtual_a11y_slots::SHELL_ANNOUNCEMENT,
        );
        ctx.tree_update().nodes.push((announce_id, announce));
        children.push(announce_id);
        node.set_children(children);
    }

    fn children_ids(&self) -> ChildrenIds {
        let mut ids: Vec<WidgetId> = Vec::new();
        for tab in self.tabs.values() {
            ids.extend(tab.pane_hosts.values().map(|host| host.id()));
            ids.extend(tab.pending_orphans.iter().map(|orphan| orphan.id()));
        }
        ChildrenIds::from_slice(&ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{
        ClientConnectionEvent, ClientEditQueue, ClientInitialState, ClientResyncSnapshot,
    };
    use crate::masonry_editor::{EditorWidget, TabBarAction};
    use crate::masonry_pane_document::PaneDocumentView;
    use crate::protocol::{
        BehaviorManifest, ClientMessage, DocumentAccess, DocumentMetadata, EditOperation,
        SduiEditorBinding, SduiFlexDirection, SduiNode, SduiNodeId, SduiNodeKind, SduiTree,
    };
    use masonry::app::{RenderRoot, RenderRootOptions, RenderRootSignal, WindowSizePolicy};
    use masonry::core::keyboard::{Code, Key, KeyState, KeyboardEvent, NamedKey};
    use masonry::core::{Ime, PointerButtonEvent, TextEvent};
    use masonry::dpi::PhysicalSize;
    use masonry::theme::default_property_set;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn initial_state(access: DocumentAccess, version: u64) -> ClientInitialState {
        ClientInitialState {
            client_id: 11,
            document_id: 7,
            document_version: version,
            text: "server text".to_string(),
            access,
            behavior_manifest: BehaviorManifest::minimal_text_editing(3),
            active_theme: crate::protocol::ActiveTheme {
                specifier: "@clay/default".to_string(),
                overrides: Vec::new(),
                design_tokens: Vec::new(),
            },
            active_typography: crate::protocol::ActiveTypography::default(),
            workspace_root: "/tmp/root".to_string(),
        }
    }

    fn sdui_tree(label_text: &str) -> SduiTree {
        SduiTree {
            ui_version: 1,
            root_id: SduiNodeId(1),
            nodes: vec![
                SduiNode::new(
                    SduiNodeId(1),
                    SduiNodeKind::Flex {
                        direction: SduiFlexDirection::Row,
                        children: vec![SduiNodeId(2), SduiNodeId(3)],
                    },
                ),
                SduiNode::new(
                    SduiNodeId(2),
                    SduiNodeKind::Label {
                        text: label_text.to_string(),
                    },
                ),
                SduiNode::new(
                    SduiNodeId(3),
                    SduiNodeKind::EditorView {
                        binding: SduiEditorBinding {
                            document_id: 7,
                            expected_version: Some(3),
                        },
                    },
                ),
            ],
        }
    }

    fn render_root_options() -> RenderRootOptions {
        RenderRootOptions {
            default_properties: default_property_set().into(),
            use_system_fonts: false,
            size_policy: WindowSizePolicy::User,
            size: PhysicalSize::new(900, 600),
            scale_factor: 1.0,
            test_font: None,
        }
    }

    fn render_root_for_shell(editor: EditorWidget) -> (RenderRoot, WidgetId) {
        let shell = ClayShellWidget::single_editor(0, editor);
        let editor_widget_id = shell.editor_widget_id();
        let mut render_root = RenderRoot::new(NewWidget::new(shell), |_| {}, render_root_options());

        assert!(render_root.has_widget(editor_widget_id));
        assert!(render_root.set_focus_fallback(Some(editor_widget_id)));
        assert!(render_root.focus_on(Some(editor_widget_id)));
        assert_eq!(render_root.focused_widget(), Some(editor_widget_id));

        (render_root, editor_widget_id)
    }

    fn with_shell_editor<R>(
        render_root: &mut RenderRoot,
        editor_widget_id: WidgetId,
        f: impl FnOnce(&mut EditorWidget) -> R,
    ) -> R {
        render_root.edit_widget(editor_widget_id, |mut widget| {
            let editor = widget
                .try_downcast::<EditorWidget>()
                .expect("shell editor child downcasts to EditorWidget");
            f(editor.widget)
        })
    }

    #[test]
    fn shell_observable_snapshot_captures_default_working_area() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());

        let observation = shell.observable_snapshot(Size::new(900.0, 600.0));

        assert_eq!(observation.layout.layout_version, ShellLayoutVersion(1));
        assert_eq!(observation.layout.working_area_id, WorkingAreaId(1));
        assert_eq!(observation.layout.root_pane_id, PaneId(1));
        assert_eq!(observation.layout.active_pane_id, PaneId(1));
        assert_eq!(observation.layout.editor_component.id, ShellComponentId(1));
        assert_eq!(
            observation.layout.editor_component.kind,
            ShellComponentKind::Editor
        );
        assert_eq!(observation.layout.editor_component.pane_id, PaneId(1));
        assert!(matches!(
            observation.layout.pane_tree,
            PaneTreeObservation::Leaf { pane_id: PaneId(1) }
        ));
        assert_eq!(observation.layout.pane_count, 1);
        assert_eq!(observation.layout.split_count, 0);
        assert_eq!(observation.layout.slots.len(), 1);
        assert_eq!(observation.layout.slots[0].slot_id, PaneSlotId::Main);
        assert!(observation.layout.editor_region_non_empty);
        assert!(observation.editor_component_bound);
        assert!(observation.sdui_state_present);
        assert!(observation.status_present);
    }

    #[test]
    fn shell_root_registers_editor_child_and_focus_fallback() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());

        // Phase 22.1: the shell's direct child is the pane content host; the
        // editor is nested inside the editor pane's host.
        let host_ids: Vec<WidgetId> = shell.children_ids().iter().copied().collect();
        assert_eq!(host_ids.len(), 1);
        assert_ne!(host_ids[0], shell.editor_widget_id());
        assert_eq!(shell.focus_fallback_widget_id(), shell.editor_widget_id());
    }

    #[test]
    fn shell_editor_text_input_remains_client_first() {
        let (queue, mut receiver) = ClientEditQueue::bounded(4);
        let queue = queue
            .with_authority(11, &DocumentAccess::Editable { lease_id: 1 })
            .with_confirmed_version(3);
        let editor = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            3,
        ))
        .with_edit_queue(queue);
        let (mut render_root, editor_widget_id) = render_root_for_shell(editor);

        render_root.handle_text_event(TextEvent::Ime(Ime::Commit("!".to_string())));

        let visible_text = with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            editor.visible_text_for_test()
        });
        assert_eq!(visible_text, "!server text");
        assert_eq!(
            receiver.try_recv().expect("local edit is queued"),
            ClientMessage::Edit {
                document_id: 7,
                client_id: 11,
                lease_id: Some(1),
                base_version: 3,
                behavior_version: 3,
                transaction_id: 1,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "!".to_string(),
                },
            }
        );
    }

    #[test]
    fn shell_editor_keyboard_routing_uses_installed_behavior_manifest() {
        let (queue, mut receiver) = ClientEditQueue::bounded(4);
        let queue = queue
            .with_authority(11, &DocumentAccess::Editable { lease_id: 1 })
            .with_confirmed_version(3);
        let editor = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            3,
        ))
        .with_edit_queue(queue);
        let (mut render_root, editor_widget_id) = render_root_for_shell(editor);

        render_root.handle_text_event(TextEvent::Keyboard(KeyboardEvent {
            state: KeyState::Down,
            key: Key::Named(NamedKey::Enter),
            code: Code::Enter,
            ..KeyboardEvent::default()
        }));

        let visible_text = with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            editor.visible_text_for_test()
        });
        assert_eq!(visible_text, "\nserver text");
        assert!(matches!(
            receiver.try_recv().expect("manifest-routed edit is queued"),
            ClientMessage::Edit {
                operation: EditOperation::Insert { byte_offset: 0, text },
                behavior_version: 3,
                ..
            } if text == "\n"
        ));
    }

    #[test]
    fn shell_editor_read_only_observer_blocks_local_edit_queue() {
        let (queue, mut receiver) = ClientEditQueue::bounded(4);
        let queue = queue
            .with_authority(11, &DocumentAccess::ReadOnly)
            .with_confirmed_version(3);
        let editor = EditorWidget::with_initial_state(initial_state(DocumentAccess::ReadOnly, 3))
            .with_edit_queue(queue);
        let (mut render_root, editor_widget_id) = render_root_for_shell(editor);

        render_root.handle_text_event(TextEvent::Ime(Ime::Commit("!".to_string())));

        let visible_text = with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            editor.visible_text_for_test()
        });
        assert_eq!(visible_text, "server text");
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn shell_routes_edit_ack_and_resync_to_editor() {
        let editor = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            3,
        ));
        let (mut render_root, editor_widget_id) = render_root_for_shell(editor);

        with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            assert!(
                editor.apply_connection_event(ClientConnectionEvent::EditAck {
                    document_id: 7,
                    version: 4,
                    transaction_id: 1,
                })
            );
        });
        let status_after_ack = with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            editor.status_text()
        });
        assert_eq!(status_after_ack, "Clay — Connected — Editable — doc 7 — v4");

        with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            assert!(
                editor.apply_connection_event(ClientConnectionEvent::ResyncSnapshot(
                    ClientResyncSnapshot {
                        document_id: 7,
                        version: 9,
                        text: "server resync".to_string(),
                        access: DocumentAccess::ReadOnly,
                        lease_id: None,
                    },
                ))
            );
        });
        let (visible_text, status_after_resync) =
            with_shell_editor(&mut render_root, editor_widget_id, |editor| {
                (editor.visible_text_for_test(), editor.status_text())
            });
        assert_eq!(visible_text, "server resync");
        assert_eq!(
            status_after_resync,
            "Clay — Connected — Read-only Observer — doc 7 — v9"
        );
    }

    #[test]
    fn shell_routes_sdui_snapshots_to_editor_component() {
        let editor = EditorWidget::with_initial_state(initial_state(
            DocumentAccess::Editable { lease_id: 1 },
            3,
        ));
        let (mut render_root, editor_widget_id) = render_root_for_shell(editor);

        with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            assert!(
                editor.apply_connection_event(ClientConnectionEvent::SduiSnapshot {
                    client_id: 11,
                    tree: sdui_tree("Ready"),
                })
            );
        });

        let visible_texts = with_shell_editor(&mut render_root, editor_widget_id, |editor| {
            editor.sdui_visible_texts()
        });
        assert!(visible_texts.contains(&"Ready".to_string()));
    }

    #[test]
    fn shell_root_delegates_connection_events_to_editor_component() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());
        let layout = shell.working_area_layout();
        let observation = shell.observable_snapshot(Size::new(900.0, 600.0));

        assert_eq!(layout.editor_component().kind, ShellComponentKind::Editor);
        assert_eq!(layout.editor_component().pane_id, layout.active_pane_id());
        assert_eq!(
            observation.layout.editor_component.pane_id,
            layout.editor_component().pane_id
        );
        assert!(observation.editor_component_bound);
    }

    #[test]
    fn shell_places_editor_child_in_main_slot_rect() {
        let slot_layout = PaneSlotLayout::main_only()
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Left, 240.0, 120.0, 320.0).unwrap())
            .with_fixed_slot(FixedSlotState::new(FixedSlotId::Bottom, 80.0, 40.0, 120.0).unwrap());
        let layout = WorkingAreaLayout::single_editor().with_editor_pane_slot_layout(slot_layout);
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);

        let editor_rect = shell.editor_component_rect_for_size(Size::new(900.0, 600.0));

        assert_eq!(editor_rect, Rect::new(240.0, 0.0, 900.0, 520.0));
        // Phase 22.1: the host placement follows the same main-slot rect.
        assert_eq!(
            shell.pane_host_rects(Size::new(900.0, 600.0)),
            vec![(PaneId(1), Rect::new(240.0, 0.0, 900.0, 520.0))]
        );
        assert_eq!(shell.children_ids().iter().count(), 1);
    }

    #[test]
    fn shell_observable_snapshot_captures_split_and_slots() {
        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::new(0.25).unwrap(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(2),
        )
        .unwrap();
        let mut layout = WorkingAreaLayout::with_pane_tree(tree, PaneId(2)).unwrap();
        layout
            .apply_update(WorkingAreaLayoutUpdate {
                base_version: ShellLayoutVersion(1),
                working_area_id: WorkingAreaId(1),
                pane_tree: PaneSplitTree::new(
                    PaneSplitNode::split(
                        SplitOrientation::Horizontal,
                        SplitRatio::new(0.25).unwrap(),
                        PaneSplitNode::leaf(PaneId(1)),
                        PaneSplitNode::leaf(PaneId(2)),
                    ),
                    PaneId(2),
                )
                .unwrap(),
                editor_pane_id: PaneId(2),
                pane_slots: vec![PaneSlotLayoutAssignment {
                    pane_id: PaneId(2),
                    layout: PaneSlotLayout::main_only().with_fixed_slot(
                        FixedSlotState::new(FixedSlotId::Left, 120.0, 80.0, 200.0).unwrap(),
                    ),
                }],
            })
            .unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);

        let snapshot = shell.observable_snapshot(Size::new(1000.0, 600.0));

        assert_eq!(snapshot.layout.layout_version, ShellLayoutVersion(2));
        assert_eq!(snapshot.layout.active_pane_id, PaneId(2));
        assert_eq!(snapshot.layout.editor_component.pane_id, PaneId(2));
        assert_eq!(snapshot.layout.pane_count, 2);
        assert_eq!(snapshot.layout.split_count, 1);
        assert!(matches!(
            snapshot.layout.pane_tree,
            PaneTreeObservation::Split { .. }
        ));
        assert!(snapshot.layout.slots.iter().any(|slot| {
            slot.pane_id == PaneId(2)
                && slot.slot_id == PaneSlotId::Left
                && slot.visible
                && slot.rect == Rect::new(250.0, 0.0, 370.0, 600.0)
        }));
        assert!(snapshot.layout.slots.iter().any(|slot| {
            slot.pane_id == PaneId(2)
                && slot.slot_id == PaneSlotId::Main
                && slot.rect == Rect::new(370.0, 0.0, 1000.0, 600.0)
        }));
    }

    #[test]
    fn shell_observation_does_not_expose_document_text_or_native_handles() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());

        let snapshot = shell.observable_snapshot(Size::new(900.0, 600.0));
        let debug = format!("{snapshot:?}");

        assert!(!debug.contains("hello from a document"));
        assert!(!debug.contains("WidgetId"));
        assert!(!debug.contains("Deno.core.ops"));
        assert!(!debug.contains("raw_css"));
    }

    #[test]
    fn shell_layout_update_rejects_stale_or_oversize_payload() {
        let mut shell = ClayShellWidget::single_editor(0, EditorWidget::default());

        assert!(matches!(
            shell.apply_layout_update(WorkingAreaLayoutUpdate {
                base_version: ShellLayoutVersion(0),
                working_area_id: WorkingAreaId(1),
                pane_tree: PaneSplitTree::default(),
                editor_pane_id: PaneId(1),
                pane_slots: Vec::new(),
            }),
            Err(WorkingAreaLayoutUpdateError::StaleVersion { .. })
        ));

        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(1),
        )
        .unwrap();
        assert!(matches!(
            shell.apply_layout_update(WorkingAreaLayoutUpdate {
                base_version: ShellLayoutVersion(1),
                working_area_id: WorkingAreaId(1),
                pane_tree: tree,
                editor_pane_id: PaneId(1),
                pane_slots: vec![
                    PaneSlotLayoutAssignment {
                        pane_id: PaneId(1),
                        layout: PaneSlotLayout::main_only(),
                    },
                    PaneSlotLayoutAssignment {
                        pane_id: PaneId(2),
                        layout: PaneSlotLayout::main_only(),
                    },
                    PaneSlotLayoutAssignment {
                        pane_id: PaneId(3),
                        layout: PaneSlotLayout::main_only(),
                    },
                ],
            }),
            Err(WorkingAreaLayoutUpdateError::TooManyPaneSlotLayouts { .. })
        ));
    }

    #[test]
    fn pane_split_tree_layout_does_not_mutate_children() {
        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(1),
        )
        .unwrap();
        let layout = WorkingAreaLayout::with_pane_tree(tree, PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);

        let child_ids_before: Vec<_> = shell.children_ids().iter().copied().collect();
        let editor_rect = shell.editor_component_rect_for_size(Size::new(1000.0, 600.0));
        let child_ids_after: Vec<_> = shell.children_ids().iter().copied().collect();

        assert_eq!(editor_rect, Rect::new(0.0, 0.0, 500.0, 600.0));
        assert_eq!(child_ids_after, child_ids_before);
        // Phase 22.1: one host per pane leaf; hosts wrap the editor, so their
        // ids differ from the editor's own id.
        let host_ids: Vec<WidgetId> = shell.pane_host_ids().iter().map(|(_, id)| *id).collect();
        assert_eq!(host_ids.len(), 2);
        assert_eq!(child_ids_after, host_ids);
        assert!(!child_ids_after.contains(&shell.editor_widget_id()));
    }

    // -- Phase 22.1: multi-pane hosting --

    fn two_pane_tree() -> PaneSplitTree {
        PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(1),
        )
        .unwrap()
    }

    #[test]
    fn shell_hosts_placeholder_for_non_editor_pane_leaves() {
        let layout = WorkingAreaLayout::with_pane_tree(two_pane_tree(), PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);
        let editor_widget_id = shell.editor_widget_id();
        let hosts = shell.pane_host_ids();
        assert_eq!(hosts.len(), 2);

        let mut render_root = RenderRoot::new(NewWidget::new(shell), |_| {}, render_root_options());
        assert!(render_root.has_widget(editor_widget_id));

        // Pane 1's host wraps the editor; pane 2's host is an inert placeholder.
        let pane1_host = hosts[0].1;
        render_root.edit_widget(pane1_host, |mut widget| {
            let host = widget
                .try_downcast::<PaneContentHost>()
                .expect("pane host downcasts");
            assert!(!host.widget.is_placeholder());
            assert_eq!(host.widget.editor_widget_id(), Some(editor_widget_id));
        });
        let pane2_host = hosts[1].1;
        render_root.edit_widget(pane2_host, |mut widget| {
            let host = widget
                .try_downcast::<PaneContentHost>()
                .expect("pane host downcasts");
            assert!(host.widget.is_placeholder());
            assert_eq!(host.widget.editor_widget_id(), None);
        });
    }

    #[test]
    fn set_active_theme_stamps_placeholder_hosts_for_new_splits() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        let _ = render_root.redraw();

        let themed = crate::protocol::ActiveTheme {
            specifier: "@clay/theme-modus-operandi".to_string(),
            overrides: Vec::new(),
            design_tokens: vec![crate::protocol::UiDesignTokenOverride {
                token: "surface.panel".to_string(),
                value: crate::protocol::WireDesignTokenValue::Color([0xff, 0xff, 0xff, 0xff]),
                provenance: "test".to_string(),
            }],
        };

        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_active_theme(&mut shell.ctx, &themed);
        });
        dispatch_shell_command(
            &mut render_root,
            shell_id,
            ShellClientCommand::SplitPaneHorizontal,
        );

        let hosts = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_ids()
        });
        assert_eq!(hosts.len(), 2);
        render_root.edit_widget(hosts[1].1, |mut widget| {
            let host = widget.try_downcast::<PaneContentHost>().expect("host");
            assert!(host.widget.is_placeholder());
            assert_eq!(
                host.widget.placeholder_background(),
                masonry::peniko::Color::from_rgb8(0xff, 0xff, 0xff)
            );
        });
    }

    #[test]
    fn split_placeholder_seeds_theme_from_editor_at_construction() {
        // The editor arrives with the active theme already installed
        // (handshake initial state); the shell seeds its chrome theme from
        // it so a split placeholder follows the theme without any theme
        // event (the startup handshake theme never arrives as one).
        let mut state = initial_state(DocumentAccess::Editable { lease_id: 99 }, 1);
        state.active_theme = crate::protocol::ActiveTheme {
            specifier: "@clay/theme-modus-operandi".to_string(),
            overrides: Vec::new(),
            design_tokens: vec![crate::protocol::UiDesignTokenOverride {
                token: "surface.panel".to_string(),
                value: crate::protocol::WireDesignTokenValue::Color([0x11, 0x22, 0x33, 0xff]),
                provenance: "test".to_string(),
            }],
        };
        let shell = ClayShellWidget::single_editor(0, EditorWidget::with_initial_state(state));
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        let _ = render_root.redraw();

        dispatch_shell_command(
            &mut render_root,
            shell_id,
            ShellClientCommand::SplitPaneHorizontal,
        );

        let hosts = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_ids()
        });
        assert_eq!(hosts.len(), 2);
        render_root.edit_widget(hosts[1].1, |mut widget| {
            let host = widget.try_downcast::<PaneContentHost>().expect("host");
            assert!(host.widget.is_placeholder());
            assert_eq!(
                host.widget.placeholder_background(),
                masonry::peniko::Color::from_rgb8(0x11, 0x22, 0x33)
            );
        });
    }

    #[test]
    fn shell_places_each_pane_host_at_its_main_slot_rect() {
        let layout = WorkingAreaLayout::with_pane_tree(two_pane_tree(), PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);

        assert_eq!(
            shell.pane_host_rects(Size::new(1000.0, 600.0)),
            vec![
                (PaneId(1), Rect::new(0.0, 0.0, 500.0, 600.0)),
                (PaneId(2), Rect::new(500.0, 0.0, 1000.0, 600.0)),
            ]
        );
    }

    #[test]
    fn reconcile_pane_hosts_keeps_surviving_host_ids_stable() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        let _ = render_root.redraw();

        let host_before = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_ids()[0].1
        });

        // Split the working area into two panes and reconcile.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            let base_version = shell.widget.working_area_layout().version();
            shell
                .widget
                .apply_layout_update(WorkingAreaLayoutUpdate {
                    base_version,
                    working_area_id: WorkingAreaId(1),
                    pane_tree: two_pane_tree(),
                    editor_pane_id: PaneId(1),
                    pane_slots: Vec::new(),
                })
                .unwrap();
            shell.widget.reconcile_pane_hosts(&mut shell.ctx);
        });
        let _ = render_root.redraw(); // register pass must not panic

        let hosts_after = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_ids()
        });

        assert_eq!(hosts_after.len(), 2);
        assert_eq!(hosts_after[0], (PaneId(1), host_before)); // stable identity
        assert!(render_root.has_widget(hosts_after[1].1)); // new host registered
    }

    #[test]
    fn reconcile_pane_hosts_detaches_closed_pane_hosts() {
        let layout = WorkingAreaLayout::with_pane_tree(two_pane_tree(), PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        let _ = render_root.redraw();

        let (host_keep, host_drop) = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            let hosts = shell.widget.pane_host_ids();
            (hosts[0].1, hosts[1].1)
        });

        // Collapse back to a single pane and reconcile.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            let base_version = shell.widget.working_area_layout().version();
            shell
                .widget
                .apply_layout_update(WorkingAreaLayoutUpdate {
                    base_version,
                    working_area_id: WorkingAreaId(1),
                    pane_tree: PaneSplitTree::single_leaf(PaneId(1)),
                    editor_pane_id: PaneId(1),
                    pane_slots: Vec::new(),
                })
                .unwrap();
            shell.widget.reconcile_pane_hosts(&mut shell.ctx);
        });
        let _ = render_root.redraw(); // removal must not panic the register pass

        let hosts_after = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_ids()
        });

        assert_eq!(hosts_after, vec![(PaneId(1), host_keep)]);
        assert!(!render_root.has_widget(host_drop));
    }

    #[test]
    fn shell_pointer_down_focuses_placeholder_pane() {
        use masonry::core::{
            PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
        };
        use masonry::dpi::PhysicalPosition;

        let layout = WorkingAreaLayout::with_pane_tree(two_pane_tree(), PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        let _ = render_root.redraw();

        // Click inside pane 2 (right half of the 900x600 window).
        render_root.handle_pointer_event(PointerEvent::Down(PointerButtonEvent {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            button: Some(PointerButton::Primary),
            state: PointerState {
                position: PhysicalPosition::new(675.0, 300.0),
                ..Default::default()
            },
        }));

        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert_eq!(
                shell.widget.working_area_layout().active_pane_id(),
                PaneId(2)
            );
        });
    }

    /// A primary-button down/up/cancel event at `(x, y)`.
    fn pointer_button_event(x: f64, y: f64, button: Option<PointerButton>) -> PointerEvent {
        use masonry::core::{PointerId, PointerInfo, PointerState, PointerType};
        use masonry::dpi::PhysicalPosition;
        PointerEvent::Down(PointerButtonEvent {
            button,
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState {
                position: PhysicalPosition::new(x, y),
                ..Default::default()
            },
        })
    }

    fn pointer_move_event(x: f64, y: f64) -> PointerEvent {
        use masonry::core::{PointerId, PointerInfo, PointerState, PointerType, PointerUpdate};
        use masonry::dpi::PhysicalPosition;

        PointerEvent::Move(PointerUpdate {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            current: PointerState {
                position: PhysicalPosition::new(x, y),
                ..Default::default()
            },
            coalesced: Vec::new(),
            predicted: Vec::new(),
        })
    }

    fn two_pane_shell_root() -> (RenderRoot, WidgetId) {
        let layout = WorkingAreaLayout::with_pane_tree(two_pane_tree(), PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        let _ = render_root.redraw();
        (render_root, shell_id)
    }

    fn assert_active_pane(render_root: &mut RenderRoot, shell_id: WidgetId, expected: PaneId) {
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert_eq!(
                shell.widget.working_area_layout().active_pane_id(),
                expected
            );
        });
    }

    #[test]
    fn shell_default_pane_focus_policy_is_click_to_focus_and_move_is_noop() {
        let (mut render_root, shell_id) = two_pane_shell_root();

        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert_eq!(
                shell.widget.pane_focus_policy(),
                PaneFocusPolicy::ClickToFocus
            );
        });

        // Motion over pane 2 must not activate it under click-to-focus.
        render_root.handle_pointer_event(pointer_move_event(675.0, 300.0));
        assert_active_pane(&mut render_root, shell_id, PaneId(1));
    }

    #[test]
    fn shell_follows_cursor_activates_pane_under_pointer() {
        let (mut render_root, shell_id) = two_pane_shell_root();

        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell
                .widget
                .set_pane_focus_policy(PaneFocusPolicy::FollowsCursor);
        });

        render_root.handle_pointer_event(pointer_move_event(675.0, 300.0));
        assert_active_pane(&mut render_root, shell_id, PaneId(2));

        render_root.handle_pointer_event(pointer_move_event(225.0, 300.0));
        assert_active_pane(&mut render_root, shell_id, PaneId(1));
    }

    #[test]
    fn shell_follows_cursor_skips_focus_changes_during_divider_drag() {
        use masonry::core::{
            PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
        };
        use masonry::dpi::PhysicalPosition;

        let (mut render_root, shell_id) = two_pane_shell_root();

        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell
                .widget
                .set_pane_focus_policy(PaneFocusPolicy::FollowsCursor);
        });

        // Grab the divider at the pane boundary.
        render_root.handle_pointer_event(PointerEvent::Down(PointerButtonEvent {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            button: Some(PointerButton::Primary),
            state: PointerState {
                position: PhysicalPosition::new(450.0, 300.0),
                ..Default::default()
            },
        }));

        // Drag toward pane 2: ratio updates but focus must not churn.
        render_root.handle_pointer_event(pointer_move_event(675.0, 300.0));
        assert_active_pane(&mut render_root, shell_id, PaneId(1));
    }

    // -- Phase 22.1: shell command dispatch --

    fn shell_command_root() -> (RenderRoot, WidgetId) {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        let _ = render_root.redraw();
        (render_root, shell_id)
    }

    fn dispatch_shell_command(
        render_root: &mut RenderRoot,
        shell_id: WidgetId,
        command: ShellClientCommand,
    ) {
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell
                .widget
                .apply_shell_client_command(&mut shell.ctx, command);
        });
        let _ = render_root.redraw();
    }

    fn pane_count(render_root: &mut RenderRoot, shell_id: WidgetId) -> usize {
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.working_area_layout().pane_tree().pane_count()
        })
    }

    #[test]
    fn shell_command_split_vertical_creates_side_by_side_panes() {
        let (mut rr, sid) = shell_command_root();
        assert_eq!(pane_count(&mut rr, sid), 1);

        dispatch_shell_command(&mut rr, sid, ShellClientCommand::SplitPaneVertical);
        assert_eq!(pane_count(&mut rr, sid), 2);

        // Side by side: panes share y-range, differ in x.
        let rects = rr.edit_widget(sid, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_rects(Size::new(1000.0, 600.0))
        });
        assert_eq!(rects.len(), 2);
        assert!(
            rects[0].1.x0 < rects[1].1.x0,
            "panes should be side by side"
        );
        assert_eq!(rects[0].1.y0, rects[1].1.y0);
    }

    #[test]
    fn shell_command_split_horizontal_creates_stacked_panes() {
        let (mut rr, sid) = shell_command_root();
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::SplitPaneHorizontal);
        assert_eq!(pane_count(&mut rr, sid), 2);

        let rects = rr.edit_widget(sid, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_rects(Size::new(1000.0, 600.0))
        });
        // Stacked: panes share x-range, differ in y.
        assert_eq!(rects[0].1.x0, rects[1].1.x0);
        assert!(rects[0].1.y0 < rects[1].1.y0, "panes should be stacked");
    }

    #[test]
    fn shell_command_add_equal_pane_grows_to_three_equal_panes() {
        let (mut rr, sid) = shell_command_root();
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::SplitPaneVertical);
        assert_eq!(pane_count(&mut rr, sid), 2);

        dispatch_shell_command(&mut rr, sid, ShellClientCommand::AddEqualPane);
        assert_eq!(pane_count(&mut rr, sid), 3);

        let rects = rr.edit_widget(sid, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_rects(Size::new(900.0, 600.0))
        });
        // Equal areas: each pane ≈ 300px wide.
        for (_, rect) in &rects {
            let width = rect.width();
            assert!((width - 300.0).abs() < 1.0, "equal pane width {width}");
        }
    }

    /// Phase 22.7 (plan 078 task 7, A1/F1/F2): closing pane 1 — the pane that
    /// mounts the tab's connection owner (`EditorWidget`) — with a sibling
    /// present keeps the owner wired: the editor host becomes a permanent
    /// zero-size orphan, `editor_widget_id_for` still resolves, routing
    /// fallback still reaches it, and a connection event (theme) applies
    /// through it without error.
    #[test]
    fn close_editor_pane_keeps_connection_owner_wired() {
        let (mut rr, sid) = two_pane_shell_root(); // pane 1 = editor, pane 2 = placeholder
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::ClosePane);
        assert_eq!(pane_count(&mut rr, sid), 1, "pane 1 closes; pane 2 remains");

        let owner = rr.edit_widget(sid, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            let owner = shell
                .widget
                .editor_widget_id_for(0)
                .expect("owner id resolves after pane 1 closes");
            // No pane target exists for the placeholder pane, so routing
            // falls back to the owner — which must still be in the tree.
            assert_eq!(shell.widget.focus_fallback_widget_id(), owner);
            owner
        });

        // The driver's event-application path (theme/SDUI/runtime) edits the
        // owner by id; a detached owner panics here.
        rr.edit_widget(owner, |mut widget| {
            let editor = widget
                .try_downcast::<EditorWidget>()
                .expect("owner is the editor");
            assert!(
                editor
                    .widget
                    .apply_connection_event(ClientConnectionEvent::ActiveTheme(
                        crate::protocol::ActiveTheme {
                            specifier: "@clay/default".to_string(),
                            overrides: Vec::new(),
                            design_tokens: Vec::new(),
                        },
                    )),
                "theme event applies through the orphaned owner"
            );
        });
        let _ = rr.redraw();
    }

    /// Phase 22.7 (plan 078 task 7): split/close churn never drops the owner
    /// registration — the exact cycle that previously detached the editor
    /// host (a second tree mutation drains `pending_orphans`, so the owner
    /// must live in the permanent `chrome_orphans` list, not the drainable
    /// one).
    #[test]
    fn owner_survives_repeated_split_close_cycles() {
        let (mut rr, sid) = two_pane_shell_root();
        let owner = rr.edit_widget(sid, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.editor_widget_id_for(0).expect("owner mounted")
        });
        // Two churn cycles: close pane 1, split, close the new pane, split.
        for _ in 0..2 {
            dispatch_shell_command(&mut rr, sid, ShellClientCommand::ClosePane);
            dispatch_shell_command(&mut rr, sid, ShellClientCommand::SplitPaneVertical);
        }
        // Owner still editable after every cycle.
        rr.edit_widget(owner, |mut widget| {
            let editor = widget
                .try_downcast::<EditorWidget>()
                .expect("owner is the editor");
            assert!(
                editor
                    .widget
                    .apply_connection_event(ClientConnectionEvent::ActiveTheme(
                        crate::protocol::ActiveTheme {
                            specifier: "@clay/default".to_string(),
                            overrides: Vec::new(),
                            design_tokens: Vec::new(),
                        },
                    )),
                "owner receives events after split/close churn"
            );
        });
        // And the shell still reports it as the routing fallback.
        rr.edit_widget(sid, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert_eq!(shell.widget.focus_fallback_widget_id(), owner);
        });
        let _ = rr.redraw();
    }

    #[test]
    fn shell_command_close_pane_merges_back_to_single() {
        let (mut rr, sid) = shell_command_root();
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::SplitPaneVertical);
        assert_eq!(pane_count(&mut rr, sid), 2);

        dispatch_shell_command(&mut rr, sid, ShellClientCommand::ClosePane);
        assert_eq!(pane_count(&mut rr, sid), 1);
    }

    #[test]
    fn shell_command_close_single_pane_is_noop() {
        let (mut rr, sid) = shell_command_root();
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::ClosePane);
        assert_eq!(pane_count(&mut rr, sid), 1);
    }

    #[test]
    fn shell_command_focus_next_prev_cycles_panes() {
        let (mut rr, sid) = shell_command_root();
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::SplitPaneVertical);
        assert_active_pane(&mut rr, sid, PaneId(1));

        dispatch_shell_command(&mut rr, sid, ShellClientCommand::FocusPaneNext);
        assert_active_pane(&mut rr, sid, PaneId(2));

        dispatch_shell_command(&mut rr, sid, ShellClientCommand::FocusPaneNext);
        assert_active_pane(&mut rr, sid, PaneId(1)); // wraps

        dispatch_shell_command(&mut rr, sid, ShellClientCommand::FocusPanePrev);
        assert_active_pane(&mut rr, sid, PaneId(2)); // wraps back
    }

    #[test]
    fn shell_command_move_pane_swaps_reading_order() {
        let (mut rr, sid) = shell_command_root();
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::SplitPaneVertical);

        // Pane 1 is left, pane 2 is right. Move pane 1 next → IDs swap.
        let before = rr.edit_widget(sid, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_ids()
        });
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::MovePaneNext);
        let after = rr.edit_widget(sid, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_ids()
        });
        // Same host IDs (stable identity), but reading order swapped.
        assert_eq!(before, after);
        assert_eq!(pane_count(&mut rr, sid), 2);
    }

    #[test]
    fn shell_command_move_at_end_is_noop() {
        let (mut rr, sid) = shell_command_root();
        // Single pane: move is a no-op.
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::MovePaneNext);
        assert_eq!(pane_count(&mut rr, sid), 1);
    }

    #[test]
    fn shell_command_resize_changes_divider_ratio() {
        let (mut rr, sid) = shell_command_root();
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::SplitPaneVertical);

        let before = rr.edit_widget(sid, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_rects(Size::new(1000.0, 600.0))
        });
        // Pane 1 is left (x: 0..500). Resize right grows pane 1.
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::ResizePaneRight);
        let after = rr.edit_widget(sid, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_rects(Size::new(1000.0, 600.0))
        });
        // Pane 1's width increased.
        assert!(
            after[0].1.width() > before[0].1.width(),
            "resize right should grow pane 1"
        );
    }

    #[test]
    fn shell_command_cap_enforcement_at_four_panes() {
        let (mut rr, sid) = shell_command_root();
        // Grow to 4 panes.
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::SplitPaneVertical);
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::AddEqualPane);
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::AddEqualPane);
        assert_eq!(pane_count(&mut rr, sid), 4);

        // Split and add-equal are no-ops at cap.
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::SplitPaneVertical);
        assert_eq!(pane_count(&mut rr, sid), 4);
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::AddEqualPane);
        assert_eq!(pane_count(&mut rr, sid), 4);
    }

    /// Hot-path guard: shell command dispatch is a pure client-side operation.
    /// The `RenderRoot` created by `shell_command_root` has no server
    /// connection, no JS runtime, and no IPC channel. Every shell command
    /// succeeds using only the bounded `PaneSplitTree` rebuild +
    /// `reconcile_pane_hosts` — no server round-trip, no package JavaScript,
    /// no raw op. If a future change routes split dispatch through the server,
    /// this test breaks because the dispatch would need a server handle.
    #[test]
    fn shell_command_dispatch_requires_no_server_or_js_runtime() {
        let (mut rr, sid) = shell_command_root();
        // Dispatch the full lifecycle on a server-less root: 1 → 4 panes,
        // focus traversal, resize, move, then close back to 1.
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::SplitPaneVertical);
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::AddEqualPane);
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::AddEqualPane);
        assert_eq!(pane_count(&mut rr, sid), 4);

        dispatch_shell_command(&mut rr, sid, ShellClientCommand::FocusPaneNext);
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::FocusPanePrev);
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::ResizePaneRight);
        dispatch_shell_command(&mut rr, sid, ShellClientCommand::MovePaneNext);

        // Close panes back to single.
        for _ in 0..3 {
            dispatch_shell_command(&mut rr, sid, ShellClientCommand::ClosePane);
        }
        assert_eq!(pane_count(&mut rr, sid), 1);
    }

    // -- Phase 22.2: per-pane document views --

    fn document_opened(
        document_id: u64,
        version: u64,
        path: &str,
        text: &str,
    ) -> ClientConnectionEvent {
        ClientConnectionEvent::DocumentOpened {
            metadata: DocumentMetadata {
                document_id,
                version,
                access: DocumentAccess::Editable {
                    lease_id: document_id,
                },
                lease_id: Some(document_id),
                dirty: false,
                workspace_root_id: 77,
                path: path.to_string(),
            },
            text: text.to_string(),
        }
    }

    fn edit_ack(document_id: u64, version: u64) -> ClientConnectionEvent {
        ClientConnectionEvent::EditAck {
            document_id,
            version,
            transaction_id: 1,
        }
    }

    /// Mount a document view into a pane's host and register its routing
    /// target, exactly as the app driver does on a new-document open.
    fn mount_document_view(
        render_root: &mut RenderRoot,
        shell_id: WidgetId,
        pane: PaneId,
        view: PaneDocumentView,
    ) -> WidgetId {
        let host_id = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_id(pane).expect("pane host")
        });
        let view_new = NewWidget::new(view);
        let view_id = view_new.id();
        render_root.edit_widget(host_id, |mut widget| {
            let mut host = widget.try_downcast::<PaneContentHost>().expect("host");
            host.widget.set_document_view(&mut host.ctx, view_new);
        });
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_pane_target(pane, view_id);
        });
        let _ = render_root.redraw();
        view_id
    }

    fn visible_text(render_root: &mut RenderRoot, target: WidgetId) -> String {
        render_root.edit_widget(target, |mut widget| {
            if let Some(editor) = widget.try_downcast::<EditorWidget>() {
                editor.widget.visible_text_for_test()
            } else if let Some(view) = widget.try_downcast::<PaneDocumentView>() {
                view.widget.visible_text_for_test()
            } else {
                String::new()
            }
        })
    }

    fn status_text(render_root: &mut RenderRoot, target: WidgetId) -> String {
        render_root.edit_widget(target, |mut widget| {
            if let Some(editor) = widget.try_downcast::<EditorWidget>() {
                editor.widget.status_text()
            } else if let Some(view) = widget.try_downcast::<PaneDocumentView>() {
                view.widget.status_text()
            } else {
                String::new()
            }
        })
    }

    #[test]
    fn panes_host_independent_document_views_with_document_scoped_routing() {
        let (queue, _receiver) = ClientEditQueue::bounded(16);
        let layout = WorkingAreaLayout::with_pane_tree(two_pane_tree(), PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(
            EditorWidget::with_initial_state(initial_state(
                DocumentAccess::Editable { lease_id: 1 },
                3,
            ))
            .with_edit_queue(queue.clone()),
            layout,
        );
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        let chrome_id = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.editor_widget_id()
        });
        let _ = render_root.redraw();

        // Pane 2 gets a live document view (driver mount flow).
        let view2 = PaneDocumentView::new(
            PaneId(2),
            std::rc::Rc::new(std::cell::Cell::new(1)),
            std::rc::Rc::new(std::cell::Cell::new(0)),
        )
        .with_edit_queue(queue.clone());
        let view2_id = mount_document_view(&mut render_root, shell_id, PaneId(2), view2);

        // Document A opens into pane 1 (chrome), document B into pane 2.
        // (The chrome's initial state already owns doc 7; 70 is a fresh open.)
        render_root.edit_widget(chrome_id, |mut widget| {
            let editor = widget.try_downcast::<EditorWidget>().expect("chrome");
            assert!(
                editor
                    .widget
                    .apply_connection_event(document_opened(70, 1, "a.md", "alpha"))
            );
        });
        render_root.edit_widget(view2_id, |mut widget| {
            let view = widget.try_downcast::<PaneDocumentView>().expect("view");
            assert!(
                view.widget
                    .apply_connection_event(document_opened(42, 1, "b.md", "beta"))
            );
        });
        assert_eq!(visible_text(&mut render_root, chrome_id), "alpha");
        assert_eq!(visible_text(&mut render_root, view2_id), "beta");

        // EditAck for document A changes only pane 1; document B only pane 2.
        render_root.edit_widget(chrome_id, |mut widget| {
            let editor = widget.try_downcast::<EditorWidget>().expect("chrome");
            assert!(editor.widget.apply_connection_event(edit_ack(70, 2)));
        });
        render_root.edit_widget(view2_id, |mut widget| {
            let view = widget.try_downcast::<PaneDocumentView>().expect("view");
            assert!(view.widget.apply_connection_event(edit_ack(42, 2)));
        });
        assert!(status_text(&mut render_root, chrome_id).contains("v2"));
        assert!(status_text(&mut render_root, view2_id).contains("v2"));

        // A foreign ack (for pane 1's document) never touches pane 2.
        render_root.edit_widget(view2_id, |mut widget| {
            let view = widget.try_downcast::<PaneDocumentView>().expect("view");
            assert!(!view.widget.apply_connection_event(edit_ack(70, 9)));
        });
        assert!(status_text(&mut render_root, view2_id).contains("v2"));
        assert!(!status_text(&mut render_root, view2_id).contains("v9"));
    }

    #[test]
    fn concurrent_pane_major_modes_stay_isolated_across_behavior_manifests() {
        use crate::protocol::DocumentFontRole;

        let (queue, _receiver) = ClientEditQueue::bounded(16);
        let layout = WorkingAreaLayout::with_pane_tree(two_pane_tree(), PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(
            EditorWidget::with_initial_state(initial_state(
                DocumentAccess::Editable { lease_id: 1 },
                3,
            ))
            .with_edit_queue(queue.clone()),
            layout,
        );
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        let chrome_id = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.editor_widget_id()
        });
        let _ = render_root.redraw();
        let view2 = PaneDocumentView::new(
            PaneId(2),
            std::rc::Rc::new(std::cell::Cell::new(1)),
            std::rc::Rc::new(std::cell::Cell::new(0)),
        )
        .with_edit_queue(queue.clone());
        let view2_id = mount_document_view(&mut render_root, shell_id, PaneId(2), view2);

        // Pane 1 owns document 70 (.md), pane 2 owns document 42 (.rs).
        render_root.edit_widget(chrome_id, |mut widget| {
            let editor = widget.try_downcast::<EditorWidget>().expect("chrome");
            assert!(
                editor
                    .widget
                    .apply_connection_event(document_opened(70, 1, "a.md", "alpha"))
            );
        });
        render_root.edit_widget(view2_id, |mut widget| {
            let view = widget.try_downcast::<PaneDocumentView>().expect("view");
            assert!(
                view.widget
                    .apply_connection_event(document_opened(42, 1, "b.rs", "beta"))
            );
        });

        fn mode_manifest(
            version: crate::protocol::BehaviorVersion,
            document_id: u64,
            mode_id: &str,
            font_role: DocumentFontRole,
        ) -> crate::protocol::BehaviorManifest {
            let mut manifest = crate::protocol::BehaviorManifest::minimal_text_editing(version);
            manifest.manifest_id = format!("{mode_id}.{mode_id}");
            manifest.scope = crate::protocol::BehaviorScope::Document { document_id };
            manifest.document_font_role = font_role;
            manifest
        }

        // Markdown activation for doc 70: pane 1 installs it, pane 2 only
        // tracks the version bump (versions 4/5 are above the baseline 3).
        let markdown = mode_manifest(4, 70, "markdown", DocumentFontRole::Proportional);
        render_root.edit_widget(chrome_id, |mut widget| {
            let editor = widget.try_downcast::<EditorWidget>().expect("chrome");
            editor.widget.apply_behavior_manifest(&markdown);
        });
        render_root.edit_widget(view2_id, |mut widget| {
            let view = widget.try_downcast::<PaneDocumentView>().expect("view");
            view.widget.apply_behavior_manifest(&markdown);
        });

        // Rust activation for doc 42: pane 2 installs it, pane 1 only tracks
        // the version bump.
        let rust = mode_manifest(5, 42, "rust", DocumentFontRole::Monospace);
        render_root.edit_widget(chrome_id, |mut widget| {
            let editor = widget.try_downcast::<EditorWidget>().expect("chrome");
            editor.widget.apply_behavior_manifest(&rust);
        });
        render_root.edit_widget(view2_id, |mut widget| {
            let view = widget.try_downcast::<PaneDocumentView>().expect("view");
            view.widget.apply_behavior_manifest(&rust);
        });

        // Pane 1: markdown content, connection version 3.
        render_root.edit_widget(chrome_id, |mut widget| {
            let editor = widget.try_downcast::<EditorWidget>().expect("chrome");
            let state = editor.widget.editor_state_for_test();
            assert_eq!(
                state.behavior_manifest.as_ref().unwrap().manifest_id,
                "markdown.markdown"
            );
            assert_eq!(
                state.behavior_manifest.as_ref().unwrap().document_font_role,
                DocumentFontRole::Proportional
            );
            assert_eq!(state.behavior_version, 5);
        });
        // Pane 2: rust content, connection version 5.
        render_root.edit_widget(view2_id, |mut widget| {
            let view = widget.try_downcast::<PaneDocumentView>().expect("view");
            let state = view.widget.editor_state_for_test();
            assert_eq!(
                state.behavior_manifest.as_ref().unwrap().manifest_id,
                "rust.rust"
            );
            assert_eq!(
                state.behavior_manifest.as_ref().unwrap().document_font_role,
                DocumentFontRole::Monospace
            );
            assert_eq!(state.behavior_version, 5);
        });
    }

    /// Hot-path guard: typing in one pane performs no work against other
    /// panes' surfaces and no IPC. The `RenderRoot` has no server connection,
    /// no JS runtime, and no IPC channel.
    #[test]
    fn keystroke_in_one_pane_touches_only_that_pane_without_ipc() {
        let (queue, _receiver) = ClientEditQueue::bounded(16);
        let tree = PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::balanced(),
                PaneSplitNode::split(
                    SplitOrientation::Horizontal,
                    SplitRatio::balanced(),
                    PaneSplitNode::leaf(PaneId(1)),
                    PaneSplitNode::leaf(PaneId(2)),
                ),
                PaneSplitNode::split(
                    SplitOrientation::Horizontal,
                    SplitRatio::balanced(),
                    PaneSplitNode::leaf(PaneId(3)),
                    PaneSplitNode::leaf(PaneId(4)),
                ),
            ),
            PaneId(3),
        )
        .unwrap();
        let layout = WorkingAreaLayout::with_pane_tree(tree, PaneId(3)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(
            EditorWidget::with_initial_state(initial_state(
                DocumentAccess::Editable { lease_id: 1 },
                3,
            ))
            .with_edit_queue(queue.clone()),
            layout,
        );
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        let _ = render_root.redraw();

        // Mount live views in panes 2, 3 and 4; pane 1 is the chrome.
        let mut view_ids = Vec::new();
        for pane in [PaneId(2), PaneId(3), PaneId(4)] {
            let view = PaneDocumentView::new(
                pane,
                std::rc::Rc::new(std::cell::Cell::new(1)),
                std::rc::Rc::new(std::cell::Cell::new(0)),
            )
            .with_edit_queue(queue.clone());
            let view_id = mount_document_view(&mut render_root, shell_id, pane, view);
            render_root.edit_widget(view_id, |mut widget| {
                let view = widget.try_downcast::<PaneDocumentView>().expect("view");
                let _ = view
                    .widget
                    .apply_connection_event(document_opened(pane.0, 1, "x.md", "base"));
            });
            view_ids.push(view_id);
        }

        // Type into pane 3's view.
        render_root.focus_on(Some(view_ids[1]));
        render_root.handle_text_event(TextEvent::Ime(Ime::Commit("!".to_string())));

        assert_eq!(visible_text(&mut render_root, view_ids[0]), "base");
        assert_eq!(visible_text(&mut render_root, view_ids[1]), "!base");
        assert_eq!(visible_text(&mut render_root, view_ids[2]), "base");
    }

    #[test]
    fn pane_close_removes_routing_target_and_reconciles_host() {
        let (queue, _receiver) = ClientEditQueue::bounded(16);
        // Chrome hosts pane 1; pane 2 is a placeholder we mount a view into.
        let layout = WorkingAreaLayout::with_pane_tree(two_pane_tree(), PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(
            EditorWidget::with_initial_state(initial_state(
                DocumentAccess::Editable { lease_id: 1 },
                3,
            ))
            .with_edit_queue(queue.clone()),
            layout,
        );
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        let _ = render_root.redraw();

        // Pane 2 must be the active pane so ClosePane targets it.
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_active_pane(PaneId(2));
        });

        let view2 = PaneDocumentView::new(
            PaneId(2),
            std::rc::Rc::new(std::cell::Cell::new(1)),
            std::rc::Rc::new(std::cell::Cell::new(0)),
        )
        .with_edit_queue(queue.clone());
        let view2_id = mount_document_view(&mut render_root, shell_id, PaneId(2), view2);

        assert!(render_root.has_widget(view2_id), "view mounted");
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert_eq!(shell.widget.pane_target(PaneId(2)), Some(view2_id));
            assert_eq!(shell.widget.active_pane_target(), Some(view2_id));
        });

        // Close pane 2 (the active pane); the target is dropped with the host.
        dispatch_shell_command(&mut render_root, shell_id, ShellClientCommand::ClosePane);
        let _ = render_root.redraw();

        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert_eq!(shell.widget.pane_target(PaneId(2)), None);
            assert_eq!(shell.widget.pane_targets().len(), 1);
        });
        // Note: Masonry's arena retains removed nodes; `has_widget` is not a
        // reliable post-removal probe for detached subtrees. The routing
        // contract (targets dropped with the pane) is asserted above.
    }

    // -- Phase 22.3: multi-tab hosting --

    fn second_tab_chrome() -> TabChrome {
        TabChrome::single_editor(EditorWidget::default(), false)
    }

    #[test]
    fn install_tab_switches_to_new_tab_and_retains_previous() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());
        let first_chrome_id = shell.editor_widget_id();
        let first_host_ids: Vec<WidgetId> = shell.children_ids().iter().copied().collect();
        assert_eq!(first_host_ids.len(), 1);

        let second = second_tab_chrome();
        let second_chrome_id = second.editor_widget_id();
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.install_tab(&mut shell.ctx, 2, second);
            // install does not switch; the open-tab path activates explicitly.
            assert_eq!(shell.widget.editor_widget_id(), first_chrome_id);
            assert!(shell.widget.set_active_tab(&mut shell.ctx, 2));
        });

        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            // The new tab is mounted (active); the previous tab is retained
            // (stashed, not announced).
            assert_eq!(shell.widget.editor_widget_id(), second_chrome_id);
            assert_eq!(shell.widget.tab_for_chrome(first_chrome_id), Some(0));
            assert_eq!(shell.widget.tab_for_chrome(second_chrome_id), Some(2));
            // Both tabs' hosts are registered children (stable ids).
            let ids: Vec<WidgetId> = shell.widget.children_ids().iter().copied().collect();
            assert_eq!(ids.len(), 2);
            assert!(ids.contains(&first_host_ids[0]));
        });
    }

    #[test]
    fn set_active_tab_keeps_widget_ids_stable() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());
        let first_chrome_id = shell.editor_widget_id();
        let second = second_tab_chrome();
        let second_chrome_id = second.editor_widget_id();
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.install_tab(&mut shell.ctx, 2, second);
            assert!(shell.widget.set_active_tab(&mut shell.ctx, 2));
        });

        // Switch back to tab 0: the first tab's chrome id is unchanged.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert!(shell.widget.set_active_tab(&mut shell.ctx, 0));
            assert_eq!(shell.widget.editor_widget_id(), first_chrome_id);
            assert!(
                !shell.widget.set_active_tab(&mut shell.ctx, 0),
                "no-op on same tab"
            );
            assert!(
                !shell.widget.set_active_tab(&mut shell.ctx, 99),
                "unknown tab rejected"
            );
        });
        // And back again: the second tab's chrome id is unchanged.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert!(shell.widget.set_active_tab(&mut shell.ctx, 2));
            assert_eq!(shell.widget.editor_widget_id(), second_chrome_id);
        });
    }

    #[test]
    fn inactive_tab_hosts_laid_out_at_zero_size() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());
        let second = second_tab_chrome();
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.install_tab(&mut shell.ctx, 2, second);
        });
        let _ = render_root.redraw();

        // The active tab's host occupies the working area; the inactive tab's
        // host is retained at zero size (no paint, no hit-test).
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            let active_rects = shell.widget.pane_host_rects(Size::new(900.0, 600.0));
            assert_eq!(active_rects.len(), 1);
            assert_eq!(active_rects[0].1, Rect::new(0.0, 0.0, 900.0, 600.0));
        });
        // Switch: the other tab's host now gets the working area.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_active_tab(&mut shell.ctx, 2);
        });
        let _ = render_root.redraw();
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            let active_rects = shell.widget.pane_host_rects(Size::new(900.0, 600.0));
            assert_eq!(active_rects.len(), 1);
            assert_eq!(active_rects[0].1, Rect::new(0.0, 0.0, 900.0, 600.0));
        });
    }

    #[test]
    fn single_tab_behavior_is_pre_22_3() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());
        let chrome_id = shell.editor_widget_id();
        // One tab, one host, pane 1 routed to the chrome — the pre-22.3 shape.
        assert_eq!(shell.tabs.len(), 1);
        assert_eq!(shell.active_tab, 0);
        assert_eq!(shell.active_pane_id(), PaneId(1));
        assert_eq!(shell.pane_targets(), vec![(PaneId(1), chrome_id)]);
        assert_eq!(shell.active_pane_target(), Some(chrome_id));
        assert_eq!(shell.focus_fallback_widget_id(), chrome_id);
        let ids: Vec<WidgetId> = shell.children_ids().iter().copied().collect();
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn per_tab_routing_targets_are_isolated() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());
        let second = second_tab_chrome();
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.install_tab(&mut shell.ctx, 2, second);
        });

        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            // Each tab's routing queries answer only that tab's targets.
            assert_eq!(shell.widget.pane_targets_for(0).len(), 1);
            assert_eq!(shell.widget.pane_targets_for(2).len(), 1);
            assert_ne!(
                shell.widget.pane_targets_for(0)[0].1,
                shell.widget.pane_targets_for(2)[0].1,
                "tabs never share pane targets"
            );
            assert_eq!(shell.widget.pane_targets_for(99).len(), 0);
            // Focus policy is per-tab.
            shell
                .widget
                .set_pane_focus_policy_for(0, PaneFocusPolicy::ClickToFocus);
            assert_eq!(
                shell.widget.pane_focus_policy_for(0),
                PaneFocusPolicy::ClickToFocus
            );
            assert_eq!(
                shell.widget.pane_focus_policy_for(2),
                PaneFocusPolicy::default()
            );
        });
    }

    // -- Phase 22.5: tab × split composition guards --

    fn vertical_two_pane_tree() -> PaneSplitTree {
        PaneSplitTree::new(
            PaneSplitNode::split(
                SplitOrientation::Vertical,
                SplitRatio::new(0.7).unwrap(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            ),
            PaneId(2),
        )
        .unwrap()
    }

    /// Tab 0 active with a horizontal split; tab 2 inactive with a vertical
    /// split (ratio 0.7, active pane 2) — distinct topologies per tab.
    fn two_split_tab_shell_root() -> (RenderRoot, WidgetId) {
        let layout = WorkingAreaLayout::with_pane_tree(two_pane_tree(), PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);
        let second = TabChrome::with_layout(
            EditorWidget::default(),
            WorkingAreaLayout::with_pane_tree(vertical_two_pane_tree(), PaneId(1)).unwrap(),
        );
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            // install does not switch: tab 0 stays active.
            shell.widget.install_tab(&mut shell.ctx, 2, second);
        });
        let _ = render_root.redraw();
        (render_root, shell_id)
    }

    #[test]
    fn pane_commands_only_mutate_the_active_tab() {
        let (mut render_root, shell_id) = two_split_tab_shell_root();
        let mut inactive_before = None;
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            inactive_before = shell.widget.working_area_layout_for(2).cloned();
            assert_eq!(shell.widget.active_pane_id(), PaneId(1));
        });
        // Pane-family commands while tab 0 is active.
        for command in [
            ShellClientCommand::SplitPaneVertical,
            ShellClientCommand::FocusPaneNext,
            ShellClientCommand::ResizePaneLeft,
        ] {
            dispatch_shell_command(&mut render_root, shell_id, command);
        }
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            // The active tab took the split.
            assert_eq!(
                shell.widget.working_area_layout().pane_tree().pane_count(),
                3
            );
            // The inactive tab's layout is byte-identical: tree, ratios,
            // slots, active pane.
            assert_eq!(
                shell.widget.working_area_layout_for(2),
                inactive_before.as_ref()
            );
        });
    }

    #[test]
    fn divider_drag_credits_only_the_active_tab() {
        use masonry::core::{
            PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
        };
        use masonry::dpi::PhysicalPosition;

        let (mut render_root, shell_id) = two_split_tab_shell_root();
        // Grab the active tab's divider at the pane boundary and drag it.
        render_root.handle_pointer_event(PointerEvent::Down(PointerButtonEvent {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            button: Some(PointerButton::Primary),
            state: PointerState {
                position: PhysicalPosition::new(450.0, 300.0),
                ..Default::default()
            },
        }));
        render_root.handle_pointer_event(pointer_move_event(675.0, 300.0));
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            // The active tab's divider moved (675/900).
            assert_eq!(
                shell
                    .widget
                    .working_area_layout()
                    .pane_tree()
                    .split_ratio_at_path(&[]),
                Some(SplitRatio::new(0.75).unwrap())
            );
            // The inactive tab's ratio is untouched.
            assert_eq!(
                shell
                    .widget
                    .working_area_layout_for(2)
                    .and_then(|layout| layout.pane_tree().split_ratio_at_path(&[])),
                Some(SplitRatio::new(0.7).unwrap())
            );
        });
    }

    #[test]
    fn tab_switch_round_trip_preserves_split_trees_and_active_panes() {
        let (mut render_root, shell_id) = two_split_tab_shell_root();
        let mut layouts_before = (None, None);
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            layouts_before.0 = shell.widget.working_area_layout_for(0).cloned();
            layouts_before.1 = shell.widget.working_area_layout_for(2).cloned();
        });
        // Switch 0 -> 2: tab 2's own tree is live (active pane 2).
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert!(shell.widget.set_active_tab(&mut shell.ctx, 2));
            assert_eq!(shell.widget.active_pane_id(), PaneId(2));
        });
        // Switch back: both tabs' layouts are exactly as before.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert!(shell.widget.set_active_tab(&mut shell.ctx, 0));
            assert_eq!(shell.widget.active_pane_id(), PaneId(1));
            assert_eq!(
                shell.widget.working_area_layout_for(0),
                layouts_before.0.as_ref()
            );
            assert_eq!(
                shell.widget.working_area_layout_for(2),
                layouts_before.1.as_ref()
            );
        });
        let _ = render_root.redraw();
    }

    // -- Phase 22.5: persistence signals --

    /// A two-tab shell (tab 0 active with a split, tab 2 inactive) capturing
    /// submitted actions.
    fn persistence_signal_root() -> (RenderRoot, WidgetId, Rc<RefCell<Vec<EditorAction>>>) {
        let layout = WorkingAreaLayout::with_pane_tree(two_pane_tree(), PaneId(1)).unwrap();
        let shell = ClayShellWidget::single_editor_with_layout(EditorWidget::default(), layout);
        let second = TabChrome::with_layout(
            EditorWidget::default(),
            WorkingAreaLayout::with_pane_tree(vertical_two_pane_tree(), PaneId(1)).unwrap(),
        );
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let captured: Rc<RefCell<Vec<EditorAction>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = captured.clone();
        let mut render_root = RenderRoot::new(
            shell_new,
            move |signal| {
                if let RenderRootSignal::Action(action, _id) = signal
                    && let Ok(editor_action) = action.downcast::<EditorAction>()
                {
                    sink.borrow_mut().push(*editor_action);
                }
            },
            render_root_options(),
        );
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.install_tab(&mut shell.ctx, 2, second);
        });
        let _ = render_root.redraw();
        (render_root, shell_id, captured)
    }

    #[test]
    fn layout_mutation_signals_persistence_with_multiple_tabs() {
        // The 22.3 single-tab guard is gone: a topology mutation with two
        // tabs mounted reaches the driver as a PersistenceDue action.
        let (mut render_root, shell_id, captured) = persistence_signal_root();
        dispatch_shell_command(
            &mut render_root,
            shell_id,
            ShellClientCommand::SplitPaneVertical,
        );
        assert!(
            captured
                .borrow()
                .iter()
                .any(|action| matches!(action, EditorAction::PersistenceDue)),
            "split with 2 tabs must signal persistence"
        );
    }

    #[test]
    fn keyboard_resize_signals_persistence() {
        let (mut render_root, shell_id, captured) = persistence_signal_root();
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell
                .widget
                .apply_keyboard_resize(&mut shell.ctx, PaneResizeDirection::Right);
        });
        assert!(
            captured
                .borrow()
                .iter()
                .any(|action| matches!(action, EditorAction::PersistenceDue))
        );
    }

    #[test]
    fn tab_layout_data_returns_every_mounted_tab_layout() {
        let (mut render_root, shell_id, _captured) = persistence_signal_root();
        let layouts = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.tab_layout_data()
        });
        assert_eq!(layouts.len(), 2);
        let (id0, layout0) = &layouts[0];
        let (id2, layout2) = &layouts[1];
        assert_eq!(*id0, 0);
        assert_eq!(layout0.active_pane, PaneId(1));
        assert!(matches!(&layout0.tree, PaneSplitNode::Split { .. }));
        assert_eq!(*id2, 2);
        assert_eq!(layout2.active_pane, PaneId(2));
        match &layout2.tree {
            PaneSplitNode::Split {
                orientation, ratio, ..
            } => {
                assert_eq!(*orientation, SplitOrientation::Vertical);
                assert_eq!(*ratio, SplitRatio::new(0.7).unwrap());
            }
            _ => panic!("tab 2 must be a split"),
        }
    }

    // -- Phase 22.5: whole-window restore --

    /// A persisted tab with a 0.7 horizontal two-pane tree, active pane 2.
    fn persisted_two_pane_tab() -> PersistedTabState {
        PersistedTabState {
            workspace_root: "/tmp".to_string(),
            active_pane: PaneId(2),
            tree: Some(PaneSplitNode::split(
                SplitOrientation::Horizontal,
                SplitRatio::new(0.7).unwrap(),
                PaneSplitNode::leaf(PaneId(1)),
                PaneSplitNode::leaf(PaneId(2)),
            )),
            slots: Vec::new(),
            panes: BTreeMap::new(),
        }
    }

    #[test]
    fn restored_single_editor_mounts_persisted_split_tree() {
        let shell = ClayShellWidget::restored_single_editor(
            7,
            EditorWidget::default(),
            &persisted_two_pane_tab(),
        );
        let layout = shell.working_area_layout_for(7).expect("tab 7 layout");
        assert_eq!(layout.pane_tree().pane_count(), 2);
        assert_eq!(layout.active_pane_id(), PaneId(2));
        assert_eq!(
            layout.pane_tree().split_ratio_at_path(&[]),
            Some(SplitRatio::new(0.7).unwrap())
        );
    }

    #[test]
    fn install_restored_tab_mounts_persisted_tree_without_switching() {
        let (mut render_root, shell_id, _captured) = persistence_signal_root();
        let persisted = persisted_two_pane_tab();
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.install_restored_tab(
                &mut shell.ctx,
                5,
                EditorWidget::default(),
                &persisted,
            );
        });
        // Tab 5 is mounted with the persisted tree. Pane targets hold only
        // content hosts (the chrome); the placeholder pane becomes a target
        // when a document opens into it.
        let (targets, layouts) = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            (
                shell.widget.pane_targets_for(5).len(),
                shell.widget.tab_layout_data(),
            )
        });
        assert_eq!(targets, 1);
        let tab5 = layouts.iter().find(|(id, _)| *id == 5).expect("tab 5");
        assert_eq!(tab5.1.active_pane, PaneId(2));
        // The active tab is unchanged (still tab 0's balanced split — the
        // restore activates the persisted active tab at the very end).
        let active_ratio = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell
                .widget
                .working_area_layout()
                .pane_tree()
                .split_ratio_at_path(&[])
        });
        assert_eq!(active_ratio, Some(SplitRatio::new(0.5).unwrap()));
    }

    // -- Phase 22.3: tab bar chrome --

    fn tab_bar_shell_root() -> (RenderRoot, WidgetId, Rc<RefCell<Vec<EditorAction>>>) {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let captured: Rc<RefCell<Vec<EditorAction>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = captured.clone();
        let mut render_root = RenderRoot::new(
            shell_new,
            move |signal| {
                if let RenderRootSignal::Action(action, _id) = signal
                    && let Ok(editor_action) = action.downcast::<EditorAction>()
                {
                    sink.borrow_mut().push(*editor_action);
                }
            },
            render_root_options(),
        );
        let _ = render_root.redraw();
        (render_root, shell_id, captured)
    }

    /// A shell with two mounted tabs (clients 0 and 2) and two cards; the
    /// second tab is active. Card 0 = "alpha", card 1 = "beta".
    fn tab_bar_two_card_root() -> (RenderRoot, WidgetId, Rc<RefCell<Vec<EditorAction>>>) {
        let (mut render_root, shell_id, captured) = tab_bar_shell_root();
        let second = TabChrome::single_editor(EditorWidget::default(), false);
        let second_chrome_id = second.editor_widget_id();
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.install_tab(&mut shell.ctx, 2, second);
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                vec![
                    TabCard {
                        client_id: 0,
                        name: "alpha".to_string(),
                        closable: true,
                    },
                    TabCard {
                        client_id: 2,
                        name: "beta".to_string(),
                        closable: true,
                    },
                ],
            );
            shell.widget.set_active_tab(&mut shell.ctx, 2);
        });
        let _ = render_root.redraw();
        let _ = second_chrome_id;
        (render_root, shell_id, captured)
    }

    fn tab_bar_click(render_root: &mut RenderRoot, x: f64, y: f64) {
        use masonry::core::{PointerId, PointerInfo, PointerState, PointerType};
        use masonry::dpi::PhysicalPosition;
        render_root.handle_pointer_event(PointerEvent::Down(PointerButtonEvent {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            button: Some(PointerButton::Primary),
            state: PointerState {
                position: PhysicalPosition::new(x, y),
                ..Default::default()
            },
        }));
    }

    #[test]
    fn tab_bar_hidden_with_less_than_two_cards() {
        let (mut render_root, shell_id, _) = tab_bar_shell_root();
        // No cards at all: no bar.
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert!(
                shell
                    .widget
                    .tab_bar_geometry(Size::new(900.0, 600.0))
                    .is_none()
            );
        });
        // One card is still no bar (single-tab-match-today).
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                vec![TabCard {
                    client_id: 0,
                    name: "alpha".to_string(),
                    closable: true,
                }],
            );
        });
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert!(
                shell
                    .widget
                    .tab_bar_geometry(Size::new(900.0, 600.0))
                    .is_none()
            );
            // Single-tab working-area geometry is the pre-22.3 shape: the
            // host sits at the window top (no carve).
            let rects = shell.widget.pane_host_rects(Size::new(900.0, 600.0));
            assert_eq!(rects.len(), 1);
            assert_eq!(rects[0].1.y0, 0.0);
        });
    }

    #[test]
    fn tab_bar_with_two_cards_carves_the_working_area() {
        let (mut render_root, shell_id, _) = tab_bar_two_card_root();
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            let geometry = shell
                .widget
                .tab_bar_geometry(Size::new(900.0, 600.0))
                .expect("bar visible with two cards");
            assert_eq!(geometry.bar, Rect::new(0.0, 0.0, 900.0, 30.0));
            assert_eq!(geometry.cards.len(), 2);
            // Card 0: gap 4, width 180 → 4..184; card 1 starts at 188.
            assert_eq!(geometry.cards[0].rect, Rect::new(4.0, 4.0, 184.0, 26.0));
            assert_eq!(geometry.cards[1].rect, Rect::new(188.0, 4.0, 368.0, 26.0));
            // The close glyph is the rightmost affordance inside card 0.
            let close = geometry.cards[0].close_rect;
            assert!(geometry.cards[0].rect.contains(close.center()));
            // The working area starts below the bar.
            let rects = shell.widget.pane_host_rects(Size::new(900.0, 600.0));
            assert_eq!(rects.len(), 1);
            assert_eq!(rects[0].1.y0, 30.0);
        });
    }

    #[test]
    fn tab_bar_card_click_submits_activate_for_other_tab() {
        let (mut render_root, _shell_id, captured) = tab_bar_two_card_root();
        // Click card 0 ("alpha", client 0) — the active tab is 2.
        tab_bar_click(&mut render_root, 94.0, 15.0);
        let actions = captured.borrow();
        assert!(
            actions.contains(&EditorAction::TabBar(TabBarAction::Activate {
                client_id: 0
            }))
        );
    }

    fn tab_bar_wheel_event(render_root: &mut RenderRoot, x: f64, y: f64, dx: f32, dy: f32) {
        use masonry::core::ScrollDelta;
        use masonry::core::{
            PointerId, PointerInfo, PointerScrollEvent, PointerState, PointerType,
        };
        use masonry::dpi::PhysicalPosition;
        render_root.handle_pointer_event(PointerEvent::Scroll(PointerScrollEvent {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            delta: ScrollDelta::LineDelta(dx, dy),
            state: PointerState {
                position: PhysicalPosition::new(x, y),
                ..Default::default()
            },
        }));
    }

    /// Precise-scroll variant: pixel deltas must map 1:1 (no line
    /// multiplier).
    fn tab_bar_pixel_wheel_event(render_root: &mut RenderRoot, x: f64, y: f64, dy: f64) {
        use masonry::core::ScrollDelta;
        use masonry::core::{
            PointerId, PointerInfo, PointerScrollEvent, PointerState, PointerType,
        };
        use masonry::dpi::PhysicalPosition;
        render_root.handle_pointer_event(PointerEvent::Scroll(PointerScrollEvent {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            delta: ScrollDelta::PixelDelta(PhysicalPosition::new(0.0, dy)),
            state: PointerState {
                position: PhysicalPosition::new(x, y),
                ..Default::default()
            },
        }));
    }

    /// Six overflowing cards at 900px: widths 180,180,180,180,124,100 →
    /// strip = 972 vs 868 available → scroll_max = 100.
    fn six_overflowing_cards(render_root: &mut RenderRoot, shell_id: WidgetId) {
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                (0..6)
                    .map(|i| TabCard {
                        client_id: i,
                        name: format!("t{i}"),
                        closable: true,
                    })
                    .collect(),
            );
        });
    }

    /// Phase 22.7 (F3): the direction-named split aliases resolve to the
    /// canonical commands; unknown IDs stay rejected.
    #[test]
    fn split_alias_ids_resolve_to_canonical_commands() {
        use crate::masonry_shell::ShellClientCommand;
        assert_eq!(
            ShellClientCommand::from_command_id("shell.clientSplitPaneRight"),
            Some(ShellClientCommand::SplitPaneVertical),
            "right = side-by-side split (canonical Vertical)"
        );
        assert_eq!(
            ShellClientCommand::from_command_id("shell.clientSplitPaneDown"),
            Some(ShellClientCommand::SplitPaneHorizontal),
            "down = stacked split (canonical Horizontal)"
        );
        // Canonical IDs unchanged.
        assert_eq!(
            ShellClientCommand::from_command_id("shell.clientSplitPaneVertical"),
            Some(ShellClientCommand::SplitPaneVertical)
        );
        assert_eq!(
            ShellClientCommand::from_command_id("shell.clientSplitPaneHorizontal"),
            Some(ShellClientCommand::SplitPaneHorizontal)
        );
        // Unknown direction names are not aliases.
        assert_eq!(
            ShellClientCommand::from_command_id("shell.clientSplitPaneLeft"),
            None
        );
        assert_eq!(
            ShellClientCommand::from_command_id("shell.clientSplitPaneUp"),
            None
        );
    }

    #[test]
    fn tab_bar_cards_never_below_min_width() {
        let (mut render_root, shell_id, _) = tab_bar_two_card_root();
        six_overflowing_cards(&mut render_root, shell_id);
        // A 260px window: the strip (228px) cannot hold six 100px cards, so
        // the floor binds and the bar overflows instead of crushing cards.
        let geometry = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell
                .widget
                .tab_bar_geometry(Size::new(260.0, 600.0))
                .expect("bar visible")
        });
        for card in &geometry.cards {
            assert!(
                card.rect.width() >= TAB_BAR_CARD_MIN_WIDTH,
                "card width {} below floor",
                card.rect.width()
            );
        }
        assert!(geometry.scroll_max > 0.0, "overflow must be scrollable");
        // A wide window keeps the pre-22.7 shrink-to-fit shape (no scroll).
        let wide = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell
                .widget
                .tab_bar_geometry(Size::new(1200.0, 600.0))
                .expect("bar visible")
        });
        assert_eq!(wide.scroll, 0.0);
        assert_eq!(wide.scroll_max, 0.0);
        assert_eq!(wide.cards[0].rect.width(), TAB_BAR_CARD_WIDTH);
    }

    #[test]
    fn tab_bar_wheel_scroll_clamps() {
        let (mut render_root, shell_id, _) = tab_bar_two_card_root();
        six_overflowing_cards(&mut render_root, shell_id);
        let _ = render_root.redraw();

        // Wheel down (dy > 0) over the bar scrolls the strip right, clamped
        // at scroll_max = 100.
        for _ in 0..5 {
            tab_bar_wheel_event(&mut render_root, 300.0, 15.0, 0.0, 3.0);
        }
        let scroll = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.tab_bar_scroll()
        });
        assert_eq!(scroll, 100.0, "wheel clamps at scroll_max");

        // Wheel up returns to the left edge (clamps at 0).
        for _ in 0..10 {
            tab_bar_wheel_event(&mut render_root, 300.0, 15.0, 0.0, -3.0);
        }
        let scroll = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.tab_bar_scroll()
        });
        assert_eq!(scroll, 0.0, "wheel clamps at 0");

        // Wheel over the working area (below the bar) does not scroll.
        tab_bar_wheel_event(&mut render_root, 300.0, 15.0, 0.0, 3.0);
        tab_bar_wheel_event(&mut render_root, 450.0, 300.0, 0.0, 3.0);
        let scroll = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.tab_bar_scroll()
        });
        assert_eq!(scroll, 72.0, "only bar-area wheel scrolls");

        // Pixel deltas (precise scroll) map 1:1 — no 24px line multiplier
        // (72 - 72 = 0, then +50px lands exactly at 50, not clamped 100).
        for _ in 0..3 {
            tab_bar_wheel_event(&mut render_root, 300.0, 15.0, 0.0, -1.0);
        }
        tab_bar_pixel_wheel_event(&mut render_root, 300.0, 15.0, 50.0);
        let scroll = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.tab_bar_scroll()
        });
        assert_eq!(scroll, 50.0, "pixel deltas scroll 1:1 in pixels");
    }

    #[test]
    fn tab_bar_hit_test_honors_scroll() {
        let (mut render_root, _shell_id, captured) = tab_bar_two_card_root();
        six_overflowing_cards(&mut render_root, _shell_id);
        let _ = render_root.redraw();
        // Scroll to the far right: card 0 shifts to [-96, 84], card 1 to
        // [88, 268]. A click at x=140 would hit card 0 at scroll 0 but must
        // hit card 1 at scroll 100.
        for _ in 0..5 {
            tab_bar_wheel_event(&mut render_root, 300.0, 15.0, 0.0, 3.0);
        }
        tab_bar_click(&mut render_root, 140.0, 15.0);
        assert!(
            captured
                .borrow()
                .contains(&EditorAction::TabBar(TabBarAction::Activate {
                    client_id: 1
                })),
            "click must hit the scrolled card 1, actions: {:?}",
            captured.borrow()
        );
        // The close glyph inside a scrolled card still wins: card 1's close
        // is [246, 260] at scroll 100.
        tab_bar_click(&mut render_root, 253.0, 15.0);
        assert!(
            captured
                .borrow()
                .contains(&EditorAction::TabBar(TabBarAction::Close { client_id: 1 })),
            "close glyph must win inside scrolled cards, actions: {:?}",
            captured.borrow()
        );
    }

    #[test]
    fn activating_offscreen_tab_scrolls_it_into_view() {
        let (mut render_root, shell_id, _) = tab_bar_two_card_root();
        // Ten cards (widths 180x5, 124, 100x4) → scroll_max = 412; tabs 0 and 2
        // are mounted.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                (0..10)
                    .map(|i| TabCard {
                        client_id: i,
                        name: format!("t{i}"),
                        closable: true,
                    })
                    .collect(),
            );
        });
        // Wheel to the far right (active card 2 rides out of view on the left).
        for _ in 0..50 {
            tab_bar_wheel_event(&mut render_root, 300.0, 15.0, 0.0, 3.0);
        }
        // Programmatic activation scrolls the target card back into view.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert!(shell.widget.set_active_tab(&mut shell.ctx, 0));
        });
        let scroll = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.tab_bar_scroll()
        });
        assert_eq!(scroll, 4.0, "activation reveals the off-screen left card");
        // Registry-driven reorder (active card 0 moves to the end) scrolls it
        // back into the strip.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                (1..10)
                    .chain(0..1)
                    .map(|i| TabCard {
                        client_id: i,
                        name: format!("t{i}"),
                        closable: true,
                    })
                    .collect(),
            );
        });
        let scroll = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.tab_bar_scroll()
        });
        assert_eq!(scroll, 516.0, "reorder reveals the trailing active card");
    }

    #[test]
    fn rekey_tab_moves_chrome_and_keeps_widget_ids_stable() {
        let shell = ClayShellWidget::single_editor(0, EditorWidget::default());
        let first_chrome_id = shell.editor_widget_id();
        let second = second_tab_chrome();
        let second_chrome_id = second.editor_widget_id();
        let shell_new = NewWidget::new(shell);
        let shell_id = shell_new.id();
        let mut render_root = RenderRoot::new(shell_new, |_| {}, render_root_options());
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.install_tab(&mut shell.ctx, 2, second);
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                vec![
                    TabCard {
                        client_id: 0,
                        name: "alpha".into(),
                        closable: true,
                    },
                    TabCard {
                        client_id: 2,
                        name: "beta".into(),
                        closable: true,
                    },
                ],
            );
        });

        // Reconnect re-keys tab 2 to its new connection's client id (7).
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert!(shell.widget.rekey_tab(&mut shell.ctx, 2, 7));
            assert!(!shell.widget.rekey_tab(&mut shell.ctx, 2, 9), "unknown tab");
        });
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            // Chrome identity (and therefore routing) is unchanged.
            assert_eq!(shell.widget.editor_widget_id_for(7), Some(second_chrome_id));
            assert_eq!(shell.widget.editor_widget_id_for(2), None);
            assert_eq!(shell.widget.tab_for_chrome(second_chrome_id), Some(7));
            assert_eq!(shell.widget.pane_targets_for(7).len(), 1);
            // The card's client id follows so bar clicks keep working.
            assert_eq!(shell.widget.tab_cards[1].client_id, 7);
        });
        // Re-keying the active tab moves active_tab to the new key.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert!(shell.widget.rekey_tab(&mut shell.ctx, 0, 11));
            assert_eq!(shell.widget.active_tab, 11);
        });
        let _ = first_chrome_id;
    }

    #[test]
    fn tab_bar_new_tab_affordance_sits_at_bar_right_and_submits_new_tab() {
        let (mut render_root, shell_id, captured) = tab_bar_two_card_root();
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            let geometry = shell
                .widget
                .tab_bar_geometry(Size::new(900.0, 600.0))
                .expect("bar visible");
            // The affordance is a square at the bar's right edge (gap 4).
            assert_eq!(
                geometry.new_tab_rect,
                Rect::new(900.0 - 28.0 - 4.0, 1.0, 900.0 - 4.0, 29.0)
            );
        });
        // Click the affordance center → NewTab action.
        tab_bar_click(&mut render_root, 900.0 - 4.0 - 14.0, 15.0);
        let actions = captured.borrow();
        assert!(actions.contains(&EditorAction::TabBar(TabBarAction::NewTab)));
        // A single card hides the bar and the affordance with it.
        let single = ClayShellWidget::single_editor(0, EditorWidget::default());
        let single_new = NewWidget::new(single);
        let single_id = single_new.id();
        let mut single_root = RenderRoot::new(single_new, |_| {}, render_root_options());
        single_root.edit_widget(single_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                vec![TabCard {
                    client_id: 0,
                    name: "alpha".to_string(),
                    closable: true,
                }],
            );
            assert!(
                shell
                    .widget
                    .tab_bar_geometry(Size::new(900.0, 600.0))
                    .is_none()
            );
        });
        let _ = single;
    }

    #[test]
    fn tab_bar_close_glyph_click_submits_close() {
        let (mut render_root, _shell_id, captured) = tab_bar_two_card_root();
        // Click card 0's close glyph (center ≈ (169, 15)).
        tab_bar_click(&mut render_root, 169.0, 15.0);
        let actions = captured.borrow();
        assert!(actions.contains(&EditorAction::TabBar(TabBarAction::Close { client_id: 0 })));
    }

    #[test]
    fn tab_bar_click_on_active_card_is_a_noop() {
        let (mut render_root, _shell_id, captured) = tab_bar_two_card_root();
        // Card 1 is the active tab (client 2): clicking it activates nothing.
        tab_bar_click(&mut render_root, 278.0, 15.0);
        assert!(captured.borrow().is_empty());
    }

    #[test]
    fn tab_bar_hover_tracks_cards_and_clears_outside() {
        let (mut render_root, shell_id, _) = tab_bar_two_card_root();
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert_eq!(shell.widget.tab_bar_hover_index(), None);
        });
        // Move over card 0.
        render_root.handle_pointer_event(pointer_move_event(94.0, 15.0));
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert_eq!(shell.widget.tab_bar_hover_index(), Some(0));
            assert!(!shell.widget.tab_bar_new_tab_hover);
        });
        // Move over the pinned new-tab affordance.
        render_root.handle_pointer_event(pointer_move_event(882.0, 15.0));
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert_eq!(shell.widget.tab_bar_hover_index(), None);
            assert!(shell.widget.tab_bar_new_tab_hover);
        });
        // Move into the working area (below the bar): hover clears.
        render_root.handle_pointer_event(pointer_move_event(94.0, 300.0));
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert_eq!(shell.widget.tab_bar_hover_index(), None);
        });
    }

    #[test]
    fn remove_tab_uninstalls_hosts_and_switches_to_first_remaining() {
        let (mut render_root, shell_id, _) = tab_bar_two_card_root();
        // Removing the active tab (2) leaves tab 0 active with its single
        // host still registered.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.remove_tab(&mut shell.ctx, 2);
        });
        render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert_eq!(
                shell.widget.working_area_layout().active_pane_id(),
                PaneId(1)
            );
            let ids = shell.widget.children_ids();
            eprintln!("children ids: {:?}", ids);
            assert_eq!(ids.len(), 1);
            assert_eq!(shell.widget.tab_bar_hover_index(), None);
        });
    }

    // -- Phase 22.7 (task 2): zero-tab invariant ----------------------------

    /// All `ShellClientCommand` variants (the shell must no-op each one on a
    /// zero-tab shell).
    fn all_shell_commands() -> Vec<ShellClientCommand> {
        vec![
            ShellClientCommand::SplitPaneVertical,
            ShellClientCommand::SplitPaneHorizontal,
            ShellClientCommand::AddEqualPane,
            ShellClientCommand::ClosePane,
            ShellClientCommand::FocusPaneNext,
            ShellClientCommand::FocusPanePrev,
            ShellClientCommand::ResizePaneLeft,
            ShellClientCommand::ResizePaneRight,
            ShellClientCommand::ResizePaneUp,
            ShellClientCommand::ResizePaneDown,
            ShellClientCommand::MovePaneNext,
            ShellClientCommand::MovePanePrev,
            ShellClientCommand::TabNext,
            ShellClientCommand::TabPrev,
            ShellClientCommand::TabNew,
            ShellClientCommand::TabClose,
            ShellClientCommand::TabMoveLeft,
            ShellClientCommand::TabMoveRight,
            ShellClientCommand::TabActivate(1),
            ShellClientCommand::TabMoveTo(1),
        ]
    }

    #[test]
    fn zero_tab_shell_is_inert() {
        let (mut render_root, shell_id, _captured) = tab_bar_shell_root();
        // Remove the only mounted tab: the shell enters the zero-tab state
        // (the removed `active_tab` value stays in the field; nothing may
        // dereference it).
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.remove_tab(&mut shell.ctx, 0);
        });
        assert_eq!(
            render_root.edit_widget(shell_id, |mut widget| {
                let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
                shell.widget.children_ids().len()
            }),
            0
        );

        // Layout + paint + post_paint with zero tabs: no panic, empty scene.
        let (scene, _) = render_root.redraw();
        assert!(scene.encoding().is_empty(), "zero-tab shell paints nothing");

        // Pointer events are all ignored (the zero-tab guard sits at the top
        // of `on_pointer_event`, before any variant is matched).
        tab_bar_click(&mut render_root, 50.0, 50.0);
        render_root.handle_pointer_event(pointer_move_event(50.0, 50.0));
        render_root.handle_pointer_event(pointer_button_event(50.0, 50.0, None));

        // A text event (Tab focus navigation) is a no-op too.
        render_root.handle_text_event(TextEvent::Keyboard(KeyboardEvent {
            state: KeyState::Down,
            key: Key::Named(NamedKey::Tab),
            code: Code::Tab,
            ..KeyboardEvent::default()
        }));

        // Every shell client command variant is a no-op.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            for command in all_shell_commands() {
                shell
                    .widget
                    .apply_shell_client_command(&mut shell.ctx, command);
            }
        });

        // The accessibility tree still builds: one group, no active pane,
        // the polite announcement node registered.
        let update = access_tree(&mut render_root);
        let groups = nodes_with_role(&update, Role::Group);
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].1.label().map(str::to_string),
            Some("Clay working area shell. No mounted tabs.".to_string())
        );
        assert!(announcement_label(&update).is_some());

        // Still inert after a further redraw.
        let (scene, _) = render_root.redraw();
        assert!(scene.encoding().is_empty());
    }

    #[test]
    fn remove_last_tab_then_reinstall() {
        let (mut render_root, shell_id, _captured) = tab_bar_shell_root();
        // Removing the last tab leaves the zero-tab inert state.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.remove_tab(&mut shell.ctx, 0);
        });
        let (scene, _) = render_root.redraw();
        assert!(scene.encoding().is_empty());

        // Reinstalling a tab makes it active again and re-attaches chrome.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.install_tab(
                &mut shell.ctx,
                0,
                TabChrome::single_editor(EditorWidget::default(), false),
            );
            assert_eq!(shell.widget.active_pane_id(), PaneId(1));
            // Already active: the reinstall made the first tab active.
            assert!(!shell.widget.set_active_tab(&mut shell.ctx, 0));
            assert_eq!(shell.widget.children_ids().len(), 1);
        });
        // The reinstalled shell lays out and paints again.
        let _ = render_root.redraw();
        let update = access_tree(&mut render_root);
        let groups = nodes_with_role(&update, Role::Group);
        assert!(
            groups
                .iter()
                .any(|(_, n)| n.label().is_some_and(|l| l.contains("Active pane")))
        );
    }

    // -- Phase 22.6: accessibility tree structure ---------------------------

    /// Enable the AccessKit tree and rebuild it.
    fn access_tree(render_root: &mut RenderRoot) -> masonry::accesskit::TreeUpdate {
        render_root.handle_window_event(masonry::core::WindowEvent::EnableAccessTree);
        let (_, update) = render_root.redraw();
        update.expect("access tree is active after EnableAccessTree")
    }

    /// Phase 22.6 (task 4): the polite live-region announcement node's
    /// current label (`None` when the node has no text).
    fn announcement_label(update: &masonry::accesskit::TreeUpdate) -> Option<String> {
        update
            .nodes
            .iter()
            .find(|(_, node)| node.live() == Some(masonry::accesskit::Live::Polite))
            .and_then(|(_, node)| node.label().map(str::to_string))
    }

    fn nodes_with_role(
        update: &masonry::accesskit::TreeUpdate,
        role: Role,
    ) -> Vec<(NodeId, &Node)> {
        update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() == role)
            .map(|(id, node)| (*id, node))
            .collect()
    }

    #[test]
    fn tab_bar_accessibility_exposes_tablist_with_selected_active_card() {
        // Two mounted tabs (clients 0 and 2); card 0 = "alpha", card 1 =
        // "beta"; the active tab is client 2.
        let (mut render_root, _shell_id, _captured) = tab_bar_two_card_root();
        let update = access_tree(&mut render_root);

        let lists = nodes_with_role(&update, Role::TabList);
        assert_eq!(lists.len(), 1, "exactly one TabList for the tab bar");
        let (_, list) = lists[0];
        assert_eq!(list.label(), Some("Workspace tabs"));

        let tabs = nodes_with_role(&update, Role::Tab);
        assert_eq!(tabs.len(), 2);
        let mut labels: Vec<&str> = tabs
            .iter()
            .map(|(_, node)| node.label().expect("tab label"))
            .collect();
        labels.sort_unstable();
        assert_eq!(labels, vec!["alpha", "beta"]);
        for (id, node) in list.children().iter().map(|id| {
            (
                *id,
                update
                    .nodes
                    .iter()
                    .find(|(node_id, _)| *node_id == *id)
                    .map(|(_, node)| node)
                    .expect("TabList child node exists"),
            )
        }) {
            assert_eq!(node.role(), Role::Tab, "TabList child {id:?} is a Tab");
        }
        // The active card ("beta") is selected; the other is not.
        for (_id, node) in &tabs {
            let expected = node.label() == Some("beta");
            if expected {
                assert_eq!(node.is_selected(), Some(true));
            } else {
                assert_ne!(node.is_selected(), Some(true));
            }
        }
    }

    #[test]
    fn shell_accessibility_tree_leads_with_tablist_and_hides_inactive_tabs() {
        let (mut render_root, shell_id, _captured) = tab_bar_two_card_root();
        // Inactive tab hosts stay in `children_ids` (register_children
        // requires it) but are stashed by shell::layout, so the walk never
        // emits them and the consumer never sees an unattached node.
        let (all_hosts, active_hosts) = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            let all: Vec<WidgetId> = shell.widget.children_ids().iter().copied().collect();
            let active: Vec<WidgetId> = shell
                .widget
                .pane_host_ids()
                .into_iter()
                .map(|(_, id)| id)
                .collect();
            (all, active)
        });
        assert_eq!(all_hosts.len(), 2);
        assert_eq!(active_hosts.len(), 1);

        let update = access_tree(&mut render_root);
        let shell_node = update
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId::from(shell_id))
            .map(|(_, node)| node)
            .expect("shell node present");
        let children = shell_node.children();

        // TabList first, then the mounted tab's pane hosts only.
        let (list_id, _) = nodes_with_role(&update, Role::TabList)[0];
        assert_eq!(children.first().copied(), Some(list_id));
        for host in &active_hosts {
            assert!(children.contains(&NodeId::from(*host)));
        }
        for host in all_hosts.iter().filter(|id| !active_hosts.contains(id)) {
            assert!(
                !children.contains(&NodeId::from(*host)),
                "inactive tab host {host:?} must not be announced"
            );
        }
        assert_eq!(
            nodes_with_role(&update, Role::Pane).len(),
            1,
            "the walk emits only the active tab's pane host"
        );
    }

    #[test]
    fn single_tab_accessibility_tree_has_no_tablist() {
        let (mut render_root, shell_id) = shell_command_root();
        let update = access_tree(&mut render_root);
        assert!(
            nodes_with_role(&update, Role::TabList).is_empty(),
            "hidden tab bar must not appear in the accessibility tree"
        );
        let shell_node = update
            .nodes
            .iter()
            .find(|(id, _)| *id == NodeId::from(shell_id))
            .map(|(_, node)| node)
            .expect("shell node present");
        assert_eq!(
            shell_node.children().len(),
            2,
            "the single pane host plus the live announcement node"
        );
        // The live region is present from tree start (so ATs register it)
        // with no text until the first window-model action.
        assert_eq!(announcement_label(&update), None);
    }

    // -- Plan 086 task 3: consumer-valid incremental updates -----------------

    /// No-op consumer change handler: validation is the panic-free update.
    struct NoopChangeHandler;

    impl accesskit_consumer::TreeChangeHandler for NoopChangeHandler {
        fn node_added(&mut self, _node: &accesskit_consumer::Node) {}
        fn node_updated(
            &mut self,
            _old: &accesskit_consumer::Node,
            _new: &accesskit_consumer::Node,
        ) {
        }
        fn focus_moved(
            &mut self,
            _old: Option<&accesskit_consumer::Node>,
            _new: Option<&accesskit_consumer::Node>,
        ) {
        }
        fn node_removed(&mut self, _node: &accesskit_consumer::Node) {}
    }

    /// Run the real first update through `accesskit_consumer::Tree` exactly
    /// as `accesskit_unix` does on the live desktop; panics on any orphaned
    /// or unattached node.
    fn consumer_tree(render_root: &mut RenderRoot) -> accesskit_consumer::Tree {
        accesskit_consumer::Tree::new(access_tree(render_root), false)
    }

    fn consumer_update(tree: &mut accesskit_consumer::Tree, render_root: &mut RenderRoot) {
        tree.update_and_process_changes(access_tree(render_root), &mut NoopChangeHandler);
    }

    /// Every node reachable from the tree root (the consumer itself rejects
    /// unreachable nodes on update; this walks the accepted tree).
    fn reachable_ids(tree: &accesskit_consumer::Tree) -> std::collections::HashSet<NodeId> {
        let mut ids = std::collections::HashSet::new();
        let mut stack = vec![tree.state().root_id()];
        while let Some(id) = stack.pop() {
            if !ids.insert(id) {
                continue;
            }
            if let Some(node) = tree.state().node_by_id(id) {
                stack.extend(node.child_ids());
            }
        }
        ids
    }

    fn reachable_labels_with_role(tree: &accesskit_consumer::Tree, role: Role) -> Vec<String> {
        let mut out = Vec::new();
        for id in reachable_ids(tree) {
            if let Some(node) = tree.state().node_by_id(id) {
                let data = node.data();
                if data.role() == role {
                    out.push(data.label().unwrap_or("").to_string());
                }
            }
        }
        out
    }

    #[test]
    fn consumer_accepts_single_tab_initial_tree_with_region_attached() {
        // P0-1 regression: the first update used to panic in
        // accesskit_consumer ("neither in the current tree nor a child of
        // another node from the update: [#1]") because Masonry's walk
        // emitted the package region while the editor's accessibility()
        // omitted it from its children when no sidebar was present.
        let editor = crate::masonry_editor::EditorWidget::default();
        let (mut render_root, editor_widget_id) = render_root_for_shell(editor);
        let mut tree = consumer_tree(&mut render_root);

        // The editor node and its stable status node are attached and
        // reachable; an unchanged redraw produces no churn and no panic.
        let editor_node_id = NodeId::from(editor_widget_id);
        let reachable = reachable_ids(&tree);
        assert!(reachable.contains(&editor_node_id));
        let status_id = crate::editor::accessibility::virtual_a11y_node_id(
            editor_widget_id,
            crate::editor::accessibility::virtual_a11y_slots::STATUS,
        );
        assert!(
            reachable.contains(&status_id),
            "stable status node reachable"
        );
        let children: Vec<NodeId> = tree
            .state()
            .node_by_id(editor_node_id)
            .expect("editor node present")
            .child_ids()
            .collect();
        assert!(
            children.contains(&status_id),
            "status node attached to editor"
        );
        consumer_update(&mut tree, &mut render_root);
    }

    #[test]
    fn consumer_accepts_multi_tab_incremental_updates_and_drops_stale_nodes() {
        // P0-1 regression: the two-tab first update used to panic with three
        // orphans (both regions plus the inactive tab's host) because the
        // walk emitted every tab's hosts while the shell attached only the
        // active tab's.
        let (mut render_root, shell_id, _captured) = tab_bar_shell_root();
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.install_tab(
                &mut shell.ctx,
                2,
                TabChrome::single_editor(crate::masonry_editor::EditorWidget::default(), false),
            );
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                vec![
                    TabCard {
                        client_id: 0,
                        name: "alpha".to_string(),
                        closable: true,
                    },
                    TabCard {
                        client_id: 2,
                        name: "beta".to_string(),
                        closable: true,
                    },
                ],
            );
            shell.widget.set_active_tab(&mut shell.ctx, 2);
        });
        let mut tree = consumer_tree(&mut render_root);

        // Initial tree: both Tabs reachable; exactly ONE pane node (the
        // active tab's host) — the inactive host is never emitted.
        let tabs = reachable_labels_with_role(&tree, Role::Tab);
        assert_eq!(tabs.len(), 2);
        assert!(tabs.iter().any(|label| label == "alpha"));
        assert!(tabs.iter().any(|label| label == "beta"));
        assert_eq!(
            reachable_labels_with_role(&tree, Role::Pane).len(),
            1,
            "inactive tab host must not be reachable"
        );

        // Unchanged redraw: accepted without churn.
        consumer_update(&mut tree, &mut render_root);

        // Announcement: the polite live region updates.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.announce_tab_created(&mut shell.ctx, "gamma");
        });
        consumer_update(&mut tree, &mut render_root);
        assert!(
            reachable_labels_with_role(&tree, Role::Status)
                .iter()
                .any(|label| label.contains("Opened tab")),
            "announcement reaches the live region"
        );

        // Tab-add + selected-tab: install client 3 and activate it.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.install_tab(
                &mut shell.ctx,
                3,
                TabChrome::single_editor(crate::masonry_editor::EditorWidget::default(), false),
            );
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                vec![
                    TabCard {
                        client_id: 0,
                        name: "alpha".to_string(),
                        closable: true,
                    },
                    TabCard {
                        client_id: 2,
                        name: "beta".to_string(),
                        closable: true,
                    },
                    TabCard {
                        client_id: 3,
                        name: "gamma".to_string(),
                        closable: true,
                    },
                ],
            );
            shell.widget.set_active_tab(&mut shell.ctx, 3);
        });
        consumer_update(&mut tree, &mut render_root);
        assert_eq!(reachable_labels_with_role(&tree, Role::Tab).len(), 3);

        // Tab-reorder: registry order changes; ids are client-derived, so
        // the same tabs keep their node ids (no remove/add churn).
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                vec![
                    TabCard {
                        client_id: 3,
                        name: "gamma".to_string(),
                        closable: true,
                    },
                    TabCard {
                        client_id: 0,
                        name: "alpha".to_string(),
                        closable: true,
                    },
                    TabCard {
                        client_id: 2,
                        name: "beta".to_string(),
                        closable: true,
                    },
                ],
            );
        });
        consumer_update(&mut tree, &mut render_root);
        assert_eq!(reachable_labels_with_role(&tree, Role::Tab).len(), 3);

        // Tab-remove: alpha leaves; its Tab node must vanish from the
        // reachable tree (stale removed virtual nodes absent).
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.remove_tab(&mut shell.ctx, 0);
            shell.widget.set_tab_cards(
                &mut shell.ctx,
                vec![
                    TabCard {
                        client_id: 3,
                        name: "gamma".to_string(),
                        closable: true,
                    },
                    TabCard {
                        client_id: 2,
                        name: "beta".to_string(),
                        closable: true,
                    },
                ],
            );
        });
        consumer_update(&mut tree, &mut render_root);
        let tabs = reachable_labels_with_role(&tree, Role::Tab);
        assert_eq!(tabs.len(), 2);
        assert!(!tabs.iter().any(|label| label == "alpha"));

        // Status/label update: mount a live document view (driver mount
        // flow) and set its display name; the change flows through the
        // consumer and stays basename-sanitized (no host path leaks).
        let active_pane_id = render_root.edit_widget(shell_id, |mut widget| {
            let shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.pane_host_ids()[0].0
        });
        let view = PaneDocumentView::new(
            active_pane_id,
            std::rc::Rc::new(std::cell::Cell::new(1)),
            std::rc::Rc::new(std::cell::Cell::new(0)),
        );
        mount_document_view(&mut render_root, shell_id, active_pane_id, view);
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_pane_document_name(
                &mut shell.ctx,
                3,
                active_pane_id,
                Some("/home/user/secret/report.md"),
            );
        });
        consumer_update(&mut tree, &mut render_root);
        let panes = reachable_labels_with_role(&tree, Role::Pane);
        assert!(
            panes.iter().any(|label| label.contains("report.md")),
            "pane label updates through the consumer"
        );
        assert!(
            !panes.iter().any(|label| label.contains("/home/")),
            "host paths never reach the tree"
        );
    }

    #[test]
    fn pane_accessibility_labels_number_panes_and_name_documents() {
        let (mut render_root, shell_id) = shell_command_root();
        dispatch_shell_command(
            &mut render_root,
            shell_id,
            ShellClientCommand::SplitPaneVertical,
        );
        assert_eq!(pane_count(&mut render_root, shell_id), 2);

        // Pane 2 gets a live document view (driver mount flow) and a raw
        // host path, which must surface as the sanitized basename only.
        let view = PaneDocumentView::new(
            PaneId(2),
            std::rc::Rc::new(std::cell::Cell::new(1)),
            std::rc::Rc::new(std::cell::Cell::new(0)),
        );
        mount_document_view(&mut render_root, shell_id, PaneId(2), view);
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.set_pane_document_name(
                &mut shell.ctx,
                0,
                PaneId(2),
                Some("/home/alice/secret/note.md"),
            );
        });

        let update = access_tree(&mut render_root);
        let mut labels: Vec<String> = nodes_with_role(&update, Role::Pane)
            .into_iter()
            .map(|(_, node)| node.label().expect("pane label").to_string())
            .collect();
        labels.sort();
        assert_eq!(labels, vec!["Pane 1 of 2: editor", "Pane 2 of 2: note.md"]);
    }

    #[test]
    fn empty_pane_accessibility_label_reports_count() {
        let (mut render_root, shell_id) = shell_command_root();
        dispatch_shell_command(
            &mut render_root,
            shell_id,
            ShellClientCommand::SplitPaneVertical,
        );
        let update = access_tree(&mut render_root);
        let mut labels: Vec<String> = nodes_with_role(&update, Role::Pane)
            .into_iter()
            .map(|(_, node)| node.label().expect("pane label").to_string())
            .collect();
        labels.sort();
        assert_eq!(labels, vec!["Empty pane 2 of 2", "Pane 1 of 2: editor"]);
    }
    // -- Phase 22.6: screen-reader announcements (task 4) --

    #[test]
    fn announcement_builder_composes_exact_strings() {
        assert_eq!(
            compose_announcement(AnnouncementKind::TabActivated, Some("notes"), 2, 0),
            "Switched to tab 2: notes"
        );
        assert_eq!(
            compose_announcement(
                AnnouncementKind::TabCreated,
                Some("/home/alice/notes"),
                3,
                0
            ),
            // Sanitized basename: the host path never reaches the live region.
            "Opened tab 3: notes"
        );
        assert_eq!(
            compose_announcement(AnnouncementKind::TabClosed, Some("beta"), 2, 1),
            "Closed tab 2: beta; 1 tab open"
        );
        assert_eq!(
            compose_announcement(AnnouncementKind::TabClosed, Some("beta"), 2, 3),
            "Closed tab 2: beta; 3 tabs open"
        );
        assert_eq!(
            compose_announcement(AnnouncementKind::SplitPaneVertical, None, 0, 2),
            "Split pane vertically"
        );
        assert_eq!(
            compose_announcement(AnnouncementKind::SplitPaneHorizontal, None, 0, 2),
            "Split pane horizontally"
        );
        assert_eq!(
            compose_announcement(AnnouncementKind::PaneAdded, None, 0, 3),
            "Added pane"
        );
        assert_eq!(
            compose_announcement(AnnouncementKind::PaneClosed, None, 0, 1),
            "Closed pane; 1 pane remains"
        );
        assert_eq!(
            compose_announcement(AnnouncementKind::PaneClosed, None, 0, 2),
            "Closed pane; 2 panes remain"
        );
        assert_eq!(
            compose_announcement(AnnouncementKind::PaneMovedForward, None, 0, 2),
            "Moved pane forward"
        );
        assert_eq!(
            compose_announcement(AnnouncementKind::PaneMovedBackward, None, 0, 2),
            "Moved pane backward"
        );
    }

    #[test]
    fn announcement_text_is_capped_and_path_free() {
        // The display-name sanitizer caps at 64 chars before composition, so
        // the composed cap (256) is a backstop for future longer formats.
        let long = "x".repeat(300);
        let text = compose_announcement(AnnouncementKind::TabCreated, Some(&long), 1, 0);
        assert_eq!(
            text,
            format!(
                "Opened tab 1: {}",
                "x".repeat(crate::editor::accessibility::ACCESSIBILITY_DISPLAY_NAME_MAX_CHARS)
            )
        );
        assert!(text.chars().count() <= ANNOUNCEMENT_MAX_CHARS);
        let path = compose_announcement(
            AnnouncementKind::TabActivated,
            Some("/home/alice/secret/workspace"),
            2,
            0,
        );
        assert_eq!(path, "Switched to tab 2: workspace");
        assert!(!path.contains('/'));
    }

    #[test]
    fn tab_switch_and_close_announce_exact_strings() {
        let (mut render_root, shell_id, _captured) = tab_bar_two_card_root();
        // User activates tab 0 ("alpha"); the driver announces after the
        // successful switch.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert!(shell.widget.set_active_tab(&mut shell.ctx, 0));
            shell.widget.announce_tab_activated(&mut shell.ctx, 0);
        });
        assert_eq!(
            announcement_label(&access_tree(&mut render_root)).as_deref(),
            Some("Switched to tab 1: alpha")
        );
        // Close the other tab; the count reflects the post-close map.
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            shell.widget.remove_tab(&mut shell.ctx, 2);
        });
        assert_eq!(
            announcement_label(&access_tree(&mut render_root)).as_deref(),
            Some("Closed tab 2: beta; 1 tab open")
        );
    }

    #[test]
    fn split_commands_announce_one_message_each_and_noops_stay_silent() {
        let (mut render_root, shell_id) = shell_command_root();
        dispatch_shell_command(
            &mut render_root,
            shell_id,
            ShellClientCommand::SplitPaneVertical,
        );
        assert_eq!(
            announcement_label(&access_tree(&mut render_root)).as_deref(),
            Some("Split pane vertically")
        );
        dispatch_shell_command(&mut render_root, shell_id, ShellClientCommand::ClosePane);
        assert_eq!(
            announcement_label(&access_tree(&mut render_root)).as_deref(),
            Some("Closed pane; 1 pane remains")
        );
        // A bounds no-op (single-pane close) announces nothing new.
        dispatch_shell_command(&mut render_root, shell_id, ShellClientCommand::ClosePane);
        assert_eq!(
            announcement_label(&access_tree(&mut render_root)).as_deref(),
            Some("Closed pane; 1 pane remains")
        );
        dispatch_shell_command(&mut render_root, shell_id, ShellClientCommand::AddEqualPane);
        assert_eq!(
            announcement_label(&access_tree(&mut render_root)).as_deref(),
            Some("Added pane")
        );
        dispatch_shell_command(&mut render_root, shell_id, ShellClientCommand::MovePaneNext);
        assert_eq!(
            announcement_label(&access_tree(&mut render_root)).as_deref(),
            Some("Moved pane forward")
        );
        dispatch_shell_command(&mut render_root, shell_id, ShellClientCommand::MovePanePrev);
        assert_eq!(
            announcement_label(&access_tree(&mut render_root)).as_deref(),
            Some("Moved pane backward")
        );
        dispatch_shell_command(
            &mut render_root,
            shell_id,
            ShellClientCommand::SplitPaneHorizontal,
        );
        assert_eq!(
            announcement_label(&access_tree(&mut render_root)).as_deref(),
            Some("Split pane horizontally")
        );
    }

    #[test]
    fn tab_switch_submits_no_actions_or_messages() {
        // Phase 22.6 (plan 077 task 5): a tab switch is pure widget-tree
        // state — no editor actions, no client messages, no document
        // reserialization. The driver owns all queues/IPC; the shell must
        // not emit anything from the switch path.
        let (mut render_root, shell_id, captured) = tab_bar_two_card_root();
        render_root.edit_widget(shell_id, |mut widget| {
            let mut shell = widget.try_downcast::<ClayShellWidget>().expect("shell");
            assert!(shell.widget.set_active_tab(&mut shell.ctx, 0));
        });
        let _ = render_root.redraw();
        assert!(
            captured.borrow().is_empty(),
            "tab switch must not submit editor actions, got {:?}",
            captured.borrow()
        );
    }

    #[test]
    fn focus_moves_and_repaints_do_not_reannounce() {
        let (mut render_root, shell_id) = shell_command_root();
        dispatch_shell_command(
            &mut render_root,
            shell_id,
            ShellClientCommand::SplitPaneVertical,
        );
        let update = access_tree(&mut render_root);
        assert_eq!(
            announcement_label(&update).as_deref(),
            Some("Split pane vertically")
        );
        // A pure focus move keeps the previous announcement text.
        dispatch_shell_command(
            &mut render_root,
            shell_id,
            ShellClientCommand::FocusPaneNext,
        );
        assert_eq!(
            announcement_label(&access_tree(&mut render_root)).as_deref(),
            Some("Split pane vertically")
        );
    }
}
