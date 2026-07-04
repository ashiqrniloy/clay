//! Generic transient menu session state.
//!
//! `TransientMenuSession` is a Clay-owned typed state model for command
//! palettes, completion pickers, file search, Git pickers, and other
//! bottom-pane transient overlays. It stores query text, a bounded item list,
//! selection state, status text, and inert activation actions. It does not
//! contain callbacks, native widget handles, raw CSS, raw ops, executable
//! package code, or hidden authority fields.
//!
//! Rendering and command execution are separate concerns: the session is
//! projected onto existing shell transient-overlay/component primitives, and
//! activation normalizes into the server-owned `CommandExecutionRequest` path.

#![allow(dead_code)]

use serde_json::Value;

use crate::perf::budgets::{
    TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS, TRANSIENT_MENU_MAX_DETAIL_CHARS,
    TRANSIENT_MENU_MAX_ITEMS, TRANSIENT_MENU_MAX_LABEL_CHARS, TRANSIENT_MENU_MAX_QUERY_CHARS,
};
use crate::protocol::{
    BehaviorVersion, CompletionItem, CompletionReplacementRange, CompletionRequestId,
    CompletionResultSet, CompletionStatus, DocumentId, DocumentVersion,
};

const MAX_ITEMS: usize = TRANSIENT_MENU_MAX_ITEMS;
const MAX_QUERY_CHARS: usize = TRANSIENT_MENU_MAX_QUERY_CHARS;
const MAX_LABEL_CHARS: usize = TRANSIENT_MENU_MAX_LABEL_CHARS;
const MAX_DETAIL_CHARS: usize = TRANSIENT_MENU_MAX_DETAIL_CHARS;
const MAX_ACCESSIBILITY_LABEL_CHARS: usize = TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TransientMenuSessionId(pub(crate) u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransientMenuSession {
    session_id: TransientMenuSessionId,
    prompt: String,
    query: String,
    items: Vec<TransientMenuItem>,
    selected_index: usize,
    status: TransientMenuStatus,
    focus_policy: TransientMenuFocusPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransientMenuItem {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) detail: Option<String>,
    pub(crate) accessibility_label: String,
    pub(crate) provenance: TransientMenuItemProvenance,
    pub(crate) action: TransientMenuAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransientMenuAction {
    pub(crate) command_id: String,
    pub(crate) arguments: Value,
    pub(crate) completion_accept: Option<CompletionMenuAcceptAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionMenuAcceptAction {
    pub(crate) request_id: CompletionRequestId,
    pub(crate) document_id: DocumentId,
    pub(crate) document_version: DocumentVersion,
    pub(crate) behavior_version: BehaviorVersion,
    pub(crate) replacement_range: CompletionReplacementRange,
    pub(crate) insert_text: String,
    pub(crate) commit_characters: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TransientMenuItemProvenance {
    BuiltIn,
    Package { name: String, version: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransientMenuFocusPolicy {
    Modal,
    Modeless,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TransientMenuStatus {
    Active,
    Empty { message: String },
    Cancelled,
}

impl TransientMenuSession {
    pub(crate) fn new(session_id: TransientMenuSessionId, prompt: impl Into<String>) -> Self {
        Self {
            session_id,
            prompt: truncate(&prompt.into(), MAX_LABEL_CHARS),
            query: String::new(),
            items: Vec::new(),
            selected_index: 0,
            status: TransientMenuStatus::Empty {
                message: "No results".to_string(),
            },
            focus_policy: TransientMenuFocusPolicy::Modal,
        }
    }

    pub(crate) fn session_id(&self) -> TransientMenuSessionId {
        self.session_id
    }

    pub(crate) fn prompt(&self) -> &str {
        &self.prompt
    }

    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    pub(crate) fn items(&self) -> &[TransientMenuItem] {
        &self.items
    }

    pub(crate) fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub(crate) fn status(&self) -> &TransientMenuStatus {
        &self.status
    }

    pub(crate) fn focus_policy(&self) -> TransientMenuFocusPolicy {
        self.focus_policy
    }

    pub(crate) fn with_focus_policy(mut self, policy: TransientMenuFocusPolicy) -> Self {
        self.focus_policy = policy;
        self
    }

    pub(crate) fn with_items(mut self, items: Vec<TransientMenuItem>) -> Self {
        self.items = items.into_iter().take(MAX_ITEMS).collect();
        self.selected_index = 0;
        if self.items.is_empty() {
            self.status = TransientMenuStatus::Empty {
                message: "No results".to_string(),
            };
        } else {
            self.status = TransientMenuStatus::Active;
        }
        self
    }

    pub(crate) fn with_empty_status(mut self, message: impl Into<String>) -> Self {
        if self.items.is_empty() {
            self.status = TransientMenuStatus::Empty {
                message: truncate(&message.into(), MAX_DETAIL_CHARS),
            };
        }
        self
    }

    pub(crate) fn update_query(&mut self, query: impl Into<String>) {
        self.query = truncate(&query.into(), MAX_QUERY_CHARS);
        self.selected_index = 0;
        self.update_status_after_filter();
    }

    pub(crate) fn select_next(&mut self) {
        if self.items.is_empty() {
            self.selected_index = 0;
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.items.len();
    }

    pub(crate) fn select_previous(&mut self) {
        if self.items.is_empty() {
            self.selected_index = 0;
            return;
        }
        self.selected_index = if self.selected_index == 0 {
            self.items.len() - 1
        } else {
            self.selected_index - 1
        };
    }

    pub(crate) fn selected_item(&self) -> Option<&TransientMenuItem> {
        self.items.get(self.selected_index)
    }

    pub(crate) fn activate_selected(&self) -> Option<&TransientMenuAction> {
        if !self.is_active() {
            return None;
        }
        self.selected_item().map(|item| &item.action)
    }

    pub(crate) fn cancel(&mut self) {
        self.status = TransientMenuStatus::Cancelled;
    }

    pub(crate) fn is_active(&self) -> bool {
        !matches!(self.status, TransientMenuStatus::Cancelled)
    }

    fn update_status_after_filter(&mut self) {
        if self.items.is_empty() && matches!(self.status, TransientMenuStatus::Active) {
            self.status = TransientMenuStatus::Empty {
                message: "No results".to_string(),
            };
        } else if !self.items.is_empty() && matches!(self.status, TransientMenuStatus::Empty { .. })
        {
            self.status = TransientMenuStatus::Active;
        }
    }
}

impl TransientMenuItem {
    pub(crate) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        action: TransientMenuAction,
    ) -> Self {
        let label = truncate(&label.into(), MAX_LABEL_CHARS);
        Self {
            id: id.into(),
            label: label.clone(),
            detail: None,
            accessibility_label: label,
            provenance: TransientMenuItemProvenance::BuiltIn,
            action,
        }
    }

    pub(crate) fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(truncate(&detail.into(), MAX_DETAIL_CHARS));
        self
    }

    pub(crate) fn with_accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.accessibility_label = truncate(&label.into(), MAX_ACCESSIBILITY_LABEL_CHARS);
        self
    }

    pub(crate) fn with_package_provenance(
        mut self,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        self.provenance = TransientMenuItemProvenance::Package {
            name: name.into(),
            version: version.into(),
        };
        self
    }

    pub(crate) fn with_provenance(mut self, provenance: TransientMenuItemProvenance) -> Self {
        self.provenance = provenance;
        self
    }
}

impl TransientMenuAction {
    pub(crate) fn new(command_id: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            arguments: Value::Null,
            completion_accept: None,
        }
    }

    pub(crate) fn completion_accept(action: CompletionMenuAcceptAction) -> Self {
        Self {
            command_id: "completion.accept".to_string(),
            arguments: Value::Null,
            completion_accept: Some(action),
        }
    }

    pub(crate) fn with_arguments(mut self, arguments: Value) -> Self {
        self.arguments = arguments;
        self
    }
}

pub(crate) fn completion_result_to_menu_session(
    result: &CompletionResultSet,
) -> TransientMenuSession {
    let items = result
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| completion_item_to_menu_item(result, index, item))
        .collect();
    let session =
        TransientMenuSession::new(TransientMenuSessionId(result.request_id), "Completion")
            .with_focus_policy(TransientMenuFocusPolicy::Modeless)
            .with_items(items);
    if !result.items.is_empty() {
        return session;
    }
    session.with_empty_status(match result.status {
        CompletionStatus::Ok | CompletionStatus::Empty => "No completions",
        CompletionStatus::Timeout => "Completion provider timed out",
        CompletionStatus::ProviderError => "Completion provider error",
    })
}

fn completion_item_to_menu_item(
    result: &CompletionResultSet,
    index: usize,
    item: &CompletionItem,
) -> TransientMenuItem {
    let action = CompletionMenuAcceptAction {
        request_id: result.request_id,
        document_id: result.document_id,
        document_version: result.document_version,
        behavior_version: result.behavior_version,
        replacement_range: result.replacement_range,
        insert_text: item.insert_text.clone(),
        commit_characters: item.commit_characters.clone(),
    };
    let detail = if item.detail.is_empty() {
        format!(
            "{} {}",
            item.provenance.package_name, item.provenance.package_version
        )
    } else {
        format!(
            "{} · {} {}",
            item.detail, item.provenance.package_name, item.provenance.package_version
        )
    };
    TransientMenuItem::new(
        format!("completion.{index}"),
        item.label.clone(),
        TransientMenuAction::completion_accept(action),
    )
    .with_detail(detail)
    .with_accessibility_label(format!("Completion {}", item.label))
    .with_package_provenance(
        item.provenance.package_name.clone(),
        item.provenance.package_version.clone(),
    )
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(id: &str, label: &str) -> TransientMenuItem {
        TransientMenuItem::new(id, label, TransientMenuAction::new("clay.builtIn.test"))
    }

    #[test]
    fn session_stores_prompt_and_starts_empty() {
        let session = TransientMenuSession::new(TransientMenuSessionId(1), "Control Center");
        assert_eq!(session.prompt(), "Control Center");
        assert_eq!(session.query(), "");
        assert!(session.items().is_empty());
        assert_eq!(session.selected_index(), 0);
        assert!(matches!(
            session.status(),
            TransientMenuStatus::Empty { .. }
        ));
        assert_eq!(session.focus_policy(), TransientMenuFocusPolicy::Modal);
    }

    #[test]
    fn with_items_bounds_count_and_resets_selection() {
        let items: Vec<_> = (0..300)
            .map(|i| sample_item(&format!("item-{i}"), &format!("Item {i}")))
            .collect();
        let session =
            TransientMenuSession::new(TransientMenuSessionId(2), "Picker").with_items(items);
        assert_eq!(session.items().len(), MAX_ITEMS);
        assert_eq!(session.selected_index(), 0);
        assert_eq!(*session.status(), TransientMenuStatus::Active);
    }

    #[test]
    fn query_update_truncates_and_resets_selection() {
        let session = TransientMenuSession::new(TransientMenuSessionId(3), "Search")
            .with_items(vec![sample_item("a", "Alpha"), sample_item("b", "Beta")]);
        let mut session = session;
        session.select_next();
        assert_eq!(session.selected_index(), 1);

        let long_query = "x".repeat(MAX_QUERY_CHARS + 10);
        session.update_query(&long_query);
        assert_eq!(session.query().len(), MAX_QUERY_CHARS);
        assert_eq!(session.selected_index(), 0);
    }

    #[test]
    fn selection_wraps_at_boundaries() {
        let mut session =
            TransientMenuSession::new(TransientMenuSessionId(4), "List").with_items(vec![
                sample_item("a", "Alpha"),
                sample_item("b", "Beta"),
                sample_item("c", "Gamma"),
            ]);

        session.select_previous();
        assert_eq!(session.selected_index(), 2);
        session.select_next();
        assert_eq!(session.selected_index(), 0);
        session.select_previous();
        assert_eq!(session.selected_index(), 2);
    }

    #[test]
    fn empty_session_selection_is_no_op() {
        let mut session = TransientMenuSession::new(TransientMenuSessionId(5), "Empty");
        session.select_next();
        session.select_previous();
        assert_eq!(session.selected_index(), 0);
        assert!(session.selected_item().is_none());
        assert!(session.activate_selected().is_none());
    }

    #[test]
    fn activate_selected_returns_action() {
        let session =
            TransientMenuSession::new(TransientMenuSessionId(6), "Commands").with_items(vec![
                sample_item("a", "Alpha"),
                TransientMenuItem::new(
                    "b",
                    "Beta",
                    TransientMenuAction::new("clay.builtIn.run")
                        .with_arguments(serde_json::json!({"foo": "bar"})),
                ),
            ]);

        let action = session.activate_selected().expect("first item has action");
        assert_eq!(action.command_id, "clay.builtIn.test");

        let mut session = session;
        session.select_next();
        let action = session.activate_selected().expect("second item has action");
        assert_eq!(action.command_id, "clay.builtIn.run");
        assert_eq!(action.arguments, serde_json::json!({"foo": "bar"}));
    }

    #[test]
    fn cancel_marks_session_inactive() {
        let mut session = TransientMenuSession::new(TransientMenuSessionId(7), "Commands")
            .with_items(vec![sample_item("a", "Alpha")]);
        assert!(session.is_active());
        session.cancel();
        assert!(!session.is_active());
        assert_eq!(session.status(), &TransientMenuStatus::Cancelled);
    }

    #[test]
    fn item_labels_and_details_are_truncated() {
        let long_label = "a".repeat(MAX_LABEL_CHARS + 5);
        let long_detail = "b".repeat(MAX_DETAIL_CHARS + 5);
        let item = sample_item("id", &long_label).with_detail(&long_detail);
        assert_eq!(item.label.len(), MAX_LABEL_CHARS);
        assert_eq!(item.detail.as_ref().unwrap().len(), MAX_DETAIL_CHARS);
    }

    #[test]
    fn package_provenance_is_stored() {
        let item = sample_item("pkg.cmd", "Package Command")
            .with_package_provenance("@clay/markdown", "1.0.0");
        assert_eq!(
            item.provenance,
            TransientMenuItemProvenance::Package {
                name: "@clay/markdown".to_string(),
                version: "1.0.0".to_string(),
            }
        );
    }

    #[test]
    fn item_uses_accessibility_label_when_set() {
        let item =
            sample_item("id", "Display").with_accessibility_label("Accessible display label");
        assert_eq!(item.accessibility_label, "Accessible display label");
    }

    #[test]
    fn focus_policy_can_be_modeless() {
        let session = TransientMenuSession::new(TransientMenuSessionId(8), "HUD")
            .with_focus_policy(TransientMenuFocusPolicy::Modeless);
        assert_eq!(session.focus_policy(), TransientMenuFocusPolicy::Modeless);
    }

    #[test]
    fn cancelled_session_rejects_activation() {
        let mut session = TransientMenuSession::new(TransientMenuSessionId(9), "Commands")
            .with_items(vec![sample_item("a", "Alpha")]);
        session.cancel();

        assert!(!session.is_active());
        assert!(session.activate_selected().is_none());
    }

    #[test]
    fn item_detail_and_accessibility_budgets_are_enforced() {
        let label = "x".repeat(MAX_LABEL_CHARS + 10);
        let detail = "y".repeat(MAX_DETAIL_CHARS + 10);
        let accessibility = "z".repeat(MAX_ACCESSIBILITY_LABEL_CHARS + 10);

        let item = TransientMenuItem::new("a", &label, TransientMenuAction::new("clay.alpha"))
            .with_detail(&detail)
            .with_accessibility_label(&accessibility);

        assert_eq!(item.label.len(), MAX_LABEL_CHARS);
        assert_eq!(item.detail.as_ref().unwrap().len(), MAX_DETAIL_CHARS);
        assert_eq!(
            item.accessibility_label.len(),
            MAX_ACCESSIBILITY_LABEL_CHARS
        );
    }

    #[test]
    fn item_action_is_inert_command_intent_only() {
        let action = TransientMenuAction::new("clay.alpha")
            .with_arguments(serde_json::json!({ "preview": true }));
        let item = TransientMenuItem::new("a", "Alpha", action);

        assert_eq!(item.action.command_id, "clay.alpha");
        assert_eq!(
            item.action.arguments,
            serde_json::json!({ "preview": true })
        );
        assert!(item.action.completion_accept.is_none());
        // No callbacks, native handles, raw ops, or executable code on the item.
    }

    #[test]
    fn completion_error_status_projects_to_empty_menu_status() {
        let result = CompletionResultSet {
            request_id: 13,
            client_id: 1,
            document_id: 7,
            document_version: 8,
            behavior_version: 9,
            provider_generation: 1,
            replacement_range: CompletionReplacementRange::new(3, 5),
            status: CompletionStatus::ProviderError,
            items: Vec::new(),
            provenance: crate::protocol::CompletionProvenance::builtin_core(),
        };

        let session = completion_result_to_menu_session(&result);

        assert_eq!(
            session.status(),
            &TransientMenuStatus::Empty {
                message: "Completion provider error".to_string()
            }
        );
    }

    #[test]
    fn completion_result_projects_to_transient_menu_session() {
        let result = CompletionResultSet {
            request_id: 12,
            client_id: 1,
            document_id: 7,
            document_version: 8,
            behavior_version: 9,
            provider_generation: 1,
            replacement_range: CompletionReplacementRange::new(3, 5),
            status: CompletionStatus::Ok,
            items: vec![CompletionItem {
                label: "println".to_string(),
                insert_text: "println!".to_string(),
                detail: "macro".to_string(),
                commit_characters: ";".to_string(),
                provenance: crate::protocol::CompletionProvenance::builtin_core(),
            }],
            provenance: crate::protocol::CompletionProvenance::builtin_core(),
        };

        let session = completion_result_to_menu_session(&result);

        assert_eq!(session.prompt(), "Completion");
        assert_eq!(session.focus_policy(), TransientMenuFocusPolicy::Modeless);
        assert_eq!(session.items()[0].label, "println");
        assert_eq!(session.items()[0].accessibility_label, "Completion println");
        let accept = session.items()[0]
            .action
            .completion_accept
            .as_ref()
            .expect("completion item has inert accept payload");
        assert_eq!(
            accept.replacement_range,
            CompletionReplacementRange::new(3, 5)
        );
        assert_eq!(accept.insert_text, "println!");
        assert_eq!(accept.commit_characters, ";");
    }
}
