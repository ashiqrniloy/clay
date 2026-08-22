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
    Resume {
        session_id: String,
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
            selected_index: 0,
            provider: None,
            auth: None,
            oauth_login_id: None,
            oauth_user_code: None,
            oauth_uri: None,
        }
    }

    pub(crate) fn set_query(&mut self, query: impl Into<String>) -> TransientMenuSession {
        self.query = query.into();
        self.selected_index = 0;
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
            AgentPickerKind::Session => {
                let session_id = id.strip_prefix("session:").unwrap_or(id).to_string();
                if secondary {
                    Ok(AgentPickerActivate::Delete { session_id })
                } else {
                    Ok(AgentPickerActivate::Resume { session_id })
                }
            }
            AgentPickerKind::ProviderSetup => {
                self.provider = Some(id.strip_prefix("provider:").unwrap_or(id).to_string());
                self.stage = Stage::AuthMethods;
                self.query.clear();
                self.selected_index = 0;
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
        if id != POLL_OAUTH_ID {
            return AgentPickerActivate::StayOpen;
        }
        match &self.oauth_login_id {
            Some(login_id) => AgentPickerActivate::PollOauth {
                login_id: login_id.clone(),
            },
            None => AgentPickerActivate::StayOpen,
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
            (AgentPickerKind::ProviderSetup, _) => "Configure provider",
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
                if code.is_empty() {
                    vec![item(POLL_OAUTH_ID, "Open authorization URL", detail)]
                } else {
                    vec![item(POLL_OAUTH_ID, &format!("Device code {code}"), detail)]
                }
            }
        };
        filter_items(items, &self.query, matches!(self.stage, Stage::Secret))
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
            AgentPickerKind::ProviderSetup => {
                self.inventory.providers.iter().map(provider_item).collect()
            }
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
        "agent.clientOpenSessionPicker" => Some(AgentPickerKind::Session),
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
                },
                AgentModelInfo {
                    provider: "openai".into(),
                    model: "gpt".into(),
                    display_name: "GPT".into(),
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
                session_id: "sess-1".into()
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
}
