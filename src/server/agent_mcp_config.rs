//! MCP stdio config sources (plan 117): the user's
//! `~/.clay/agents/coding-agent/mcp.json` and the repo-root
//! `.mcp.json` (Claude Code convention), merged into the daemon allow-list.
//!
//! Decision 2026-09-09-1341 (amending 1758): these two files connect
//! WITHOUT an approval gate — the user owns the servers they configure, and
//! bare command names may PATH-resolve (Claude Code parity). Package JS
//! still never supplies argv (two-trust-domain invariant), env names stay
//! explicit, and there is no shell interpolation anywhere. A malformed or
//! unreadable file warns and contributes nothing: startup never fails on
//! config.
//!
//! This is the only exception to approval-gated process authority; the
//! exception does not compose to new surfaces
//! (`.agents/skills/clay-execution/references/packages.md`).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

use super::agent::AgentMcpAllowListEntry;

/// Hard server bound, mirroring the daemon's `MAX_MCP_SERVERS` (Prism cap).
const MAX_MCP_SERVERS: usize = 32;
/// Literal argv ceiling (decision-log invariant: argv stays bounded).
const MAX_ARGS: usize = 64;

/// One raw server declaration — the union of both config formats. The user
/// format (`{"servers": {...}}`) carries optional `cwd`/`timeoutMs`; the
/// repo format (`{"mcpServers": {...}}`) ignores them (absent = defaults).
#[derive(Deserialize)]
struct RawMcpServer {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    /// Unknown keys are collected so they can be warned by name
    /// (tool-caps.json precedent), never fatal.
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

impl RawMcpServer {
    /// Validate + resolve into an allow-list entry; `None` = warned and
    /// dropped (fail-closed per entry, never a failed startup).
    fn into_entry(self, server_id: &str, source: &str) -> Option<AgentMcpAllowListEntry> {
        for key in self.extra.keys() {
            eprintln!(
                "[mcp-config] {source}: unknown key {key:?} for server {server_id:?} (ignored)"
            );
        }
        if server_id.is_empty() {
            eprintln!("[mcp-config] {source}: empty server id (dropped)");
            return None;
        }
        if self.command.is_empty() {
            eprintln!("[mcp-config] {source}: server {server_id:?} has an empty command (dropped)");
            return None;
        }
        if self.args.len() > MAX_ARGS {
            eprintln!(
                "[mcp-config] {source}: server {server_id:?} exceeds {MAX_ARGS} argv entries (dropped)"
            );
            return None;
        }
        let command = resolve_command(&self.command, server_id, source)?;
        Some(AgentMcpAllowListEntry {
            server_id: server_id.to_string(),
            command,
            args: self.args,
            env: self.env.into_iter().collect(),
            cwd: self.cwd,
            timeout_ms: self.timeout_ms,
        })
    }
}

/// Command resolution per the locked decision: absolute canonical paths pass
/// as-is; bare names may PATH-resolve; relative paths are rejected.
fn resolve_command(command: &str, server_id: &str, source: &str) -> Option<String> {
    let candidate = Path::new(command);
    if candidate.is_absolute() {
        return Some(command.to_string());
    }
    if command.contains('/') || command.contains('\\') {
        eprintln!(
            "[mcp-config] {source}: server {server_id:?} uses relative command path {command:?} (rejected; use an absolute path or a bare name)"
        );
        return None;
    }
    // Bare name: PATH lookup with canonical resolution.
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let hit = dir.join(command);
        if hit.is_file()
            && let Ok(canonical) = std::fs::canonicalize(&hit)
        {
            return Some(canonical.to_string_lossy().into_owned());
        }
    }
    eprintln!(
        "[mcp-config] {source}: server {server_id:?} command {command:?} not found on PATH (dropped)"
    );
    None
}

/// Parse one config file body. Missing/unreadable/malformed ⇒ warning +
/// empty (never a failed startup). Unknown top-level keys warned.
fn parse_file(text: &str, key: &str, source: &str) -> Vec<(String, RawMcpServer)> {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("[mcp-config] {source} is malformed ({error}); ignoring it");
            return Vec::new();
        }
    };
    let Some(servers) = value.get(key).and_then(|servers| servers.as_object()) else {
        if let Some(object) = value.as_object()
            && !object.is_empty()
        {
            eprintln!("[mcp-config] {source} has no {key:?} object; ignoring it");
        }
        return Vec::new();
    };
    servers
        .into_iter()
        .filter_map(|(id, raw)| {
            let server: RawMcpServer = match serde_json::from_value(raw.clone()) {
                Ok(server) => server,
                Err(error) => {
                    eprintln!(
                        "[mcp-config] {source}: server {id:?} is malformed ({error}); dropped"
                    );
                    return None;
                }
            };
            Some((id.clone(), server))
        })
        .collect()
}

