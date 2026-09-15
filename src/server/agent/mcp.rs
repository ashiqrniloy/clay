//! MCP allow-list and daemon inventory (plan 119 SC-3).
//!
//! Split out of `src/server/agent.rs` unchanged. The allow-list entry type is
//! shared with `agent_mcp_config.rs`, which builds it from the agent's
//! `mcp.json` plus the session workspace's `.mcp.json`; this module hands the
//! wire form to the daemon at `initialize`/`session.new`. The inventory half
//! caches the daemon's per-generation `environment.list` answer (commands,
//! extensions, skills, MCP connect outcomes) and turns its
//! `provider.list`/`model.list`/`agentProfile.list`/`session.list` replies
//! into picker inventory.

use super::{
    AgentError, AgentHost, AgentInventory, AgentMcpServerInfo, AgentModelInfo, AgentPickerAuth,
    AgentPickerInventory, AgentPickerItem, AgentPickerKind, AgentPickerProvider, AgentProfileInfo,
    AgentProviderInfo, AgentSessionInfo, AgentSkillInfo, AgentSlashCommand,
};
use serde_json::{Value, json};
use std::path::Path;

/// One allow-listed MCP stdio server. Built by the server from the two
/// config sources (plan 117: user `mcp.json` + repo `.mcp.json`, decision
/// 2026-09-09-1341) — never from package JavaScript at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMcpAllowListEntry {
    pub server_id: String,
    pub command: String,
    pub args: Vec<String>,
    /// Explicit env names and literal values only; never inherited wholesale.
    pub env: Vec<(String, String)>,
    pub cwd: Option<String>,
    /// Per-server connect timeout in ms (user config only; None = daemon
    /// default, plan 117 task "per-server fault isolation + timeoutMs").
    pub timeout_ms: Option<u64>,
}

impl AgentMcpAllowListEntry {
    /// Wire JSON for the daemon's `initialize` allow-list. Absent optionals
    /// are OMITTED, never `null`: the daemon contract is "absent = default",
    /// and `parseEntry` rejects a literal `null` cwd/timeoutMs (which failed
    /// the whole allow-list and with it every `session.new`).
    pub(super) fn to_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("serverId".to_string(), json!(self.server_id));
        map.insert("command".to_string(), json!(self.command));
        map.insert("args".to_string(), json!(self.args));
        map.insert(
            "env".to_string(),
            json!(
                self.env
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<std::collections::BTreeMap<_, _>>()
            ),
        );
        if let Some(cwd) = &self.cwd {
            map.insert("cwd".to_string(), json!(cwd));
        }
        if let Some(timeout_ms) = self.timeout_ms {
            map.insert("timeoutMs".to_string(), json!(timeout_ms));
        }
        Value::Object(map)
    }
}

/// One daemon-generation environment fetch (plan 109 R1/R3): registered
/// slash commands, active extensions, and catalog skills — all bounded
/// at parse time.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct DaemonEnvironment {
    pub(super) commands: Vec<AgentSlashCommand>,
    pub(super) extensions: Vec<String>,
    pub(super) skills: Vec<AgentSkillInfo>,
    pub(super) mcp_servers: Vec<AgentMcpServerInfo>,
}

/// Plan 109 R1: bounded parse of the daemon `environment.list` response —
/// completion names/descriptions, loaded extension names, and catalog
/// skills (name/description pairs) only.
pub(super) fn parse_environment(value: &Value) -> DaemonEnvironment {
    const MAX_COMMANDS: usize = 64;
    const MAX_EXTENSIONS: usize = 8;
    const MAX_SKILLS: usize = 64;
    let mut commands = Vec::new();
    if let Some(list) = value.get("commands").and_then(Value::as_array) {
        for command in list.iter().take(MAX_COMMANDS) {
            let Some(name) = command.get("name").and_then(Value::as_str) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            commands.push(AgentSlashCommand {
                name: name.chars().take(48).collect(),
                description: command
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .chars()
                    .take(96)
                    .collect(),
            });
        }
    }
    let mut extensions = Vec::new();
    if let Some(list) = value.get("extensions").and_then(Value::as_array) {
        for extension in list.iter().take(MAX_EXTENSIONS) {
            if let Some(name) = extension.as_str().filter(|name| !name.is_empty()) {
                extensions.push(name.chars().take(48).collect());
            }
        }
    }
    let mut skills = Vec::new();
    if let Some(list) = value.get("skills").and_then(Value::as_array) {
        for skill in list.iter().take(MAX_SKILLS) {
            let Some(name) = skill.get("name").and_then(Value::as_str) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            skills.push(AgentSkillInfo {
                name: name.chars().take(48).collect(),
                description: skill
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .chars()
                    .take(96)
                    .collect(),
            });
        }
    }
    // Plan 117: per-server MCP connect outcomes — id, connected, tool
    // count, hidden-because error. Bounded like the other environment keys.
    let mut mcp_servers = Vec::new();
    if let Some(list) = value.get("mcpServers").and_then(Value::as_array) {
        for server in list.iter().take(32) {
            let Some(server_id) = server.get("serverId").and_then(Value::as_str) else {
                continue;
            };
            if server_id.is_empty() {
                continue;
            }
            mcp_servers.push(AgentMcpServerInfo {
                server_id: server_id.chars().take(48).collect(),
                connected: server
                    .get("connected")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                tools: server.get("tools").and_then(Value::as_u64).unwrap_or(0) as u32,
                error: server
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .chars()
                    .take(96)
                    .collect(),
            });
        }
    }
    DaemonEnvironment {
        commands,
        extensions,
        skills,
        mcp_servers,
    }
}

