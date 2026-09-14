//! Launcher surface (plan 118 Part D): the start surface's server-side data.
//!
//! Recent workspace roots persist as plain paths in `launcher.json` under the
//! Clay data root (`~/.clay`), newest first, capped; entries whose directory
//! disappeared are pruned on read. Configured agent types are a bounded
//! directory scan of the data root's `agents/` folder — the same root the
//! agent's own config uses, so an agent appears exactly when it exists.
//!
//! Display data only: the webview receives names and paths, never supplies a
//! path back, and opening an entry rides the ordinary tab/root path. Recents
//! hold no credentials, and nothing here is auto-opened.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::configuration::ConfigurationRuntime;
use crate::protocol::{LauncherAgentEntry, LauncherEntries, LauncherWorkspaceEntry};

const STORE_FILE: &str = "launcher.json";
const AGENTS_DIR: &str = "agents";
const SKILLS_DIR: &str = "skills";
const STORE_VERSION: u32 = 1;

/// Recents kept; the launcher lists at most this many workspaces.
pub(crate) const MAX_RECENTS: usize = 8;
/// Abuse bound for the agent scan (`agents/` is a hand-made folder).
const MAX_AGENTS: usize = 32;
/// Cap on a single stored path, mirroring the protocol's string bounds.
const MAX_PATH_CHARS: usize = 4096;
/// Directory entries read while counting one agent's skills.
const MAX_SKILL_DIRS: usize = 64;

/// `{ "version": 1, "workspaces": ["/abs/path", ...] }`, newest first.
#[derive(Debug, Default, Deserialize, Serialize)]
struct RecentStore {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    workspaces: Vec<String>,
}

/// Clay data root: the explicit configuration root when set, else the user's
/// default (`~/.clay`). `None` ⇒ no launcher data (never an error).
fn data_root(configuration_root: Option<&Path>) -> Option<PathBuf> {
    configuration_root
        .map(Path::to_path_buf)
        .or_else(ConfigurationRuntime::default_config_root)
}

fn store_path(data_root: &Path) -> PathBuf {
    data_root.join(STORE_FILE)
}

/// Read the recents store. Missing, unreadable or malformed ⇒ empty list
/// (the launcher shows its first-run state instead of failing).
fn read_store(data_root: &Path) -> RecentStore {
    let Ok(raw) = fs::read_to_string(store_path(data_root)) else {
        return RecentStore::default();
    };
    let Ok(mut store) = serde_json::from_str::<RecentStore>(&raw) else {
        return RecentStore::default();
    };
    if store.version != STORE_VERSION {
        return RecentStore::default();
    }
    store.workspaces.retain(|path| {
        !path.is_empty() && path.chars().count() <= MAX_PATH_CHARS && !path.contains('\0')
    });
    store.workspaces.truncate(MAX_RECENTS);
    store
}

/// Write the recents store. Best-effort: a failed write never fails the
/// folder open that triggered it (temp + rename so a crash cannot truncate
/// the list).
fn write_store(data_root: &Path, store: &RecentStore) {
    let Ok(json) = serde_json::to_string_pretty(&RecentStore {
        version: STORE_VERSION,
        workspaces: store.workspaces.clone(),
    }) else {
        return;
    };
    let path = store_path(data_root);
    if fs::create_dir_all(data_root).is_err() {
        return;
    }
    let temp = data_root.join(format!("{STORE_FILE}.tmp"));
    if fs::write(&temp, json).is_ok() {
        let _ = fs::rename(&temp, &path);
    } else {
        let _ = fs::remove_file(&temp);
    }
}

/// Record a workspace root as the most recent one. Non-directories are
/// ignored (the launcher never lists a path it cannot open).
pub(crate) fn record_recent_workspace(configuration_root: Option<&Path>, root: &Path) {
    if !root.is_dir() {
        return;
    }
    let Some(data_root) = data_root(configuration_root) else {
        return;
    };
    let display = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let display = display.to_string_lossy().into_owned();
    if display.is_empty() || display.chars().count() > MAX_PATH_CHARS {
        return;
    }
    let mut store = read_store(&data_root);
    store.workspaces.retain(|path| path != &display);
    store.workspaces.insert(0, display);
    store.workspaces.truncate(MAX_RECENTS);
    write_store(&data_root, &store);
}

/// Drop one recent by index in the *server's* list (the webview never sends a
/// path). Out-of-range indices are no-ops.
pub(crate) fn remove_recent_workspace(configuration_root: Option<&Path>, index: u32) {
    let Some(data_root) = data_root(configuration_root) else {
        return;
    };
    let mut store = read_store(&data_root);
    let index = index as usize;
    if index >= store.workspaces.len() {
        return;
    }
    store.workspaces.remove(index);
    write_store(&data_root, &store);
}

