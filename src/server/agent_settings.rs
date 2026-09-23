//! Agent settings file surface (plan 117): the coding agent's delivered
//! config files — `SYSTEM.md` plus every seeded `skills/<name>/SKILL.md`.
//! The server builds every path from its own configuration root; the webview
//! supplies only a name it got from the listing. Provenance compares each
//! file against the `.seed-manifest.json` the daemon writes when it seeds a
//! file (size + mtime): match ⇒ built-in seed, mismatch/absent ⇒ edited.
//!
//! Save is intentionally NOT here: opened files are ordinary documents, so
//! edits flow through the normal document pipeline (SaveDocument).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::configuration::ConfigurationRuntime;
use crate::protocol::AgentSettingsFileInfo;

const SYSTEM_MD: &str = "SYSTEM.md";
const SKILLS_DIR: &str = "skills";
const SKILL_FILE: &str = "SKILL.md";
const SEED_MANIFEST: &str = ".seed-manifest.json";
/// Fixed layout ⇒ the list is tiny; the cap is abuse protection only.
const MAX_LIST: usize = 64;
const MAX_SKILL_DIR: usize = 64;

/// Directory name of the agent the surface fell back to before agent types
/// existed (plan 118 task 35: a tab without an agent still shows this one's
/// delivered files — the shipped agent is what a fresh install runs).
pub(crate) const DEFAULT_AGENT_TYPE: &str = "coding-agent";

/// Resolved agent config root for one agent type: `<data root>/agents/<type>`.
///
/// Containment comes from the name rule (`valid_agent_name`), so a read can
/// only ever land inside `<data root>/agents/`; whether the directory exists
/// is the listing's business (a missing one lists nothing). `<data root>/agents/coding-agent`
/// is the default when the caller has no agent — the shipped agent a fresh
/// install runs, so a tab that never picked one still shows its files.
pub(crate) fn agent_config_root_for(
    configuration_root: Option<&Path>,
    agent_type: Option<&str>,
) -> Option<PathBuf> {
    let root = configuration_root
        .map(Path::to_path_buf)
        .or_else(ConfigurationRuntime::default_config_root)?;
    let name = agent_type
        .map(str::trim)
        .filter(|agent| !agent.is_empty())
        .filter(|agent| super::launcher::valid_agent_name(agent))
        .unwrap_or(DEFAULT_AGENT_TYPE);
    Some(root.join("agents").join(name))
}

/// Validate a client-supplied name against the fixed layout and return the
/// canonical relative path. By construction there is no traversal: the only
/// accepted shapes are `SYSTEM.md` and `skills/<dir>/SKILL.md` with `<dir>`
/// a bounded identifier.
fn validate_name(name: &str) -> Option<PathBuf> {
    if name == SYSTEM_MD {
        return Some(PathBuf::from(SYSTEM_MD));
    }
    let rest = name.strip_prefix("skills/")?;
    let (dir, file) = rest.split_once('/')?;
    if file != SKILL_FILE
        || dir.is_empty()
        || dir.len() > MAX_SKILL_DIR
        || !dir
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ')
    {
        return None;
    }
    Some(PathBuf::from(SKILLS_DIR).join(dir).join(SKILL_FILE))
}

/// Resolve a validated name to an openable canonical file path. Containment
/// is defense in depth (a symlinked skill dir must not escape the root).
pub(crate) fn resolve_agent_settings_file(root: &Path, name: &str) -> Result<PathBuf, String> {
    let Some(rel) = validate_name(name) else {
        return Err(format!("unknown agent settings file `{name}`"));
    };
    let path = root.join(&rel);
    let canonical =
        fs::canonicalize(&path).map_err(|error| format!("`{name}` is unavailable: {error}"))?;
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("agent config root unavailable: {error}"))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(format!("`{name}` escapes the agent config root"));
    }
    if !canonical.is_file() {
        return Err(format!("`{name}` is not a regular file"));
    }
    Ok(canonical)
}

