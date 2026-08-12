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
    LanguageIntelligenceFeature, LanguageIntelligencePayload, LanguageIntelligenceResult,
    LanguageIntelligenceStatus, TextLocation,
};

const MAX_ITEMS: usize = TRANSIENT_MENU_MAX_ITEMS;
const MAX_QUERY_CHARS: usize = TRANSIENT_MENU_MAX_QUERY_CHARS;
const MAX_LABEL_CHARS: usize = TRANSIENT_MENU_MAX_LABEL_CHARS;
const MAX_DETAIL_CHARS: usize = TRANSIENT_MENU_MAX_DETAIL_CHARS;
const MAX_ACCESSIBILITY_LABEL_CHARS: usize = TRANSIENT_MENU_MAX_ACCESSIBILITY_LABEL_CHARS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransientMenuSessionId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
#[doc(hidden)]
pub struct TransientMenuSession {
    session_id: TransientMenuSessionId,
    prompt: String,
    query: String,
    items: Vec<TransientMenuItem>,
    selected_index: usize,
    status: TransientMenuStatus,
    focus_policy: TransientMenuFocusPolicy,
    /// Phase 20.5: surface origin (command palette, context menu, menu bar).
    origin: TransientMenuOrigin,
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
    pub(crate) text_format: crate::protocol::CompletionItemTextFormat,
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

/// Phase 20.5: surface origin for transient menu sessions.
/// Determines overlay anchor and focus policy defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransientMenuOrigin {
    /// Bottom-anchored command palette / completion picker (default).
    CommandPalette,
    /// Pointer-anchored context menu.
    ContextMenu,
    /// Main-area-anchored menu bar dropdown.
    MenuBar,
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
            origin: TransientMenuOrigin::CommandPalette,
        }
    }

    pub fn session_id(&self) -> TransientMenuSessionId {
        self.session_id
    }

    pub fn prompt(&self) -> &str {
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

    /// Phase 20.5: surface origin for this session.
    pub(crate) fn origin(&self) -> TransientMenuOrigin {
        self.origin
    }

    /// Phase 24.1: hydrate an inert protocol snapshot into a display session.
    /// Items carry no activation action — server-owned sessions activate by
    /// opaque session id on the server, never from client-side data — and no
    /// provenance (that stays server-side; the wire carries detail text only).
    pub(crate) fn from_snapshot_data(
        snapshot: &crate::protocol::TransientMenuSnapshotData,
    ) -> Self {
        let items = snapshot
            .items
            .iter()
            .map(|item| {
                TransientMenuItem::new(
                    item.id.clone(),
                    item.label.clone(),
                    TransientMenuAction::new(item.id.clone()),
                )
                .with_detail(item.detail.clone().unwrap_or_default())
                .with_accessibility_label(item.accessibility_label.clone())
            })
            .collect();
        let status = match &snapshot.status {
            crate::protocol::TransientMenuStatusData::Active => None,
            crate::protocol::TransientMenuStatusData::Empty { message } => Some(message.as_str()),
        };
        let focus_policy = match snapshot.focus_policy {
            crate::protocol::TransientMenuFocusPolicyData::Modal => TransientMenuFocusPolicy::Modal,
            crate::protocol::TransientMenuFocusPolicyData::Modeless => {
                TransientMenuFocusPolicy::Modeless
            }
        };
        let origin = match snapshot.origin {
            crate::protocol::TransientMenuOriginData::CommandPalette => {
                TransientMenuOrigin::CommandPalette
            }
            crate::protocol::TransientMenuOriginData::ContextMenu => {
                TransientMenuOrigin::ContextMenu
            }
            crate::protocol::TransientMenuOriginData::MenuBar => TransientMenuOrigin::MenuBar,
        };
        let mut session = Self::new(
            TransientMenuSessionId(snapshot.session_id),
            snapshot.prompt.clone(),
        )
        .with_items(items)
        .with_query(&snapshot.query)
        .with_focus_policy(focus_policy)
        .with_origin(origin);
        if let Some(message) = status {
            session = session.with_empty_status(message);
        }
        session = session.with_selected_index(snapshot.selected_index as usize);
        session
    }

    pub(crate) fn with_focus_policy(mut self, policy: TransientMenuFocusPolicy) -> Self {
        self.focus_policy = policy;
        self
    }

    /// Phase 20.5: set the surface origin (command palette, context menu, menu bar).
    pub(crate) fn with_origin(mut self, origin: TransientMenuOrigin) -> Self {
        self.origin = origin;
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

    /// Phase 24.1: restore a persisted selection after `with_items`, clamped
    /// to the item list (empty list → 0). Server-owned sessions keep their
    /// selection across snapshot pushes via this builder.
    pub(crate) fn with_selected_index(mut self, index: usize) -> Self {
        self.selected_index = index.min(self.items.len().saturating_sub(1));
        self
    }

    /// Phase 24.1: restore the filter query on a produced session, truncated
    /// to the shared query budget. Server-owned sessions carry their query in
    /// the session so snapshots render what the user typed.
    pub(crate) fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = truncate(&query.into(), MAX_QUERY_CHARS);
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
        text_format: item.text_format,
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

/// Project Markdown to bounded plain text for the bottom transient UI.
/// Strips common Markdown markers and HTML tags; never executes markup.
pub(crate) fn markdown_to_plain_text(markdown: &str) -> String {
    let without_tags = strip_angle_bracket_tags(markdown);
    let mut plain = String::with_capacity(without_tags.len());
    let mut chars = without_tags.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '#' | '*' | '_' | '`' => {
                // Skip runs of Markdown emphasis/heading markers.
                while chars.peek().copied() == Some(ch) {
                    chars.next();
                }
            }
            '[' => {
                // Keep link label text; drop `](url)` targets.
                let mut label = String::new();
                while let Some(next) = chars.peek().copied() {
                    chars.next();
                    if next == ']' {
                        break;
                    }
                    label.push(next);
                }
                if chars.peek() == Some(&'(') {
                    chars.next();
                    for next in chars.by_ref() {
                        if next == ')' {
                            break;
                        }
                    }
                }
                plain.push_str(&label);
            }
            _ => plain.push(ch),
        }
    }
    truncate(
        plain.trim(),
        MAX_LABEL_CHARS.saturating_mul(4).max(MAX_DETAIL_CHARS),
    )
}