/// Build the merged MCP allow-list: user `mcp.json` first, repo `.mcp.json`
/// second; on a server-id collision the user file wins. Bounded at 32
/// servers (over-cap entries dropped with a warning).
pub(crate) fn build_mcp_allow_list(
    config_root: Option<&Path>,
    workspace_root: Option<&Path>,
) -> Vec<AgentMcpAllowListEntry> {
    let mut sources: Vec<(String, &str)> = Vec::new();
    if let Some(root) = config_root {
        let file = root.join("agents").join("coding-agent").join("mcp.json");
        sources.push((file.to_string_lossy().into_owned(), "servers"));
    }
    if let Some(root) = workspace_root {
        sources.push((
            root.join(".mcp.json").to_string_lossy().into_owned(),
            "mcpServers",
        ));
    }
    let mut entries: Vec<AgentMcpAllowListEntry> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (file, key) in sources {
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                eprintln!("[mcp-config] {file} unreadable ({error}); ignoring it");
                continue;
            }
        };
        let source = format!("{file} ({key})");
        for (id, raw) in parse_file(&text, key, &file) {
            if seen.contains(&id) {
                eprintln!("[mcp-config] duplicate server id {id:?} (user file wins)");
                continue;
            }
            if entries.len() >= MAX_MCP_SERVERS {
                eprintln!("[mcp-config] server cap of {MAX_MCP_SERVERS} reached; dropping {id:?}");
                break;
            }
            if let Some(entry) = raw.into_entry(&id, &source) {
                seen.push(id);
                entries.push(entry);
            }
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_config(dir: &Path, body: &str) -> PathBuf {
        let root = dir.join("agents").join("coding-agent");
        fs::create_dir_all(&root).expect("mkdir");
        let file = root.join("mcp.json");
        fs::write(&file, body).expect("write");
        dir.to_path_buf()
    }

    fn write_repo(dir: &Path, body: &str) {
        fs::write(dir.join(".mcp.json"), body).expect("write");
    }

    use std::path::PathBuf;

    #[test]
    fn user_format_parses_with_absolute_passthrough() {
        let dir = std::env::temp_dir().join(format!("clay-mcp-user-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let root = write_config(
            &dir,
            r#"{"servers":{"graft":{"command":"/usr/local/bin/graft","args":["mcp"],"env":{"GRAFT_MODE":"pull"},"cwd":"/tmp"}}}"#,
        );
        let list = build_mcp_allow_list(Some(&root), None);
        assert_eq!(list.len(), 1);
        let entry = &list[0];
        assert_eq!(entry.server_id, "graft");
        assert_eq!(entry.command, "/usr/local/bin/graft");
        assert_eq!(entry.args, vec!["mcp"]);
        assert_eq!(entry.env, vec![("GRAFT_MODE".into(), "pull".into())]);
        assert_eq!(entry.cwd.as_deref(), Some("/tmp"));
        assert_eq!(entry.timeout_ms, None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn repo_format_parses_and_bare_names_path_resolve() {
        let dir = std::env::temp_dir().join(format!("clay-mcp-repo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        write_repo(
            &dir,
            r#"{"mcpServers":{"sh":{"command":"sh","args":["-c","echo hi"]}}}"#,
        );
        let list = build_mcp_allow_list(None, Some(&dir));
        assert_eq!(list.len(), 1, "sh exists on every CI host");
        let canonical = std::fs::canonicalize("/bin/sh").map(|p| p.to_string_lossy().into_owned());
        match canonical {
            Ok(path) => assert_eq!(list[0].command, path),
            Err(_) => assert!(
                list[0].command.starts_with('/'),
                "PATH-resolved to an absolute path"
            ),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn relative_commands_are_rejected_and_missing_bare_names_dropped() {
        let dir = std::env::temp_dir().join(format!("clay-mcp-rel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        write_repo(
            &dir,
            r#"{"mcpServers":{"rel":{"command":"./bin/serve"},"ghost":{"command":"definitely-not-on-path-8153"},"ok":{"command":"sh"}}}"#,
        );
        let list = build_mcp_allow_list(None, Some(&dir));
        assert_eq!(
            list.len(),
            1,
            "only the bare, PATH-resolvable name survives"
        );
        assert_eq!(list[0].server_id, "ok");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn user_file_wins_on_id_collision() {
        let dir = std::env::temp_dir().join(format!("clay-mcp-collide-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        let root = write_config(
            &dir,
            r#"{"servers":{"dup":{"command":"/usr/bin/env"},"solo":{"command":"/usr/bin/env"}}}"#,
        );
        write_repo(&dir, r#"{"mcpServers":{"dup":{"command":"sh"}}}"#);
        let list = build_mcp_allow_list(Some(&root), Some(&dir));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].server_id, "dup");
        assert_eq!(
            list[0].command, "/usr/bin/env",
            "user entry wins the collision"
        );
        assert_eq!(list[1].server_id, "solo");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_file_warns_and_contributes_nothing() {
        let dir = std::env::temp_dir().join(format!("clay-mcp-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        let root = write_config(&dir, r#"{"servers": broken"#);
        write_repo(&dir, r#"{"mcpServers":{"sh":{"command":"sh"}}}"#);
        let list = build_mcp_allow_list(Some(&root), Some(&dir));
        assert_eq!(list.len(), 1, "malformed user file never fails startup");
        assert_eq!(list[0].server_id, "sh");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn server_cap_bounds_the_merged_list() {
        let dir = std::env::temp_dir().join(format!("clay-mcp-cap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        let mut servers: Vec<String> = Vec::new();
        for index in 0..40 {
            servers.push(format!("\"s{index}\":{{\"command\":\"/usr/bin/env\"}}"));
        }
        let body = format!("{{\"servers\":{{{}}}}}", servers.join(","));
        let root = write_config(&dir, &body);
        let list = build_mcp_allow_list(Some(&root), None);
        assert_eq!(list.len(), MAX_MCP_SERVERS);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn absent_files_yield_an_empty_list_silently() {
        let dir = std::env::temp_dir().join(format!("clay-mcp-none-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        assert!(build_mcp_allow_list(Some(&dir), Some(&dir)).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn shipped_example_mcp_json_parses_under_the_real_schema() {
        // The canonical example a new user copies must stay valid against
        // the actual parser, not just the README prose (plan 117 config task).
        let config_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/config");
        let list = build_mcp_allow_list(Some(&config_root), None);
        assert!(list.is_empty(), "example ships no servers by default");
    }
}
