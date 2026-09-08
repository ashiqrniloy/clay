//! Command Centre session kind for agent/provider/model/setup/session pickers.
//!
//! Reuses the shared fuzzy matcher and centered origin. Secret steps keep the
//! real query server-side and project bullets so snapshots/a11y never see it.

use crate::{
    protocol::AgentPickerKind,
    server::{
        agent::{AgentPickerAuth, AgentPickerInventory, AgentPickerProvider},
        control_center::score_menu_item,
    },
    shell::transient_menu::{
        TransientMenuAction, TransientMenuItem, TransientMenuOrigin, TransientMenuSession,
        TransientMenuSessionId,
    },
};

const CONFIGURE_ID: &str = "configure";
const STORE_SECRET_ID: &str = "store_secret";
const STORE_URL_ID: &str = "store_url";
const POLL_OAUTH_ID: &str = "poll_oauth";
/// Plan 116: manual browser-open and clipboard-copy fallbacks in the OAuth
/// device-code stage, so the authorization URL never dead-ends even when the
/// auto-open attempt fails or the user prefers another browser.
const OPEN_OAUTH_URL_ID: &str = "open_oauth_url";
const COPY_OAUTH_URL_ID: &str = "copy_oauth_url";
const SEARCH_HINT_ID: &str = "search_hint";
/// Bounded result page for the session-search picker (plan 108 task 11).
pub(crate) const AGENT_SESSION_SEARCH_LIMIT: u32 = 50;