/// Server-resolved launcher entries. `pruned` counts entries dropped because
/// their directory is gone (the caller surfaces it as a bounded diagnostic).
pub(crate) fn launcher_entries(configuration_root: Option<&Path>) -> LauncherEntries {
    let Some(data_root) = data_root(configuration_root) else {
        return LauncherEntries::default();
    };
    let mut store = read_store(&data_root);
    let before = store.workspaces.len();
    store.workspaces.retain(|path| Path::new(path).is_dir());
    let pruned = before - store.workspaces.len();
    if pruned > 0 {
        write_store(&data_root, &store);
    }
    let home = home_dir();
    LauncherEntries {
        workspaces: store
            .workspaces
            .iter()
            .map(|root| LauncherWorkspaceEntry {
                name: display_name(root),
                root: root.clone(),
            })
            .collect(),
        agents: list_agents(&data_root, home.as_deref()),
        pruned: pruned as u32,
    }
}

/// Configured agent types under `<data root>/agents/`: one per directory that
/// actually resolves. Sorted by name, capped, never panicking.
fn list_agents(data_root: &Path, home: Option<&Path>) -> Vec<LauncherAgentEntry> {
    let Ok(entries) = fs::read_dir(data_root.join(AGENTS_DIR)) else {
        return Vec::new();
    };
    let mut agents: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            (path.is_dir() && valid_agent_name(&name)).then_some((name, path))
        })
        .collect();
    agents.sort_by(|left, right| left.0.cmp(&right.0));
    agents.truncate(MAX_AGENTS);
    agents
        .into_iter()
        .map(|(name, path)| LauncherAgentEntry {
            label: display_label(&name),
            skill_count: count_skills(&path),
            config_root: tilde_display(&path, home),
            name,
        })
        .collect()
}

/// Resolve one agent type to its per-agent config root (plan 118 task 35).
///
/// The only accepted input is a bare directory name: `valid_agent_name`
/// rejects separators, `..`, and anything unbounded, so a name can never
/// address a path outside `<data root>/agents/`. The directory must actually
/// exist — an agent type the launcher would not list is not resolvable, so a
/// stale or hostile name resolves to `None` instead of a root that is not
/// configured. `configuration_root` is the Clay data root (explicit config
/// root, else `~/.clay`).
pub(crate) fn resolve_agent_type(
    configuration_root: Option<&Path>,
    agent_type: &str,
) -> Option<PathBuf> {
    let name = agent_type.trim();
    if !valid_agent_name(name) {
        return None;
    }
    let root = data_root(configuration_root)?;
    let path = root.join(AGENTS_DIR).join(name);
    path.is_dir().then_some(path)
}

/// Agent folder names are plain identifiers: bounded, no separators.
pub(crate) fn valid_agent_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Number of seeded skills (`skills/<name>/`) for one agent, bounded.
fn count_skills(agent_root: &Path) -> u32 {
    let Ok(entries) = fs::read_dir(agent_root.join(SKILLS_DIR)) else {
        return 0;
    };
    entries
        .flatten()
        .take(MAX_SKILL_DIRS)
        .filter(|entry| entry.path().is_dir())
        .count() as u32
}