fn strip_angle_bracket_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Project a language-intelligence result onto the existing bottom transient UI.
/// Hover/signature use a modeless single-item (or empty status) presentation.
/// Multiple definitions/actions become selectable menu items.
pub(crate) fn language_intelligence_result_to_menu_session(
    result: &LanguageIntelligenceResult,
) -> TransientMenuSession {
    let prompt = match result.feature {
        LanguageIntelligenceFeature::Hover => "Hover",
        LanguageIntelligenceFeature::GoToDefinition => "Definitions",
        LanguageIntelligenceFeature::CodeAction => "Code Actions",
        LanguageIntelligenceFeature::SignatureHelp => "Signature Help",
    };
    let empty_status = match result.status {
        LanguageIntelligenceStatus::Ok | LanguageIntelligenceStatus::Empty => {
            match result.feature {
                LanguageIntelligenceFeature::Hover => "No hover information",
                LanguageIntelligenceFeature::GoToDefinition => "No definitions",
                LanguageIntelligenceFeature::CodeAction => "No code actions",
                LanguageIntelligenceFeature::SignatureHelp => "No signature help",
            }
        }
        LanguageIntelligenceStatus::Timeout => "Language intelligence timed out",
        LanguageIntelligenceStatus::ProviderError => "Language intelligence provider error",
    };

    let items = match &result.payload {
        LanguageIntelligencePayload::Hover(hover) => {
            let plain = markdown_to_plain_text(&hover.markdown);
            if plain.is_empty() {
                Vec::new()
            } else {
                vec![
                    TransientMenuItem::new(
                        "hover.0",
                        plain.clone(),
                        TransientMenuAction::new("language.dismissResult"),
                    )
                    .with_detail(format!(
                        "{} {}",
                        result.provenance.package_name, result.provenance.package_version
                    ))
                    .with_accessibility_label(format!("Hover {plain}"))
                    .with_package_provenance(
                        result.provenance.package_name.clone(),
                        result.provenance.package_version.clone(),
                    ),
                ]
            }
        }
        LanguageIntelligencePayload::SignatureHelp(help) => {
            if help.signatures.is_empty() {
                Vec::new()
            } else {
                let active = help
                    .active_signature
                    .map(usize::from)
                    .unwrap_or(0)
                    .min(help.signatures.len().saturating_sub(1));
                help.signatures
                    .iter()
                    .enumerate()
                    .map(|(index, signature)| {
                        let documentation = markdown_to_plain_text(&signature.documentation);
                        let detail = if index == active {
                            if documentation.is_empty() {
                                "active signature".to_string()
                            } else {
                                format!("active · {documentation}")
                            }
                        } else {
                            documentation
                        };
                        TransientMenuItem::new(
                            format!("signature.{index}"),
                            signature.label.clone(),
                            TransientMenuAction::new("language.dismissResult"),
                        )
                        .with_detail(detail)
                        .with_accessibility_label(format!("Signature {}", signature.label))
                        .with_package_provenance(
                            result.provenance.package_name.clone(),
                            result.provenance.package_version.clone(),
                        )
                    })
                    .collect()
            }
        }
        LanguageIntelligencePayload::GoToDefinition(definition) => definition
            .locations
            .iter()
            .enumerate()
            .filter_map(|(index, location)| definition_location_to_menu_item(index, location))
            .collect(),
        LanguageIntelligencePayload::CodeAction(actions) => actions
            .actions
            .iter()
            .enumerate()
            .map(|(index, action)| {
                let mut detail_parts = Vec::new();
                if let Some(preview) = &action.edit {
                    let edit_summary = preview
                        .edits
                        .iter()
                        .map(|edit| {
                            format!(
                                "[{}-{}] {}",
                                edit.range.byte_start,
                                edit.range.byte_end,
                                truncate(&edit.replacement, 64)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                    detail_parts.push(format!("preview: {edit_summary}"));
                }
                if let Some(command_id) = &action.command_id {
                    detail_parts.push(format!("command: {command_id}"));
                }
                let detail = if detail_parts.is_empty() {
                    format!(
                        "{} {}",
                        result.provenance.package_name, result.provenance.package_version
                    )
                } else {
                    detail_parts.join(" · ")
                };
                let menu_action = if let Some(command_id) = &action.command_id {
                    TransientMenuAction::new(command_id.clone())
                } else {
                    // Inert edit-preview-only actions never mutate text in Phase 18.20.
                    TransientMenuAction::new("language.previewEdit").with_arguments(
                        serde_json::json!({
                            "title": action.title,
                            "previewOnly": true,
                        }),
                    )
                };
                TransientMenuItem::new(
                    format!("codeAction.{index}"),
                    action.title.clone(),
                    menu_action,
                )
                .with_detail(detail)
                .with_accessibility_label(format!("Code action {}", action.title))
                .with_package_provenance(
                    result.provenance.package_name.clone(),
                    result.provenance.package_version.clone(),
                )
            })
            .collect(),
    };

    let focus = match result.feature {
        LanguageIntelligenceFeature::Hover | LanguageIntelligenceFeature::SignatureHelp => {
            TransientMenuFocusPolicy::Modeless
        }
        LanguageIntelligenceFeature::GoToDefinition | LanguageIntelligenceFeature::CodeAction => {
            TransientMenuFocusPolicy::Modal
        }
    };

    let session = TransientMenuSession::new(TransientMenuSessionId(result.request_id), prompt)
        .with_focus_policy(focus)
        .with_items(items);
    if session.items().is_empty() {
        session.with_empty_status(empty_status)
    } else {
        session
    }
}

fn definition_location_to_menu_item(
    index: usize,
    location: &TextLocation,
) -> Option<TransientMenuItem> {
    match location {
        TextLocation::OpenDocument { document_id, range } => {
            let label = format!(
                "document {document_id} [{}-{}]",
                range.byte_start, range.byte_end
            );
            let action = TransientMenuAction::new("language.navigateDefinition").with_arguments(
                serde_json::json!({
                    "kind": "openDocument",
                    "documentId": document_id,
                    "byteStart": range.byte_start,
                    "byteEnd": range.byte_end,
                }),
            );
            Some(
                TransientMenuItem::new(format!("definition.{index}"), label.clone(), action)
                    .with_accessibility_label(format!("Go to {label}")),
            )
        }
        TextLocation::WorkspaceFile {
            workspace_root_id,
            relative_path,
            range,
        } => {
            let label = format!("{relative_path} [{}-{}]", range.byte_start, range.byte_end);
            // Reuse the existing workspace open command; pending caret jump is
            // applied client-side after DocumentOpened.
            let action =
                TransientMenuAction::new("workspace.openFile").with_arguments(serde_json::json!({
                    "workspaceRootId": workspace_root_id,
                    "relativePath": relative_path,
                    "byteStart": range.byte_start,
                    "byteEnd": range.byte_end,
                    "languageIntelligenceNavigation": true,
                }));
            Some(
                TransientMenuItem::new(format!("definition.{index}"), label.clone(), action)
                    .with_detail(format!("workspace root {workspace_root_id}"))
                    .with_accessibility_label(format!("Go to {label}")),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(id: &str, label: &str) -> TransientMenuItem {
        TransientMenuItem::new(id, label, TransientMenuAction::new("builtIn.test"))
    }

    #[test]
    fn tab_close_confirm_session_lists_three_choices_with_client_id_arguments() {
        // Phase 22.4: the driver-owned tab-close confirm menu. Every action
        // carries the tab's client id (the pane view hands the selection back
        // to the driver via `EditorAction::TabCloseMenuAction`); the action
        // ids are driver-local and never collide with the per-view
        // save-conflict family.
        let session = super::super::tab_close_confirm_session(
            9,
            "Close tab 'work' with 2 unsaved documents (a.md, b.md)?".to_string(),
            42,
        );
        assert_eq!(
            session.prompt(),
            "Close tab 'work' with 2 unsaved documents (a.md, b.md)?"
        );
        let items = session.items();
        assert_eq!(items.len(), 3);
        let labels = items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec!["Save all and close", "Discard and close", "Cancel"]
        );
        for item in items {
            assert_eq!(
                item.action
                    .arguments
                    .get("clientId")
                    .and_then(|v| v.as_u64()),
                Some(42),
                "every choice carries the tab's client id"
            );
            assert!(
                !item.accessibility_label.is_empty(),
                "every choice has an accessibility label"
            );
        }
        assert_eq!(items[0].action.command_id, "shell.clientTabCloseSaveAll");
        assert_eq!(items[1].action.command_id, "shell.clientTabCloseDiscard");
        assert_eq!(items[2].action.command_id, "shell.clientTabCloseCancel");
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
                    TransientMenuAction::new("builtIn.run")
                        .with_arguments(serde_json::json!({"foo": "bar"})),
                ),
            ]);

        let action = session.activate_selected().expect("first item has action");
        assert_eq!(action.command_id, "builtIn.test");

        let mut session = session;
        session.select_next();
        let action = session.activate_selected().expect("second item has action");
        assert_eq!(action.command_id, "builtIn.run");
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
                text_format: crate::protocol::CompletionItemTextFormat::PlainText,
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
        assert_eq!(
            accept.text_format,
            crate::protocol::CompletionItemTextFormat::PlainText
        );
        assert_eq!(accept.commit_characters, ";");
    }

    #[test]
    fn markdown_to_plain_text_strips_html_and_common_markers() {
        let plain = markdown_to_plain_text(
            "# Title\n**bold** and <script>alert(1)</script> [link](https://evil.example)",
        );
        assert!(plain.contains("Title"));
        assert!(plain.contains("bold"));
        assert!(plain.contains("link"));
        assert!(!plain.contains("<script>"));
        assert!(!plain.contains("https://evil.example"));
        assert!(!plain.contains("**"));
    }

    #[test]
    fn language_intelligence_hover_projects_to_modeless_plain_text_menu() {
        use crate::protocol::{
            CompletionProvenance, HoverResult, LanguageIntelligenceFeature,
            LanguageIntelligencePayload, LanguageIntelligenceResult, LanguageIntelligenceStatus,
        };

        let result = LanguageIntelligenceResult {
            request_id: 9,
            client_id: 1,
            document_id: 7,
            document_version: 3,
            behavior_version: 2,
            provider_generation: 0,
            feature: LanguageIntelligenceFeature::Hover,
            status: LanguageIntelligenceStatus::Ok,
            payload: LanguageIntelligencePayload::Hover(HoverResult {
                range: None,
                markdown: "**fn** `main` <b>doc</b>".to_string(),
            }),
            provenance: CompletionProvenance::builtin_core(),
        };
        let session = language_intelligence_result_to_menu_session(&result);
        assert_eq!(session.prompt(), "Hover");
        assert_eq!(session.focus_policy(), TransientMenuFocusPolicy::Modeless);
        assert_eq!(session.items().len(), 1);
        assert!(!session.items()[0].label.contains("<"));
        assert!(!session.items()[0].label.contains("**"));
        assert_eq!(
            session.items()[0].action.command_id,
            "language.dismissResult"
        );
    }

    #[test]
    fn language_intelligence_definitions_and_code_actions_project_to_selectable_menu() {
        use crate::protocol::{
            CodeAction, CodeActionResult, CompletionProvenance, EditPreview, GoToDefinitionResult,
            LanguageIntelligenceFeature, LanguageIntelligencePayload, LanguageIntelligenceResult,
            LanguageIntelligenceStatus, RangeEdit, TextByteRange, TextLocation,
        };

        let definition = LanguageIntelligenceResult {
            request_id: 1,
            client_id: 1,
            document_id: 7,
            document_version: 1,
            behavior_version: 1,
            provider_generation: 0,
            feature: LanguageIntelligenceFeature::GoToDefinition,
            status: LanguageIntelligenceStatus::Ok,
            payload: LanguageIntelligencePayload::GoToDefinition(GoToDefinitionResult {
                locations: vec![
                    TextLocation::OpenDocument {
                        document_id: 7,
                        range: TextByteRange {
                            byte_start: 4,
                            byte_end: 8,
                        },
                    },
                    TextLocation::WorkspaceFile {
                        workspace_root_id: 1,
                        relative_path: "src/lib.rs".to_string(),
                        range: TextByteRange {
                            byte_start: 10,
                            byte_end: 14,
                        },
                    },
                ],
            }),
            provenance: CompletionProvenance::builtin_core(),
        };
        let definition_menu = language_intelligence_result_to_menu_session(&definition);
        assert_eq!(definition_menu.prompt(), "Definitions");
        assert_eq!(
            definition_menu.focus_policy(),
            TransientMenuFocusPolicy::Modal
        );
        assert_eq!(definition_menu.items().len(), 2);
        assert_eq!(
            definition_menu.items()[0].action.command_id,
            "language.navigateDefinition"
        );
        assert_eq!(
            definition_menu.items()[1].action.command_id,
            "workspace.openFile"
        );
        assert_eq!(
            definition_menu.items()[1].action.arguments["relativePath"],
            "src/lib.rs"
        );

        let actions = LanguageIntelligenceResult {
            request_id: 2,
            client_id: 1,
            document_id: 7,
            document_version: 1,
            behavior_version: 1,
            provider_generation: 0,
            feature: LanguageIntelligenceFeature::CodeAction,
            status: LanguageIntelligenceStatus::Ok,
            payload: LanguageIntelligencePayload::CodeAction(CodeActionResult {
                actions: vec![
                    CodeAction {
                        range: TextByteRange {
                            byte_start: 0,
                            byte_end: 1,
                        },
                        title: "Rename symbol".to_string(),
                        command_id: Some("pkg.rename".to_string()),
                        edit: None,
                    },
                    CodeAction {
                        range: TextByteRange {
                            byte_start: 0,
                            byte_end: 1,
                        },
                        title: "Inline preview".to_string(),
                        command_id: None,
                        edit: Some(EditPreview {
                            document_id: 7,
                            document_version: 1,
                            edits: vec![RangeEdit {
                                range: TextByteRange {
                                    byte_start: 0,
                                    byte_end: 1,
                                },
                                replacement: "X".to_string(),
                            }],
                        }),
                    },
                ],
            }),
            provenance: CompletionProvenance::builtin_core(),
        };
        let action_menu = language_intelligence_result_to_menu_session(&actions);
        assert_eq!(action_menu.items().len(), 2);
        assert_eq!(action_menu.items()[0].action.command_id, "pkg.rename");
        assert_eq!(
            action_menu.items()[1].action.command_id,
            "language.previewEdit"
        );
        assert_eq!(action_menu.items()[1].action.arguments["previewOnly"], true);
    }
}