/// One `session.search` hit projected into the workspace-scoped session
/// search picker (plan 108 task 11, decision 2201). Carries only transcript
/// metadata — redacted snippet, never raw tool output.
#[derive(Debug, Clone)]
pub struct AgentSearchHit {
    pub session_id: String,
    pub leaf_id: Option<String>,
    pub updated_at: String,
    pub label: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    List,
    AuthMethods,
    Secret,
    Url,
    Oauth,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentPicker {
    session_id: TransientMenuSessionId,
    kind: AgentPickerKind,
    stage: Stage,
    inventory: AgentPickerInventory,
    package_profiles: Vec<(String, String)>,
    query: String,
    /// FTS hits for [`AgentPickerKind::SessionSearch`], refreshed by the
    /// connection on every query change; `filter_items` never re-scores
    /// them (the index already matched).
    search_hits: Vec<AgentSearchHit>,
    selected_index: usize,
    provider: Option<String>,
    auth: Option<AgentPickerAuth>,
    oauth_login_id: Option<String>,
    oauth_user_code: Option<String>,
    oauth_uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentPickerActivate {
    StayOpen,
    PutSecret {
        provider: String,
        name: String,
        secret: String,
    },
    StartOauth {
        provider: String,
    },
    PollOauth {
        login_id: String,
    },
    /// Plan 116: open the OAuth authorization URL in the OS default browser
    /// (manual fallback after the automatic attempt).
    OpenOauthUrl {
        uri: String,
    },
    /// Plan 116: copy the OAuth authorization URL to the OS clipboard so the
    /// user can paste it into a browser of their choice.
    CopyOauthUrl {
        uri: String,
    },
    Resume {
        session_id: String,
        /// Open at this tree entry (search-result selection, plan 108
        /// task 11). `None` resumes at the live leaf.
        entry_id: Option<String>,
    },
    Delete {
        session_id: String,
    },
    Select {
        kind: AgentPickerKind,
        id: String,
    },
}

impl AgentPicker {
    pub(crate) fn open(
        session_id: u64,
        kind: AgentPickerKind,
        inventory: AgentPickerInventory,
        package_profiles: Vec<(String, String)>,
    ) -> Self {
        Self {
            session_id: TransientMenuSessionId(session_id),
            kind,
            stage: Stage::List,
            inventory,
            package_profiles,
            query: String::new(),
            search_hits: Vec::new(),
            selected_index: 0,
            provider: None,
            auth: None,
            oauth_login_id: None,
            oauth_user_code: None,
            oauth_uri: None,
        }
    }

    pub(crate) fn set_query(&mut self, query: impl Into<String>) -> TransientMenuSession {
        let incoming = query.into();
        let merged = if self.stage == Stage::Secret {
            merge_secret_query(&self.query, &incoming)
        } else {
            incoming
        };
        // Only a genuinely changed filter resets the selection: the webview
        // flushes the same draft on Enter before activating, and that flush
        // must not clobber the arrow-selected item.
        if merged != self.query {
            self.selected_index = 0;
        }
        self.query = merged;
        self.session()
    }

    pub(crate) fn backspace(&mut self) -> TransientMenuSession {
        self.query.pop();
        self.session()
    }

    pub(crate) fn move_selection(&mut self, delta: i64) -> TransientMenuSession {
        let mut session = self.session();
        let len = session.items().len();
        if len > 0 {
            let steps = delta.rem_euclid(len as i64) as usize;
            for _ in 0..steps {
                session.select_next();
            }
            self.selected_index = session.selected_index();
        }
        session
    }

    pub(crate) fn replace_inventory(&mut self, inventory: AgentPickerInventory) {
        self.inventory = inventory;
        self.selected_index = 0;
    }

    /// Installs a fresh FTS result page for the session-search picker.
    pub(crate) fn set_search_hits(&mut self, hits: Vec<AgentSearchHit>) -> TransientMenuSession {
        self.search_hits = hits;
        self.selected_index = 0;
        self.session()
    }

    pub(crate) fn kind(&self) -> AgentPickerKind {
        self.kind
    }

    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    pub(crate) fn enter_oauth(&mut self, login_id: String, user_code: String, uri: String) {
        self.stage = Stage::Oauth;
        self.oauth_login_id = Some(login_id);
        self.oauth_user_code = Some(user_code);
        self.oauth_uri = Some(uri);
        self.query.clear();
        self.selected_index = 0;
    }

    pub(crate) fn session(&self) -> TransientMenuSession {
        let items = self.visible_items();
        let secret = matches!(self.stage, Stage::Secret);
        let query = if secret {
            "•".repeat(self.query.chars().count())
        } else {
            self.query.clone()
        };
        TransientMenuSession::new(self.session_id, self.prompt())
            .with_items(items)
            .with_selected_index(self.selected_index)
            .with_query(query)
            .with_origin(TransientMenuOrigin::Centered)
    }

    pub(crate) fn activate(&mut self, secondary: bool) -> Result<AgentPickerActivate, String> {
        let session = self.session();
        let action = session
            .activate_selected()
            .ok_or_else(|| "no agent picker item selected".to_string())?;
        let id = action.command_id.clone();
        match self.stage {
            Stage::List => self.activate_list(&id, secondary),
            Stage::AuthMethods => self.activate_auth(&id),
            Stage::Secret => Ok(self.activate_secret(&id)),
            Stage::Url => Ok(self.activate_url(&id)),
            Stage::Oauth => Ok(self.activate_oauth(&id)),
        }
    }

    fn activate_list(&mut self, id: &str, secondary: bool) -> Result<AgentPickerActivate, String> {
        if id == CONFIGURE_ID {
            self.kind = AgentPickerKind::ProviderSetup;
            self.stage = Stage::List;
            self.query.clear();
            self.selected_index = 0;
            return Ok(AgentPickerActivate::StayOpen);
        }
        match self.kind {
            AgentPickerKind::Provider | AgentPickerKind::Model | AgentPickerKind::Agent => {
                self.query.clear();
                Ok(AgentPickerActivate::Select {
                    kind: self.kind,
                    id: id.to_string(),
                })
            }
            // Plan 109 I8: OM worker models are panel dropdowns, not
            // transient-picker flows — never activated here.
            AgentPickerKind::OmObservation | AgentPickerKind::OmReflection => {
                Ok(AgentPickerActivate::StayOpen)
            }
            AgentPickerKind::Session => {
                let session_id = id.strip_prefix("session:").unwrap_or(id).to_string();
                if secondary {
                    Ok(AgentPickerActivate::Delete { session_id })
                } else {
                    Ok(AgentPickerActivate::Resume {
                        session_id,
                        entry_id: None,
                    })
                }
            }
            AgentPickerKind::SessionSearch => {
                if id == SEARCH_HINT_ID || secondary {
                    return Ok(AgentPickerActivate::StayOpen);
                }
                let rest = id.strip_prefix("search:").unwrap_or(id);
                let (session_id, entry_id) = match rest.split_once('|') {
                    Some((session_id, entry_id)) => {
                        (session_id.to_string(), Some(entry_id.to_string()))
                    }
                    None => (rest.to_string(), None),
                };
                Ok(AgentPickerActivate::Resume {
                    session_id,
                    entry_id,
                })
            }
            AgentPickerKind::ProviderSetup => {
                let provider = id.strip_prefix("provider:").unwrap_or(id).to_string();
                self.provider = Some(provider.clone());
                self.stage = Stage::AuthMethods;
                self.query.clear();
                self.selected_index = 0;
                let auth = self.provider_auth(&provider);
                if auth.len() == 1 {
                    return self.activate_auth(&format!("auth:{}", auth[0].kind));
                }
                Ok(AgentPickerActivate::StayOpen)
            }
        }
    }

    fn activate_auth(&mut self, id: &str) -> Result<AgentPickerActivate, String> {
        let provider = self
            .provider
            .clone()
            .ok_or_else(|| "no provider selected".to_string())?;
        let kind = id.strip_prefix("auth:").unwrap_or(id);
        let auth = self
            .provider_auth(&provider)
            .into_iter()
            .find(|method| method.kind == kind)
            .ok_or_else(|| format!("unknown auth method `{kind}`"))?;
        self.auth = Some(auth.clone());
        self.query.clear();
        self.selected_index = 0;
        match auth.kind.as_str() {
            "oauth" => Ok(AgentPickerActivate::StartOauth { provider }),
            "url" => {
                self.stage = Stage::Url;
                Ok(AgentPickerActivate::StayOpen)
            }
            _ => {
                self.stage = Stage::Secret;
                Ok(AgentPickerActivate::StayOpen)
            }
        }
    }

    fn activate_secret(&self, id: &str) -> AgentPickerActivate {
        if id != STORE_SECRET_ID || self.query.trim().is_empty() {
            return AgentPickerActivate::StayOpen;
        }
        let provider = self.provider.clone().unwrap_or_default();
        let name = self
            .auth
            .as_ref()
            .map(|auth| auth.credential_name.clone())
            .unwrap_or_else(|| "apiKey".to_string());
        AgentPickerActivate::PutSecret {
            provider,
            name,
            secret: self.query.clone(),
        }
    }

    fn activate_url(&self, id: &str) -> AgentPickerActivate {
        if id != STORE_URL_ID || self.query.trim().is_empty() {
            return AgentPickerActivate::StayOpen;
        }
        AgentPickerActivate::PutSecret {
            provider: self.provider.clone().unwrap_or_default(),
            name: "baseUrl".to_string(),
            secret: self.query.clone(),
        }
    }

    fn activate_oauth(&self, id: &str) -> AgentPickerActivate {
        match id {
            OPEN_OAUTH_URL_ID => match &self.oauth_uri {
                Some(uri) => AgentPickerActivate::OpenOauthUrl { uri: uri.clone() },
                None => AgentPickerActivate::StayOpen,
            },
            COPY_OAUTH_URL_ID => match &self.oauth_uri {
                Some(uri) => AgentPickerActivate::CopyOauthUrl { uri: uri.clone() },
                None => AgentPickerActivate::StayOpen,
            },
            _ if id == POLL_OAUTH_ID => match &self.oauth_login_id {
                Some(login_id) => AgentPickerActivate::PollOauth {
                    login_id: login_id.clone(),
                },
                None => AgentPickerActivate::StayOpen,
            },
            _ => AgentPickerActivate::StayOpen,
        }
    }

    fn prompt(&self) -> &'static str {
        match (self.kind, self.stage) {
            (_, Stage::Secret) => "API key (hidden)",
            (_, Stage::Url) => "API base URL",
            (_, Stage::Oauth) => "Authorize provider",
            (_, Stage::AuthMethods) => "Choose sign-in method",
            (AgentPickerKind::Provider, _) => "Providers",
            (AgentPickerKind::Model, _) => "Models",
            (AgentPickerKind::Agent, _) => "Agents",
            (AgentPickerKind::Session, _) => "Sessions",
            (AgentPickerKind::SessionSearch, _) => "Search sessions (workspace)",
            (AgentPickerKind::ProviderSetup, _) => "Configure provider",
            (AgentPickerKind::OmObservation, _) => "Observation worker models",
            (AgentPickerKind::OmReflection, _) => "Reflection worker models",
        }
    }

    fn visible_items(&self) -> Vec<TransientMenuItem> {
        let items = match self.stage {
            Stage::List => self.list_items(),
            Stage::AuthMethods => self.auth_items(),
            Stage::Secret => vec![item(
                STORE_SECRET_ID,
                "Store API key",
                "Value is hidden. Enter stores it.",
            )],
            Stage::Url => vec![item(
                STORE_URL_ID,
                "Save base URL",
                "OpenAI-compatible endpoint",
            )],
            Stage::Oauth => {
                let code = self.oauth_user_code.as_deref().unwrap_or("");
                let uri = self.oauth_uri.as_deref().unwrap_or("");
                let detail = if uri.is_empty() {
                    "Enter to check authorization"
                } else {
                    uri
                };
                // Plan 116: the primary row carries the device code and
                // checks authorization on Enter/click; the two action rows
                // are manual fallbacks when the automatic browser open fails
                // or the user prefers a different browser. The URL is the
                // detail of every row, so it stays visible and copyable.
                let mut items = vec![if code.is_empty() {
                    item(POLL_OAUTH_ID, "Open authorization URL", detail)
                } else {
                    item(POLL_OAUTH_ID, &format!("Device code {code}"), detail)
                }];
                if !uri.is_empty() {
                    items.push(
                        item(OPEN_OAUTH_URL_ID, "Open in browser", detail)
                            .with_accessibility_label(
                                "Open the authorization URL in the default browser",
                            ),
                    );
                    items.push(
                        item(COPY_OAUTH_URL_ID, "Copy URL", detail).with_accessibility_label(
                            "Copy the authorization URL to the clipboard",
                        ),
                    );
                }
                items
            }
        };
        filter_items(
            items,
            &self.query,
            !matches!(self.stage, Stage::List)
                || matches!(self.kind, AgentPickerKind::SessionSearch),
        )
    }

    fn list_items(&self) -> Vec<TransientMenuItem> {
        match self.kind {
            AgentPickerKind::Provider => {
                let mut items: Vec<_> =
                    self.inventory.providers.iter().map(provider_item).collect();
                items.push(item(
                    CONFIGURE_ID,
                    "Configure provider…",
                    "API key, OAuth, or base URL",
                ));
                items
            }
            AgentPickerKind::Model => configured_models(&self.inventory)
                .into_iter()
                .map(|model| {
                    item(
                        &format!("model:{}/{}", model.provider, model.model),
                        if model.display_name.is_empty() {
                            &model.model
                        } else {
                            &model.display_name
                        },
                        &model.provider,
                    )
                })
                .collect(),
            AgentPickerKind::Agent => self.agent_items(),
            AgentPickerKind::Session => self
                .inventory
                .sessions
                .iter()
                .map(|session| {
                    item(
                        &format!("session:{}", session.id),
                        &session.profile,
                        &session.updated_at,
                    )
                })
                .collect(),
            AgentPickerKind::SessionSearch => {
                if self.search_hits.is_empty() {
                    let hint = if self.query.is_empty() {
                        "Type to search this workspace's sessions"
                    } else {
                        "No matching sessions in this workspace"
                    };
                    return vec![item(SEARCH_HINT_ID, hint, "Enter does nothing")];
                }
                self.search_hits
                    .iter()
                    .map(|hit| {
                        // leafId rides in the item id: activating resumes the
                        // session opened at that tree entry (no tree mutation).
                        let mut id = format!("search:{}", hit.session_id);
                        if let Some(leaf) = &hit.leaf_id {
                            id.push('|');
                            id.push_str(leaf);
                        }
                        let detail = if hit.snippet.is_empty() {
                            hit.updated_at.clone()
                        } else {
                            format!("{} · {}", hit.snippet, hit.updated_at)
                        };
                        item(&id, &hit.label, &detail)
                    })
                    .collect()
            }
            AgentPickerKind::ProviderSetup => {
                self.inventory.providers.iter().map(provider_item).collect()
            }
            // Plan 109 I8: OM worker models are panel dropdowns (no
            // transient-picker list).
            AgentPickerKind::OmObservation | AgentPickerKind::OmReflection => vec![],
        }
    }

    fn auth_items(&self) -> Vec<TransientMenuItem> {
        let Some(provider) = &self.provider else {
            return Vec::new();
        };
        self.provider_auth(provider)
            .into_iter()
            .map(|method| {
                let label = if method.name.is_empty() {
                    auth_label(&method.kind)
                } else {
                    method.name
                };
                item(&format!("auth:{}", method.kind), &label, &method.kind)
            })
            .collect()
    }

    fn agent_items(&self) -> Vec<TransientMenuItem> {
        let mut items: Vec<TransientMenuItem> = self
            .package_profiles
            .iter()
            .map(|(id, name)| item(&format!("agent:{id}"), name, "package profile"))
            .collect();
        for profile in &self.inventory.profiles {
            let id = format!("agent:{}", profile.name);
            if items.iter().any(|item| item.id == id) {
                continue;
            }
            items.push(item(
                &id,
                &profile.name,
                if profile.description.is_empty() {
                    "registered"
                } else {
                    &profile.description
                },
            ));
        }
        items
    }

    fn provider_auth(&self, provider: &str) -> Vec<AgentPickerAuth> {
        self.inventory
            .providers
            .iter()
            .find(|item| item.id == provider)
            .map(|item| item.auth.clone())
            .unwrap_or_default()
    }
}

fn configured_models(inventory: &AgentPickerInventory) -> Vec<crate::protocol::AgentModelInfo> {
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
        .cloned()
        .collect()
}

fn provider_item(provider: &AgentPickerProvider) -> TransientMenuItem {
    item(
        &format!("provider:{}", provider.id),
        &provider.id,
        if provider.configured {
            "configured"
        } else {
            "not configured"
        },
    )
}

fn auth_label(kind: &str) -> String {
    match kind {
        "oauth" => "OAuth".to_string(),
        "url" => "API base URL".to_string(),
        _ => "API key".to_string(),
    }
}

fn item(id: &str, label: &str, detail: &str) -> TransientMenuItem {
    TransientMenuItem::new(id, label, TransientMenuAction::new(id))
        .with_detail(detail)
        .with_accessibility_label(label)
}

/// Secret-stage query is bullet-masked on the wire. The client sends that
/// masked value plus newly typed characters; recover the real secret by
/// treating a leading run of `•` as a length prefix into the current value.
fn merge_secret_query(current: &str, incoming: &str) -> String {
    let bullets = incoming.chars().take_while(|ch| *ch == '•').count();
    let suffix: String = incoming.chars().skip(bullets).collect();
    let prefix: String = current.chars().take(bullets).collect();
    let mut next = prefix;
    next.push_str(&suffix);
    next
}

fn filter_items(items: Vec<TransientMenuItem>, query: &str, skip: bool) -> Vec<TransientMenuItem> {
    if skip || query.is_empty() {
        return items;
    }
    let mut scored: Vec<(i32, TransientMenuItem)> = items
        .into_iter()
        .filter_map(|item| score_menu_item(&item, query).map(|score| (score, item)))
        .collect();
    scored.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.label.cmp(&right.1.label))
            .then_with(|| left.1.id.cmp(&right.1.id))
    });
    scored.into_iter().map(|(_, item)| item).collect()
}