/// `projects/clay` → `clay`; a path with no final component keeps its text.
fn display_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// `coding-agent` → `Coding Agent` (the launcher's row label).
fn display_label(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Replace a home-directory prefix with `~` for display only.
fn tilde_display(path: &Path, home: Option<&Path>) -> String {
    let text = path.to_string_lossy().into_owned();
    match home {
        Some(home) => {
            let prefix = home.to_string_lossy().into_owned();
            match text.strip_prefix(&prefix) {
                Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
                _ => text,
            }
        }
        None => text,
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "clay-launcher-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn workspace(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn recents_round_trip_newest_first_and_dedupe() {
        let data = temp_root("recents");
        let first = workspace(&data, "first");
        let second = workspace(&data, "second");

        record_recent_workspace(Some(&data), &first);
        record_recent_workspace(Some(&data), &second);
        record_recent_workspace(Some(&data), &first);

        let entries = launcher_entries(Some(&data));
        assert_eq!(entries.pruned, 0);
        let roots: Vec<&str> = entries
            .workspaces
            .iter()
            .map(|entry| entry.root.as_str())
            .collect();
        assert_eq!(roots.len(), 2);
        assert!(roots[0].ends_with("first"), "{roots:?}");
        assert!(roots[1].ends_with("second"), "{roots:?}");
        assert_eq!(entries.workspaces[0].name, "first");
    }

    #[test]
    fn recents_cap_and_prune_missing_directories() {
        let data = temp_root("prune");
        let mut paths = Vec::new();
        for index in 0..MAX_RECENTS + 2 {
            let path = workspace(&data, &format!("ws-{index}"));
            record_recent_workspace(Some(&data), &path);
            paths.push(path);
        }
        let entries = launcher_entries(Some(&data));
        assert_eq!(entries.workspaces.len(), MAX_RECENTS, "capped");

        // A folder deleted between sessions is pruned with a count.
        fs::remove_dir_all(&paths[MAX_RECENTS + 1]).unwrap();
        let entries = launcher_entries(Some(&data));
        assert_eq!(entries.pruned, 1);
        assert_eq!(entries.workspaces.len(), MAX_RECENTS - 1);
        assert!(
            entries
                .workspaces
                .iter()
                .all(|entry| !entry.root.ends_with("ws-9")),
            "{:?}",
            entries.workspaces
        );
        // Pruning is persisted: the next read reports nothing to prune.
        assert_eq!(launcher_entries(Some(&data)).pruned, 0);
    }

    #[test]
    fn remove_recent_drops_one_by_index_only() {
        let data = temp_root("remove");
        let first = workspace(&data, "keep");
        let second = workspace(&data, "drop");
        record_recent_workspace(Some(&data), &first);
        record_recent_workspace(Some(&data), &second);

        remove_recent_workspace(Some(&data), 0);
        let entries = launcher_entries(Some(&data));
        assert_eq!(entries.workspaces.len(), 1);
        assert!(entries.workspaces[0].root.ends_with("keep"));

        // Out-of-range indices are no-ops.
        remove_recent_workspace(Some(&data), 9);
        assert_eq!(launcher_entries(Some(&data)).workspaces.len(), 1);
    }

    #[test]
    fn malformed_store_degrades_to_first_run() {
        let data = temp_root("malformed");
        fs::write(store_path(&data), "{ not json").unwrap();
        assert!(launcher_entries(Some(&data)).workspaces.is_empty());
        fs::write(store_path(&data), r#"{"version":99,"workspaces":["/tmp"]}"#).unwrap();
        assert!(launcher_entries(Some(&data)).workspaces.is_empty());
    }

    #[test]
    fn agents_list_configured_directories_with_skill_counts() {
        let data = temp_root("agents");
        let agent = data.join(AGENTS_DIR).join("coding-agent");
        fs::create_dir_all(agent.join(SKILLS_DIR).join("one")).unwrap();
        fs::create_dir_all(agent.join(SKILLS_DIR).join("two")).unwrap();
        fs::create_dir_all(data.join(AGENTS_DIR).join("reviewer")).unwrap();
        // Files and traversing names are not agent types.
        fs::write(data.join(AGENTS_DIR).join("notes.md"), "x").unwrap();
        fs::create_dir_all(data.join(AGENTS_DIR).join("has space")).unwrap();

        let agents = launcher_entries(Some(&data)).agents;
        assert_eq!(
            agents
                .iter()
                .map(|entry| (entry.name.as_str(), entry.label.as_str(), entry.skill_count))
                .collect::<Vec<_>>(),
            vec![
                ("coding-agent", "Coding Agent", 2),
                ("reviewer", "Reviewer", 0)
            ]
        );
        assert!(agents[0].config_root.ends_with("agents/coding-agent"));
    }

    #[test]
    fn agent_types_resolve_only_contained_existing_directories() {
        let data = temp_root("resolve");
        let agent = data.join(AGENTS_DIR).join("reviewer");
        fs::create_dir_all(&agent).unwrap();
        fs::write(data.join(AGENTS_DIR).join("notes.md"), "x").unwrap();

        assert_eq!(
            resolve_agent_type(Some(&data), "reviewer"),
            Some(agent.clone())
        );
        // A name that is not a configured agent never resolves — and never
        // becomes a path: separators, traversal, and absolute names are all
        // rejected by the name rule before any filesystem access.
        for name in [
            "",
            " ",
            "missing",
            "../coding-agent",
            "..",
            "a/b",
            "/etc",
            "reviewer/../coding-agent",
            "notes.md",
            "has space",
            &"x".repeat(65),
        ] {
            assert!(
                resolve_agent_type(Some(&data), name).is_none(),
                "`{name}` must not resolve"
            );
        }
        // Surrounding whitespace is tolerated (the webview never sends it,
        // but a config file might).
        assert_eq!(resolve_agent_type(Some(&data), " reviewer "), Some(agent));
        // No data root ⇒ no resolution (fail closed).
        assert!(resolve_agent_type(None, "reviewer").is_none());
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn missing_data_root_lists_nothing_and_never_panics() {
        let root = temp_root("absent");
        let missing = root.join("nested-that-does-not-exist");
        let entries = launcher_entries(Some(&missing));
        assert!(entries.workspaces.is_empty());
        assert!(entries.agents.is_empty());
        // Listing is read-only: it must not create the data root as a side effect.
        assert!(!missing.exists(), "listing must never create the data root");
    }
}