pub(super) fn parse_picker_providers(value: &Value) -> Vec<AgentPickerProvider> {
    value
        .get("providers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let id = item.get("id").and_then(Value::as_str)?.to_string();
            let auth = item
                .get("auth")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|method| {
                    Some(AgentPickerAuth {
                        kind: method.get("kind").and_then(Value::as_str)?.to_string(),
                        name: method
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        credential_name: method
                            .get("credentialName")
                            .and_then(Value::as_str)
                            .unwrap_or("apiKey")
                            .to_string(),
                    })
                })
                .collect();
            Some(AgentPickerProvider {
                configured: item
                    .get("configured")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                id,
                auth,
            })
        })
        .collect()
}

pub(super) fn parse_models(value: &Value) -> Vec<AgentModelInfo> {
    value
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(AgentModelInfo {
                provider: item.get("provider").and_then(Value::as_str)?.to_string(),
                model: item.get("model").and_then(Value::as_str)?.to_string(),
                display_name: item
                    .get("displayName")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                context_window: item.get("contextWindow").and_then(Value::as_u64),
                thinking_levels: item
                    .get("thinkingLevels")
                    .and_then(Value::as_array)
                    .map(|levels| {
                        levels
                            .iter()
                            .filter_map(Value::as_str)
                            .map(ToString::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect()
}

pub(super) fn parse_profiles(value: &Value) -> Vec<AgentProfileInfo> {
    value
        .get("profiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(AgentProfileInfo {
                name: item.get("name").and_then(Value::as_str)?.to_string(),
                description: item
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect()
}

pub(super) fn parse_sessions(value: &Value) -> Vec<AgentSessionInfo> {
    value
        .get("sessions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(AgentSessionInfo {
                id: item.get("id").and_then(Value::as_str)?.to_string(),
                profile: item
                    .get("profile")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                updated_at: item
                    .get("updatedAt")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                label: item
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                // `session.list` carries no display stamp; the picker falls
                // back to the raw ISO value.
                updated_at_label: String::new(),
            })
        })
        .collect()
}

pub(super) fn picker_items(
    kind: AgentPickerKind,
    inventory: &AgentInventory,
) -> Vec<AgentPickerItem> {
    match kind {
        // Plan 109 I8: OM worker models are panel dropdowns (no list).
        AgentPickerKind::OmObservation | AgentPickerKind::OmReflection => vec![],
        AgentPickerKind::Provider | AgentPickerKind::ProviderSetup => inventory
            .providers
            .iter()
            .map(|provider| AgentPickerItem {
                id: provider.id.clone(),
                label: provider.id.clone(),
            })
            .collect(),
        AgentPickerKind::Model => {
            let configured: std::collections::HashSet<&str> = inventory
                .providers
                .iter()
                .filter(|provider| provider.configured)
                .map(|provider| provider.id.as_str())
                .collect();
            inventory
                .models
                .iter()
                .filter(|model| configured.contains(model.provider.as_str()))
                .map(|model| AgentPickerItem {
                    id: format!("{}/{}", model.provider, model.model),
                    label: if model.display_name.is_empty() {
                        model.model.clone()
                    } else {
                        model.display_name.clone()
                    },
                })
                .collect()
        }
        AgentPickerKind::Agent => inventory
            .profiles
            .iter()
            .map(|profile| AgentPickerItem {
                id: profile.name.clone(),
                label: if profile.description.is_empty() {
                    profile.name.clone()
                } else {
                    profile.description.clone()
                },
            })
            .collect(),
        AgentPickerKind::Session => inventory
            .sessions
            .iter()
            .map(|session| AgentPickerItem {
                id: session.id.clone(),
                label: if session.label.is_empty() {
                    session.profile.clone()
                } else {
                    session.label.clone()
                },
            })
            .collect(),
        // Search results are query-driven (FTS), never pre-listed in the
        // state snapshot.
        AgentPickerKind::SessionSearch => Vec::new(),
    }
}

impl AgentHost {
    pub(super) async fn agent_session_params(
        &self,
        agent: Option<&str>,
        workspace_root: Option<&str>,
    ) -> serde_json::Map<String, Value> {
        let mut params = serde_json::Map::new();
        let roots = self.inner.agent_roots.lock().await.clone();
        let Some(roots) = roots else {
            return params;
        };
        if let Some(agent) = agent
            && self.agent_config_root(agent).await.is_some()
        {
            params.insert("agent".into(), json!(agent));
        }
        // The allow-list rides every session, agent or not: the daemon's
        // spawn-time list is built from the launch root and may never bind a
        // session in another folder (plan 119 SC-6).
        let allow_list: Vec<Value> = crate::server::agent_mcp_config::build_mcp_allow_list(
            roots.config_root.as_deref(),
            workspace_root.map(Path::new),
            agent,
        )
        .iter()
        .map(AgentMcpAllowListEntry::to_json)
        .collect();
        params.insert("mcpAllowList".into(), json!(allow_list));
        params
    }

    /// Resolve one agent type to its per-agent config root (plan 118 task 35):
    /// contained to `<data root>/agents/`, and only when the directory
    /// resolves — a name the launcher would not list is not an agent.
    pub(crate) async fn picker_inventory(&self) -> AgentPickerInventory {
        if self.inner.config.inert {
            return AgentPickerInventory::default();
        }
        self.inventory_rich().await.unwrap_or_default()
    }

    pub(super) async fn inventory(&self) -> Result<AgentInventory, AgentError> {
        let rich = self.inventory_rich().await?;
        let (provider, model) = {
            let book = self.inner.book.lock().await;
            (book.provider.clone(), book.model.clone())
        };
        Ok(AgentInventory {
            providers: rich
                .providers
                .iter()
                .map(|provider| AgentProviderInfo {
                    id: provider.id.clone(),
                    configured: provider.configured,
                })
                .collect(),
            models: rich.models,
            profiles: rich.profiles,
            sessions: rich.sessions,
            provider,
            model,
        })
    }

    pub(super) async fn inventory_rich(&self) -> Result<AgentPickerInventory, AgentError> {
        let providers = self.rpc("provider.list", json!({})).await?;
        let models = self.rpc("model.list", json!({})).await?;
        let profiles = self.rpc("agentProfile.list", json!({})).await?;
        let sessions = self.rpc("session.list", json!({})).await?;
        let parsed_models = parse_models(&models);
        {
            // Cache declared thinking levels per provider/model (plan 109
            // I4) so snapshots can carry effortLevels without a daemon call.
            let mut book = self.inner.book.lock().await;
            for model in &parsed_models {
                book.model_levels.insert(
                    format!("{}/{}", model.provider, model.model),
                    model.thinking_levels.clone(),
                );
            }
        }
        Ok(AgentPickerInventory {
            providers: parse_picker_providers(&providers),
            models: parsed_models,
            profiles: parse_profiles(&profiles),
            sessions: parse_sessions(&sessions),
        })
    }

    pub(super) async fn environment(&self, agent_type: Option<&str>) -> DaemonEnvironment {
        if self.inner.config.inert {
            return DaemonEnvironment::default();
        }
        let key = agent_type.unwrap_or("").to_string();
        {
            let cached = self.inner.environment.lock().await;
            if let Some(environment) = cached.get(&key) {
                return environment.clone();
            }
        }
        // Plan 118 task 35: the daemon resolves the session's agent root from
        // the session id, so the inventories it answers with are that
        // session's (its registered commands/extensions are host-wide).
        let params = if key.is_empty() {
            json!({})
        } else {
            json!({ "agent": key })
        };
        let fetched = self
            .rpc("environment.list", params)
            .await
            .map(|value| parse_environment(&value))
            .unwrap_or_default();
        self.inner
            .environment
            .lock()
            .await
            .insert(key, fetched.clone());
        fetched
    }
}