#[derive(Deserialize)]
struct SeedManifest {
    #[serde(flatten)]
    entries: BTreeMap<String, SeedStamp>,
}

#[derive(Deserialize)]
struct SeedStamp {
    #[serde(rename = "sizeBytes")]
    size_bytes: u64,
    #[serde(rename = "mtimeMs")]
    mtime_ms: u64,
}

impl SeedManifest {
    fn load(root: &Path) -> SeedManifest {
        fs::read(root.join(SEED_MANIFEST))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(SeedManifest {
                entries: BTreeMap::new(),
            })
    }

    fn matches(&self, name: &str, size: u64, mtime_ms: u64) -> bool {
        // 1 ms tolerance: manifests written before the daemon truncated its
        // stamp (it used Math.round, up to +1 ms high) must still read as
        // built-in, or every untouched seed badges "edited" until a seed is
        // re-written. The size check is the load-bearing half.
        self.entries
            .get(name)
            .is_some_and(|stamp| stamp.size_bytes == size && stamp.mtime_ms.abs_diff(mtime_ms) <= 1)
    }
}

/// Millis since epoch, truncated (`as_millis`) to match the daemon's stamp,
/// which also truncates (`Math.trunc(mtimeMs)` in `recordSeedStamp`).
fn mtime_ms(metadata: &fs::Metadata) -> Option<u64> {
    let modified = metadata.modified().ok()?;
    let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    u64::try_from(duration.as_millis()).ok()
}

fn file_info(root: &Path, rel: &str, manifest: &SeedManifest) -> Option<AgentSettingsFileInfo> {
    let path = root.join(rel);
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let size_bytes = metadata.len();
    let modified_ms = mtime_ms(&metadata);
    let edited = !modified_ms
        .map(|mtime| manifest.matches(rel, size_bytes, mtime))
        .unwrap_or(false);
    Some(AgentSettingsFileInfo {
        name: rel.to_string(),
        display_path: path.display().to_string(),
        size_bytes,
        modified_ms,
        edited,
    })
}

