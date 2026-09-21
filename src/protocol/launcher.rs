//! Launcher setup snapshots (workspace roots, agent entries).

/// One agent-delivered settings file (plan 117): bounded metadata for the
/// settings page listing. Inert: names + provenance only, never content.
/// One recently opened workspace root, as the launcher lists it. Display
/// data: the name is the folder's basename, the root the server's own stored
/// path (the webview never supplies one back).
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
    Default,
)]
#[serde(rename_all = "camelCase")]
pub struct LauncherWorkspaceEntry {
    pub name: String,
    pub root: String,
}

/// One configured agent type under the Clay data root's `agents/` folder.
/// Data only: nothing here loads a package or grants tool authority.
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
    Default,
)]
#[serde(rename_all = "camelCase")]
pub struct LauncherAgentEntry {
    /// Directory name (the agent type's identity).
    pub name: String,
    /// Human label for the row (`coding-agent` → `Coding Agent`).
    pub label: String,
    /// Config root, home-relative for display.
    pub config_root: String,
    /// Seeded skills under `<config root>/skills/`.
    pub skill_count: u32,
}

/// Server-resolved launcher payload: recent workspaces and configured agent
/// types. `pruned` counts recents dropped because the folder is gone.
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
    Default,
)]
#[serde(rename_all = "camelCase")]
pub struct LauncherEntries {
    pub workspaces: Vec<LauncherWorkspaceEntry>,
    pub agents: Vec<LauncherAgentEntry>,
    pub pruned: u32,
}
