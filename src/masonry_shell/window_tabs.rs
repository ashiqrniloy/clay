//! Shell tab/window layer data vocabulary.
//!
//! Pure data types + methods for the tab/window working layer: pane focus
//! policy, client-routed shell commands, tab bar cards/geometry, and one
//! tab's chrome state (split tree + retained pane hosts + routing targets).
//! `ClayShellWidget` (in `mod.rs`) owns the live window state and hot paths;
//! these types hold no Masonry/`EditorSurface` coupling and are leaf data.

use std::collections::{BTreeMap, BTreeSet};

use masonry::core::{NewWidget, WidgetId, WidgetPod};
use masonry::kurbo::Rect;

use crate::editor::typography::TypographyRegistry;
use crate::masonry_editor::EditorWidget;
use crate::masonry_pane_host::PaneContentHost;
use crate::protocol::ClientId;
use crate::shell::{PaneId, WorkingAreaLayout};

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
    pub(super) layout: WorkingAreaLayout,
    /// Phase 22.1: one retained content host per pane leaf, keyed by pane ID.
    pub(super) pane_hosts: BTreeMap<PaneId, WidgetPod<PaneContentHost>>,
    /// `WidgetId` of the hosted `EditorWidget` (mounted once in Phase 22.1).
    pub(super) editor_widget_id: WidgetId,
    /// The pane that mounts the connection owner (`PaneContent::Editor`).
    /// Fixed at construction; closing this pane orphans the owner rather than
    /// detaching it (see `chrome_orphans`).
    pub(super) editor_pane_id: PaneId,
    /// The connection owner's host after its pane closed (zero-size orphan).
    /// Unlike `pending_orphans`, these are NEVER detached: the owner must stay
    /// in the tree so `editor_widget_id` remains editable and connection
    /// events keep applying. Registered and laid out at zero size forever.
    pub(super) chrome_orphans: Vec<WidgetPod<PaneContentHost>>,
    /// Phase 22.2: pane → content widget id for keyboard/event routing. Pane 1
    /// maps to the chrome (`editor_widget_id`); document panes map to their
    /// `PaneDocumentView` (registered by the app driver when mounting).
    pub(super) pane_targets: BTreeMap<PaneId, WidgetId>,
    /// Hosts removed from the tree without a `MutateCtx` available. Detached by
    /// the next [`Self::reconcile_pane_hosts`] call; laid out at zero size until
    /// then so Masonry's canonical children list stays consistent.
    pub(super) pending_orphans: Vec<WidgetPod<PaneContentHost>>,
    /// Phase 22.1: pane activation policy (click vs focus-follows-cursor).
    pub(super) pane_focus_policy: PaneFocusPolicy,
    /// Phase 22.6: pane hosts already inserted in the Masonry tree by a
    /// register pass. Newly synced hosts are absent until the next register
    /// pass, and `MutateCtx::get_mut` panics on them, so accessibility count
    /// updates skip them (they receive the count at creation instead).
    pub(super) registered_panes: BTreeSet<PaneId>,
    /// Cached typography for this tab's shell chrome. Each tab can receive
    /// connection-scoped typography independently; the window shell copies
    /// the active tab's registry for tab-bar geometry and paint.
    pub(super) typography: TypographyRegistry,
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
        let typography = editor.typography().clone();
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
            typography,
        }
    }

    /// The chrome's widget id (the tab's event-bridge routing tag).
    #[doc(hidden)]
    pub fn editor_widget_id(&self) -> WidgetId {
        self.editor_widget_id
    }
}