pub(crate) fn picker_kind_for_command(command_id: &str) -> Option<AgentPickerKind> {
    match command_id {
        "agent.clientOpenProviderPicker" | "chat.openProviderPicker" => {
            Some(AgentPickerKind::Provider)
        }
        "agent.clientOpenModelPicker" | "chat.openModelPicker" => Some(AgentPickerKind::Model),
        "agent.clientOpenAgentPicker" | "chat.openAgentPicker" => Some(AgentPickerKind::Agent),
        "agent.clientOpenProviderSetup" => Some(AgentPickerKind::ProviderSetup),
        "agent.clientOpenSessionPicker" | "coding-agent.resume" => Some(AgentPickerKind::Session),
        "agent.clientOpenSessionSearchPicker" => Some(AgentPickerKind::SessionSearch),
        _ => None,
    }
}

pub(crate) fn package_profile_commands(
    catalogue: &crate::packages::commands::CommandCatalogue,
) -> Vec<(String, String)> {
    catalogue
        .commands()
        .iter()
        .filter(|command| command.command_id.ends_with(".profile"))
        .map(|command| (command.command_id.clone(), command.display_name.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AgentModelInfo, AgentProfileInfo, AgentSessionInfo};

    fn inventory() -> AgentPickerInventory {
        AgentPickerInventory {
            providers: vec![
                AgentPickerProvider {
                    id: "anthropic".into(),
                    configured: true,
                    auth: vec![AgentPickerAuth {
                        kind: "api_key".into(),
                        name: "API key".into(),
                        credential_name: "apiKey".into(),
                    }],
                },
                AgentPickerProvider {
                    id: "openai".into(),
                    configured: false,
                    auth: vec![
                        AgentPickerAuth {
                            kind: "api_key".into(),
                            name: "API key".into(),
                            credential_name: "apiKey".into(),
                        },
                        AgentPickerAuth {
                            kind: "url".into(),
                            name: "Base URL".into(),
                            credential_name: "baseUrl".into(),
                        },
                    ],
                },
            ],
            models: vec![
                AgentModelInfo {
                    provider: "anthropic".into(),
                    model: "claude".into(),
                    display_name: "Claude".into(),
                    context_window: None,
                    thinking_levels: Vec::new(),
                },
                AgentModelInfo {
                    provider: "openai".into(),
                    model: "gpt".into(),
                    display_name: "GPT".into(),
                    context_window: None,
                    thinking_levels: Vec::new(),
                },
            ],
            profiles: vec![AgentProfileInfo {
                name: "Chat".into(),
                description: "General assistant. No tools.".into(),
            }],
            sessions: vec![AgentSessionInfo {
                id: "sess-1".into(),
                profile: "Chat".into(),
                updated_at: "now".into(),
                label: String::new(),
            }],
        }
    }

    #[test]
    fn unconfigured_provider_cannot_be_model_source() {
        let picker = AgentPicker::open(1, AgentPickerKind::Model, inventory(), Vec::new());
        let labels: Vec<_> = picker
            .session()
            .items()
            .iter()
            .map(|item| item.label.clone())
            .collect();
        assert_eq!(labels, vec!["Claude"]);
        assert!(!labels.iter().any(|label| label == "GPT"));
    }

    #[test]
    fn secret_is_not_in_snapshot_query_or_labels() {
        let mut picker =
            AgentPicker::open(1, AgentPickerKind::ProviderSetup, inventory(), Vec::new());
        assert_eq!(
            picker.activate(false).unwrap(),
            AgentPickerActivate::StayOpen
        );
        assert_eq!(
            picker.activate(false).unwrap(),
            AgentPickerActivate::StayOpen
        );
        picker.set_query("sk-secret-value");
        let session = picker.session();
        assert!(session.query().chars().all(|ch| ch == '•'));
        assert_eq!(
            session.query().chars().count(),
            "sk-secret-value".chars().count()
        );
        assert!(!format!("{session:?}").contains("sk-secret-value"));
        match picker.activate(false).unwrap() {
            AgentPickerActivate::PutSecret { secret, .. } => {
                assert_eq!(secret, "sk-secret-value")
            }
            other => panic!("expected put, got {other:?}"),
        }
    }

    #[test]
    fn secret_query_from_masked_snapshot_appends_typed_suffix() {
        assert_eq!(merge_secret_query("sk", "••x"), "skx");
        assert_eq!(merge_secret_query("sk", "••"), "sk");
        assert_eq!(merge_secret_query("sk", "new"), "new");
        assert_eq!(merge_secret_query("", "a"), "a");
    }

    #[test]
    fn unchanged_query_keeps_arrow_selection_flush_before_activate() {
        // The webview flush pattern re-sends the same draft via menuQuery
        // right before menuActivate; that flush must not clobber the
        // arrow-selected model. A genuinely changed query still resets.
        let mut inventory = inventory();
        inventory.providers[1].configured = true;
        let mut picker = AgentPicker::open(1, AgentPickerKind::Model, inventory, Vec::new());
        let session = picker.move_selection(1);
        assert_eq!(session.selected_index(), 1);
        let session = picker.set_query("");
        assert_eq!(
            session.selected_index(),
            1,
            "flush of the unchanged draft must keep the selected model"
        );
        let session = picker.set_query("claude");
        assert_eq!(session.selected_index(), 0);
    }

    #[test]
    fn leftover_filter_query_does_not_hide_auth_methods() {
        let mut picker =
            AgentPicker::open(1, AgentPickerKind::ProviderSetup, inventory(), Vec::new());
        picker.set_query("openai");
        assert_eq!(
            picker.activate(false).unwrap(),
            AgentPickerActivate::StayOpen
        );
        picker.set_query("openai");
        let labels: Vec<_> = picker
            .session()
            .items()
            .iter()
            .map(|item| item.label.clone())
            .collect();
        assert!(
            labels.iter().any(|label| label == "API key"),
            "auth methods must stay visible when a leftover list filter arrives: {labels:?}"
        );
        assert!(
            labels.iter().any(|label| label == "Base URL"),
            "url method must stay visible: {labels:?}"
        );
    }

    #[test]
    fn single_auth_provider_opens_secret_stage() {
        let mut inventory = inventory();
        inventory.providers.push(AgentPickerProvider {
            id: "opencode-go".into(),
            configured: false,
            auth: vec![AgentPickerAuth {
                kind: "api_key".into(),
                name: "API key".into(),
                credential_name: "apiKey".into(),
            }],
        });
        let mut picker =
            AgentPicker::open(1, AgentPickerKind::ProviderSetup, inventory, Vec::new());
        picker.set_query("opencode-go");
        assert_eq!(
            picker.activate(false).unwrap(),
            AgentPickerActivate::StayOpen
        );
        assert_eq!(picker.session().prompt(), "API key (hidden)");
        assert_eq!(picker.session().items()[0].id, STORE_SECRET_ID);
    }

    #[test]
    fn agent_picker_omits_coding_agent_until_registered() {
        let picker = AgentPicker::open(
            1,
            AgentPickerKind::Agent,
            inventory(),
            vec![("chat.profile".into(), "Chat".into())],
        );
        let labels: Vec<_> = picker
            .session()
            .items()
            .iter()
            .map(|item| item.label.clone())
            .collect();
        assert!(labels.iter().any(|label| label == "Chat"));
        assert!(!labels.iter().any(|label| label.contains("Coding")));
    }

    #[test]
    fn oauth_labels_distinguish_device_code_from_redirect() {
        let mut picker =
            AgentPicker::open(1, AgentPickerKind::ProviderSetup, inventory(), Vec::new());
        picker.enter_oauth(
            "login-1".into(),
            "ABCD-EFGH".into(),
            "https://example.test/device".into(),
        );
        let session = picker.session();
        assert_eq!(session.items()[0].label, "Device code ABCD-EFGH");
        assert_eq!(
            session.items()[0].detail.as_deref(),
            Some("https://example.test/device")
        );

        picker.enter_oauth(
            "login-2".into(),
            String::new(),
            "https://example.test/authorize".into(),
        );
        let session = picker.session();
        assert_eq!(session.items()[0].label, "Open authorization URL");
        assert_eq!(
            session.items()[0].detail.as_deref(),
            Some("https://example.test/authorize")
        );
        assert!(!format!("{session:?}").contains("pending"));
    }

    #[test]
    fn oauth_stage_offers_browser_open_and_copy_url_actions() {
        let mut picker =
            AgentPicker::open(1, AgentPickerKind::ProviderSetup, inventory(), Vec::new());
        picker.enter_oauth(
            "login-1".into(),
            "ABCD-EFGH".into(),
            "https://example.test/device".into(),
        );
        let session = picker.session();
        let ids: Vec<_> = session
            .items()
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![POLL_OAUTH_ID, OPEN_OAUTH_URL_ID, COPY_OAUTH_URL_ID]
        );
        // Every row keeps the URL visible as its detail text.
        for row in session.items() {
            assert_eq!(row.detail.as_deref(), Some("https://example.test/device"));
        }
        // Selecting the action rows maps to the server-side open/copy intents.
        picker.move_selection(1);
        assert_eq!(
            picker.activate(false).unwrap(),
            AgentPickerActivate::OpenOauthUrl {
                uri: "https://example.test/device".into()
            }
        );
        picker.move_selection(1);
        assert_eq!(
            picker.activate(false).unwrap(),
            AgentPickerActivate::CopyOauthUrl {
                uri: "https://example.test/device".into()
            }
        );
        // The primary row still polls for completion (wraps back to row 0).
        picker.move_selection(1);
        assert_eq!(
            picker.activate(false).unwrap(),
            AgentPickerActivate::PollOauth {
                login_id: "login-1".into()
            }
        );
    }

    #[test]
    fn oauth_stage_without_url_keeps_only_the_poll_row() {
        let mut picker =
            AgentPicker::open(1, AgentPickerKind::ProviderSetup, inventory(), Vec::new());
        picker.enter_oauth("login-1".into(), "ABCD-EFGH".into(), String::new());
        let session = picker.session();
        assert_eq!(session.items().len(), 1);
        assert_eq!(session.items()[0].id, POLL_OAUTH_ID);
        // No URL means no open/copy rows, and their activations stay open.
        assert_eq!(
            picker.activate(false).unwrap(),
            AgentPickerActivate::PollOauth {
                login_id: "login-1".into()
            }
        );
    }

    #[test]
    fn provider_list_includes_configure_action() {
        let picker = AgentPicker::open(1, AgentPickerKind::Provider, inventory(), Vec::new());
        assert!(
            picker
                .session()
                .items()
                .iter()
                .any(|item| item.id == CONFIGURE_ID)
        );
    }

    #[test]
    fn session_primary_resumes_secondary_deletes() {
        let mut picker = AgentPicker::open(1, AgentPickerKind::Session, inventory(), Vec::new());
        assert_eq!(
            picker.activate(false).unwrap(),
            AgentPickerActivate::Resume {
                session_id: "sess-1".into(),
                entry_id: None,
            }
        );
        let mut picker = AgentPicker::open(1, AgentPickerKind::Session, inventory(), Vec::new());
        assert_eq!(
            picker.activate(true).unwrap(),
            AgentPickerActivate::Delete {
                session_id: "sess-1".into()
            }
        );
    }

    #[test]
    fn session_search_picker_skips_local_filter_and_resumes_at_entry() {
        // Plan 108 task 11 (decision 2201): the FTS index already matched;
        // the picker must not re-score hits and activation resumes the
        // session opened at the matching tree entry.
        let mut picker =
            AgentPicker::open(1, AgentPickerKind::SessionSearch, inventory(), Vec::new());
        picker.set_query("llanowar");
        picker.set_search_hits(vec![AgentSearchHit {
            session_id: "s1".into(),
            leaf_id: Some("entry_9".into()),
            updated_at: "2026-09-03".into(),
            label: "coding".into(),
            snippet: "attack with llanowar elves".into(),
        }]);
        // "zzz" would locally filter out every item — hits survive.
        picker.set_query("zzz");
        let session = picker.session();
        assert_eq!(session.items().len(), 1, "FTS hits skip the local filter");
        let item = &session.items()[0];
        assert_eq!(item.id, "search:s1|entry_9");
        assert_eq!(item.label, "coding");
        assert!(item.detail.as_deref().unwrap_or("").contains("llanowar"));

        // Activating the hit resumes at the entry; secondary (delete) is a
        // no-op stay-open for search results.
        let mut picker =
            AgentPicker::open(1, AgentPickerKind::SessionSearch, inventory(), Vec::new());
        picker.set_query("q");
        picker.set_search_hits(vec![AgentSearchHit {
            session_id: "s1".into(),
            leaf_id: None,
            updated_at: "".into(),
            label: "chat".into(),
            snippet: String::new(),
        }]);
        let activated = picker.activate(false).unwrap();
        assert_eq!(
            activated,
            AgentPickerActivate::Resume {
                session_id: "s1".into(),
                entry_id: None,
            }
        );
    }

    #[test]
    fn resume_command_ids_map_to_the_session_picker() {
        // Plan 109 I9: the package's /resume palette entry rides the same
        // Command Centre session-picker flow as the core client command.
        assert_eq!(
            picker_kind_for_command("agent.clientOpenSessionPicker"),
            Some(AgentPickerKind::Session)
        );
        assert_eq!(
            picker_kind_for_command("coding-agent.resume"),
            Some(AgentPickerKind::Session)
        );
        assert_eq!(picker_kind_for_command("coding-agent.new"), None);
    }

    #[test]
    fn session_search_empty_query_shows_hint() {
        let picker = AgentPicker::open(1, AgentPickerKind::SessionSearch, inventory(), Vec::new());
        let session = picker.session();
        assert_eq!(session.items().len(), 1);
        assert!(session.items()[0].label.contains("Type to search"));
    }
}
