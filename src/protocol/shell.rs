//! Shell-owned client state: preferences and the server-authoritative tab
//! registry.

use super::*;

/// Phase 22.1 shell-level user preferences transported from the server (which
/// evaluates `init.js`) to the client (which owns the `ClayShellWidget`).
/// Currently carries only the pane-focus policy (`"click"` or `"cursor"`).
/// Pure inert data: no callbacks, no JS execution, no native handles.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct ShellPreferences {
    /// Pane-focus policy: `"click"` (default) or `"cursor"` (focus follows
    /// pointer hover). Validated server-side; the client maps the string to
    /// its `PaneFocusPolicy` enum.
    pub pane_focus_policy: String,
}

/// Phase 22.3: one tab in the server-authoritative tab registry. A tab is a
/// real separate client connection bound to a workspace root; the registry
/// holds order, the active tab, and the per-tab workspace + client binding so
/// tab structure survives client reconnects.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct TabEntry {
    pub tab_id: TabId,
    pub workspace_root_id: WorkspaceRootId,
    pub client_id: ClientId,
    /// Workspace root path for this tab (validated by `add_root` at
    /// `TabCommand::New`/`OpenWorkspace` time). The client displays the path's
    /// final segment as the tab card label. Phase 22.3.
    pub workspace_root: String,
}

/// Phase 22.3: full tab registry snapshot. Broadcast to every connection on
/// any mutation and replayed on handshake; the client applies it to its tab
/// bar and per-tab connection map. Inert data only.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase")]
pub struct TabRegistrySnapshot {
    /// Tab order (server-authoritative; reorderable since Phase 22.4 via
    /// `TabCommand::MoveLeft`/`MoveRight`/`MoveTo`).
    pub tabs: Vec<TabEntry>,
    pub active: Option<TabId>,
    /// Monotonic registry generation: bumped on every mutation. Relays from
    /// different connections can interleave out of order (a connection's
    /// handshake replay races the broadcast of its own pending tab command),
    /// so the client applies a snapshot only when its revision advances.
    pub revision: u64,
}

/// Phase 22.3: server-authoritative tab lifecycle command. `New` opens a tab
/// bound to the connection's `ClientId` and the given workspace root (resolved
/// through the validated `WorkspaceState::add_root` path); `OpenWorkspace`
/// rebinds a tab's workspace; `Close` removes the tab and triggers the bound
/// connection's close; `Activate` sets the active tab; `Reclaim` re-points a
/// tab's `ClientId` binding at the reconnecting connection (local single-client
/// reclaim in 22.3; multi-client reclaim needs client-instance identity, Phase
/// 21).
/// Phase 22.4: `MoveLeft`/`MoveRight` move a tab one position toward the
/// front/back (boundary no-ops — no wraparound); `MoveTo` moves a tab to a
/// 1-based position (`position` outside `1..=tab_count` is rejected). Moves
/// preserve the active tab's status (the registry tracks `active` by `TabId`).
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TabCommand {
    New {
        workspace_root: String,
    },
    OpenWorkspace {
        tab_id: TabId,
        root: String,
    },
    Close {
        tab_id: TabId,
    },
    Activate {
        tab_id: TabId,
    },
    Reclaim {
        tab_id: TabId,
    },
    MoveLeft {
        tab_id: TabId,
    },
    MoveRight {
        tab_id: TabId,
    },
    MoveTo {
        tab_id: TabId,
        position: u32,
    },
    /// Plan 118 task 35: the tab's agent type (the per-agent config root's
    /// directory name under the Clay data root's `agents/`). `None` detaches
    /// the tab from its agent. The server validates the name and that the
    /// agent actually resolves before the registry accepts it: this is the
    /// only way a tab's agent changes (tab chrome), and no absolute path ever
    /// travels on the wire.
    SetAgent {
        tab_id: TabId,
        agent: Option<String>,
    },
}