/// List the delivered files in the fixed layout, sorted by name, bounded.
/// A missing root or skills dir lists nothing (empty page, never an error).
pub(crate) fn list_agent_settings_files(root: &Path) -> Vec<AgentSettingsFileInfo> {
    let manifest = SeedManifest::load(root);
    let mut names = Vec::new();
    if root.join(SYSTEM_MD).is_file() {
        names.push(SYSTEM_MD.to_string());
    }
    let skills = root.join(SKILLS_DIR);
    if let Ok(entries) = fs::read_dir(&skills) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join(SKILL_FILE).is_file() {
                names.push(
                    PathBuf::from(SKILLS_DIR)
                        .join(entry.file_name())
                        .join(SKILL_FILE)
                        .display()
                        .to_string(),
                );
            }
        }
    }
    names.sort();
    names.truncate(MAX_LIST);
    names
        .iter()
        .filter_map(|name| file_info(root, name, &manifest))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "clay-agent-settings-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn stamp(root: &Path, name: &str) {
        // Simulate the daemon: stat the file and write the manifest entry.
        let path = root.join(name);
        let metadata = fs::metadata(&path).expect("stat");
        let mtime_ms = u64::try_from(
            metadata
                .modified()
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap();
        let manifest_path = root.join(SEED_MANIFEST);
        let mut entries: BTreeMap<String, serde_json::Value> = fs::read(&manifest_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        entries.insert(
            name.to_string(),
            serde_json::json!({"sizeBytes": metadata.len(), "mtimeMs": mtime_ms}),
        );
        fs::write(manifest_path, serde_json::to_vec(&entries).unwrap()).expect("manifest");
    }

    #[test]
    fn listing_returns_exactly_the_delivered_layout() {
        let root = temp_root("layout");
        fs::create_dir_all(root.join("skills/graft")).unwrap();
        fs::create_dir_all(root.join("skills/wiki-searcher")).unwrap();
        fs::write(root.join("SYSTEM.md"), "system").unwrap();
        fs::write(root.join("skills/graft/SKILL.md"), "graft").unwrap();
        fs::write(root.join("skills/wiki-searcher/SKILL.md"), "wiki").unwrap();
        // Stray content must not appear in the fixed layout.
        fs::write(root.join("README.md"), "stray").unwrap();
        fs::write(root.join("skills/graft/notes.txt"), "stray").unwrap();

        let names: Vec<String> = list_agent_settings_files(&root)
            .into_iter()
            .map(|file| file.name)
            .collect();
        assert_eq!(
            names,
            vec![
                "SYSTEM.md".to_string(),
                "skills/graft/SKILL.md".to_string(),
                "skills/wiki-searcher/SKILL.md".to_string(),
            ]
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn provenance_badges_edited_files_and_built_in_seeds() {
        let root = temp_root("provenance");
        fs::create_dir_all(root.join("skills/graft")).unwrap();
        fs::write(root.join("SYSTEM.md"), "system").unwrap();
        fs::write(root.join("skills/graft/SKILL.md"), "graft").unwrap();
        // Only SYSTEM.md is stamped: the skill counts as edited.
        stamp(&root, "SYSTEM.md");
        let files = list_agent_settings_files(&root);
        let system = files.iter().find(|f| f.name == "SYSTEM.md").unwrap();
        let graft = files
            .iter()
            .find(|f| f.name.ends_with("graft/SKILL.md"))
            .unwrap();
        assert!(!system.edited);
        assert!(graft.edited);
        let _ = fs::remove_dir_all(&root);
    }

    /// Stamps `mtimeMs` verbatim (no stat): models a manifest written by an
    /// older daemon whose stamp disagreees with the truncating reader.
    fn stamp_mtime(root: &Path, name: &str, mtime_ms: u64) {
        let size = fs::metadata(root.join(name)).expect("stat").len();
        let manifest_path = root.join(SEED_MANIFEST);
        let mut entries: BTreeMap<String, serde_json::Value> = fs::read(&manifest_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        entries.insert(
            name.to_string(),
            serde_json::json!({"sizeBytes": size, "mtimeMs": mtime_ms}),
        );
        fs::write(manifest_path, serde_json::to_vec(&entries).unwrap()).expect("manifest");
    }

    #[test]
    fn provenance_tolerates_the_rounding_the_old_daemon_wrote() {
        // The daemon used `Math.round(mtimeMs)`, up to +1 ms above the
        // truncated read; those manifests must still badge built-in, and a
        // stamp 2 ms off must not (the tolerance is bounded).
        let root = temp_root("rounding");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("SYSTEM.md"), "system").unwrap();
        let metadata = fs::metadata(root.join("SYSTEM.md")).unwrap();
        let truncated = mtime_ms(&metadata).unwrap();

        stamp_mtime(&root, "SYSTEM.md", truncated + 1);
        assert!(
            !list_agent_settings_files(&root)[0].edited,
            "+1 ms still seed"
        );

        stamp_mtime(&root, "SYSTEM.md", truncated + 2);
        assert!(
            list_agent_settings_files(&root)[0].edited,
            "+2 ms is edited"
        );

        stamp_mtime(&root, "SYSTEM.md", truncated);
        assert!(!list_agent_settings_files(&root)[0].edited, "exact match");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn edited_seed_rewrites_keep_provenance_honest() {
        let root = temp_root("rewrite");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("SYSTEM.md"), "system").unwrap();
        stamp(&root, "SYSTEM.md");
        // User edit changes the bytes: size no longer matches the stamp.
        fs::write(root.join("SYSTEM.md"), "system, edited").unwrap();
        let files = list_agent_settings_files(&root);
        assert!(files[0].edited);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_accepts_layout_names_and_rejects_everything_else() {
        let root = temp_root("resolve");
        fs::create_dir_all(root.join("skills/graft")).unwrap();
        fs::write(root.join("SYSTEM.md"), "system").unwrap();
        fs::write(root.join("skills/graft/SKILL.md"), "graft").unwrap();

        assert!(resolve_agent_settings_file(&root, "SYSTEM.md").is_ok());
        assert!(resolve_agent_settings_file(&root, "skills/graft/SKILL.md").is_ok());

        for rejected in [
            "README.md",
            "../SYSTEM.md",
            "/etc/passwd",
            "skills/../SYSTEM.md",
            "skills/graft/other.md",
            "skills//SKILL.md",
            "skills/with/slash/SKILL.md",
            "",
            "SYSTEM.md/extra",
        ] {
            assert!(
                resolve_agent_settings_file(&root, rejected).is_err(),
                "`{rejected}` must be rejected"
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_rejects_symlink_escape() {
        let root = temp_root("symlink");
        let outside = temp_root("outside");
        fs::create_dir_all(root.join("skills/graft")).unwrap();
        fs::write(outside.join("secret.md"), "secret").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("skills/escape")).unwrap();
        fs::write(root.join("skills/escape/SKILL.md"), "stolen").ok();
        let result = resolve_agent_settings_file(&root, "skills/escape/SKILL.md");
        assert!(result.is_err(), "symlink escape must be rejected");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn per_agent_roots_stay_contained_and_fall_back_to_the_shipped_agent() {
        let base = temp_root("peragent");
        // The default (no agent named) is the shipped agent.
        assert_eq!(
            agent_config_root_for(Some(&base), None),
            Some(base.join("agents").join("coding-agent"))
        );
        // A configured name resolves to its own directory.
        assert_eq!(
            agent_config_root_for(Some(&base), Some("reviewer")),
            Some(base.join("agents").join("reviewer"))
        );
        // A name that is not a plain identifier can never address a path
        // outside `agents/` — it falls back to the shipped agent instead.
        for name in ["../etc", "a/b", "..", "", " ", "a b", &"x".repeat(65)] {
            assert_eq!(
                agent_config_root_for(Some(&base), Some(name)),
                Some(base.join("agents").join("coding-agent")),
                "`{name}` must not escape the layout"
            );
        }
        // Reads are contained to the resolved root: a valid name stays under
        // it, and the resolver never produces the parent directory.
        let root = agent_config_root_for(Some(&base), Some("reviewer")).expect("root");
        assert!(root.starts_with(base.join("agents")));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn per_agent_settings_reads_are_contained_to_that_agents_root() {
        let base = temp_root("peragent-read");
        let reviewer = base.join("agents").join("reviewer");
        fs::create_dir_all(reviewer.join("skills").join("review")).unwrap();
        fs::write(reviewer.join("SYSTEM.md"), "review prompt").unwrap();
        fs::write(
            reviewer.join("skills").join("review").join("SKILL.md"),
            "review skill",
        )
        .unwrap();
        let root = agent_config_root_for(Some(&base), Some("reviewer")).expect("root");

        let files = list_agent_settings_files(&root);
        let names: Vec<&str> = files.iter().map(|file| file.name.as_str()).collect();
        assert_eq!(names, vec!["SYSTEM.md", "skills/review/SKILL.md"]);
        // A name from another agent's layout still cannot traverse: the name
        // rule rejects it before any filesystem access.
        assert!(resolve_agent_settings_file(&root, "../../coding-agent/SYSTEM.md").is_err());
        assert!(resolve_agent_settings_file(&root, "SYSTEM.md").is_ok());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn agent_config_root_joins_the_per_agent_layout() {
        let base = temp_root("cfgroot");
        let root = agent_config_root_for(Some(&base), None).expect("root");
        assert_eq!(root, base.join("agents").join("coding-agent"));
        let _ = fs::remove_dir_all(&base);
    }
}
