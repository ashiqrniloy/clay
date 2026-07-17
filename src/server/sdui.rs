#![allow(
    dead_code,
    reason = "static SDUI tree helpers remain as validation fixtures while runtime SDUI publication is opt-in"
)]

use std::collections::BTreeSet;

use crate::protocol::{
    DocumentId, DocumentVersion, ProtocolErrorCode, SduiActionIntent, SduiActionSource,
    SduiEditorBinding, SduiFlexDirection, SduiListItem, SduiNode, SduiNodeId, SduiNodeKind,
    SduiTree, SduiTreeOperation, SduiTreeUpdate, ServerMessage,
};

const DEFAULT_UI_VERSION: u64 = 1;
const ROOT_ID: SduiNodeId = SduiNodeId(1);
const SIDEBAR_ID: SduiNodeId = SduiNodeId(2);
const SIDEBAR_STACK_ID: SduiNodeId = SduiNodeId(3);
const STATUS_LABEL_ID: SduiNodeId = SduiNodeId(4);
const REFRESH_BUTTON_ID: SduiNodeId = SduiNodeId(5);
const DOCUMENT_LIST_ID: SduiNodeId = SduiNodeId(6);
const EDITOR_ID: SduiNodeId = SduiNodeId(7);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SduiValidationError {
    EmptyTree,
    DuplicateNodeId(SduiNodeId),
    MissingRoot(SduiNodeId),
    UnknownChild {
        parent_id: SduiNodeId,
        child_id: SduiNodeId,
    },
    UnknownUpdateNode(SduiNodeId),
    StaleUpdate {
        base_ui_version: u64,
        current_ui_version: u64,
    },
    NonAdvancingUpdate {
        base_ui_version: u64,
        new_ui_version: u64,
    },
    UnknownActionNode(SduiNodeId),
    UnknownActionCommand(String),
    ActionSourceMismatch(SduiNodeId),
    UnknownEditorDocument {
        node_id: SduiNodeId,
        document_id: DocumentId,
    },
}

impl SduiValidationError {
    fn into_message(self) -> ServerMessage {
        ServerMessage::Error {
            code: ProtocolErrorCode::InvalidMessage,
            message: format!("invalid SDUI message: {self:?}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaticSduiState {
    document_id: DocumentId,
    tree: Option<SduiTree>,
}

impl StaticSduiState {
    pub(crate) fn empty_for_document(document_id: DocumentId) -> Self {
        Self {
            document_id,
            tree: None,
        }
    }

    pub(crate) fn for_document(document_id: DocumentId, document_version: DocumentVersion) -> Self {
        let tree = default_document_tree(document_id, document_version);
        validate_static_tree(&tree).expect("static SDUI tree must be valid before publication");
        validate_editor_bindings(&tree, document_id)
            .expect("static SDUI editor views must bind to the open document");
        Self {
            document_id,
            tree: Some(tree),
        }
    }

    pub(crate) fn snapshot_message(&self, client_id: u64) -> Option<ServerMessage> {
        self.tree
            .clone()
            .map(|tree| ServerMessage::SduiSnapshot { client_id, tree })
    }

    pub(crate) fn cloned_tree_or_default(&self) -> SduiTree {
        self.tree
            .clone()
            .unwrap_or_else(|| default_document_tree(self.document_id, 1))
    }

    pub(crate) fn replace_with_runtime_tree(
        &mut self,
        tree: SduiTree,
    ) -> Result<(), SduiValidationError> {
        self.replace_for_document_with_runtime_tree(self.document_id, tree)
    }

    /// The document id this static SDUI state is currently bound to.
    pub(crate) fn document_id(&self) -> DocumentId {
        self.document_id
    }

    pub(crate) fn replace_for_document_with_runtime_tree(
        &mut self,
        document_id: u64,
        tree: SduiTree,
    ) -> Result<(), SduiValidationError> {
        validate_runtime_tree(&tree, document_id)?;
        self.document_id = document_id;
        self.tree = Some(tree);
        Ok(())
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "bounded SDUI update publication is exposed for Phase 12 client mapping"
        )
    )]
    pub(crate) fn update_message(
        &mut self,
        update: SduiTreeUpdate,
    ) -> Result<ServerMessage, SduiValidationError> {
        self.validate_update(&update)?;
        let Some(tree) = &mut self.tree else {
            return Err(SduiValidationError::EmptyTree);
        };
        apply_update(tree, &update);
        Ok(ServerMessage::SduiUpdate { update })
    }

    pub(crate) fn validate_action(
        &self,
        intent: &SduiActionIntent,
    ) -> Result<(), SduiValidationError> {
        let Some(tree) = &self.tree else {
            return Err(SduiValidationError::UnknownActionCommand(
                intent.command_id.clone(),
            ));
        };
        if !tree_declares_action_command(tree, &intent.command_id) {
            return Err(SduiValidationError::UnknownActionCommand(
                intent.command_id.clone(),
            ));
        }

        match &intent.source {
            SduiActionSource::Button { node_id } => {
                let Some(node) = self.node(*node_id) else {
                    return Err(SduiValidationError::UnknownActionNode(*node_id));
                };
                match &node.kind {
                    SduiNodeKind::Button { action, .. }
                        if action.command_id == intent.command_id =>
                    {
                        Ok(())
                    }
                    _ => Err(SduiValidationError::ActionSourceMismatch(*node_id)),
                }
            }
            SduiActionSource::ListItem { node_id, item_id } => {
                let Some(node) = self.node(*node_id) else {
                    return Err(SduiValidationError::UnknownActionNode(*node_id));
                };
                match &node.kind {
                    SduiNodeKind::List { items } => items
                        .iter()
                        .find(|item| item.id == *item_id)
                        .and_then(|item| item.action.as_ref())
                        .filter(|action| action.command_id == intent.command_id)
                        .map(|_| ())
                        .ok_or(SduiValidationError::ActionSourceMismatch(*node_id)),
                    _ => Err(SduiValidationError::ActionSourceMismatch(*node_id)),
                }
            }
        }
    }

    fn validate_update(&self, update: &SduiTreeUpdate) -> Result<(), SduiValidationError> {
        let Some(tree) = &self.tree else {
            return Err(SduiValidationError::EmptyTree);
        };
        if update.base_ui_version != tree.ui_version {
            return Err(SduiValidationError::StaleUpdate {
                base_ui_version: update.base_ui_version,
                current_ui_version: tree.ui_version,
            });
        }
        if update.new_ui_version <= update.base_ui_version {
            return Err(SduiValidationError::NonAdvancingUpdate {
                base_ui_version: update.base_ui_version,
                new_ui_version: update.new_ui_version,
            });
        }

        let known_ids: BTreeSet<_> = tree.nodes.iter().map(|node| node.id).collect();
        for operation in &update.operations {
            match operation {
                SduiTreeOperation::ReplaceRoot { root_id } => {
                    if !known_ids.contains(root_id)
                        && !update.operations.iter().any(|operation| {
                            matches!(operation, SduiTreeOperation::ReplaceNode { node } if node.id == *root_id)
                        })
                    {
                        return Err(SduiValidationError::UnknownUpdateNode(*root_id));
                    }
                }
                SduiTreeOperation::ReplaceNode { node } => {
                    validate_node_children(node, &known_ids)?;
                    validate_editor_binding(node, self.document_id)?;
                }
                SduiTreeOperation::RemoveNode { node_id } => {
                    if !known_ids.contains(node_id) {
                        return Err(SduiValidationError::UnknownUpdateNode(*node_id));
                    }
                }
            }
        }
        Ok(())
    }

    fn node(&self, node_id: SduiNodeId) -> Option<&SduiNode> {
        self.tree
            .as_ref()?
            .nodes
            .iter()
            .find(|node| node.id == node_id)
    }
}

pub(crate) fn default_document_tree(
    document_id: DocumentId,
    document_version: DocumentVersion,
) -> SduiTree {
    let refresh_action = SduiActionIntent::command(
        "workspace.refresh",
        SduiActionSource::Button {
            node_id: REFRESH_BUTTON_ID,
        },
    );
    let recent_action = SduiActionIntent::command(
        "document.open_recent",
        SduiActionSource::ListItem {
            node_id: DOCUMENT_LIST_ID,
            item_id: "active-document".to_string(),
        },
    );

    SduiTree {
        ui_version: DEFAULT_UI_VERSION,
        root_id: ROOT_ID,
        nodes: vec![
            SduiNode::new(
                ROOT_ID,
                SduiNodeKind::Flex {
                    direction: SduiFlexDirection::Row,
                    children: vec![SIDEBAR_ID, EDITOR_ID],
                },
            ),
            SduiNode::new(
                SIDEBAR_ID,
                SduiNodeKind::Panel {
                    title: "Workspace".to_string(),
                    children: vec![SIDEBAR_STACK_ID],
                },
            ),
            SduiNode::new(
                SIDEBAR_STACK_ID,
                SduiNodeKind::Stack {
                    children: vec![STATUS_LABEL_ID, REFRESH_BUTTON_ID, DOCUMENT_LIST_ID],
                },
            ),
            SduiNode::new(
                STATUS_LABEL_ID,
                SduiNodeKind::Label {
                    text: format!("Document {document_id} · version {document_version}"),
                },
            ),
            SduiNode::new(
                REFRESH_BUTTON_ID,
                SduiNodeKind::Button {
                    label: "Refresh".to_string(),
                    action: refresh_action,
                },
            ),
            SduiNode::new(
                DOCUMENT_LIST_ID,
                SduiNodeKind::List {
                    items: vec![SduiListItem {
                        id: "active-document".to_string(),
                        label: format!("Document {document_id}"),
                        detail: Some("Server-generated editor view".to_string()),
                        action: Some(recent_action),
                    }],
                },
            ),
            SduiNode::new(
                EDITOR_ID,
                SduiNodeKind::EditorView {
                    binding: SduiEditorBinding {
                        document_id,
                        expected_version: Some(document_version),
                    },
                },
            ),
        ],
    }
}

pub(crate) fn validate_runtime_tree(
    tree: &SduiTree,
    expected_document_id: DocumentId,
) -> Result<(), SduiValidationError> {
    validate_static_tree(tree)?;
    validate_editor_bindings(tree, expected_document_id)
}

pub(crate) fn validate_static_tree(tree: &SduiTree) -> Result<(), SduiValidationError> {
    if tree.nodes.is_empty() {
        return Err(SduiValidationError::EmptyTree);
    }

    let mut ids = BTreeSet::new();
    for node in &tree.nodes {
        if !ids.insert(node.id) {
            return Err(SduiValidationError::DuplicateNodeId(node.id));
        }
    }
    if !ids.contains(&tree.root_id) {
        return Err(SduiValidationError::MissingRoot(tree.root_id));
    }
    for node in &tree.nodes {
        validate_node_children(node, &ids)?;
    }
    Ok(())
}

fn validate_node_children(
    node: &SduiNode,
    known_ids: &BTreeSet<SduiNodeId>,
) -> Result<(), SduiValidationError> {
    let children: &[SduiNodeId] = match &node.kind {
        SduiNodeKind::Panel { children, .. }
        | SduiNodeKind::Flex { children, .. }
        | SduiNodeKind::Stack { children } => children,
        SduiNodeKind::Label { .. }
        | SduiNodeKind::Button { .. }
        | SduiNodeKind::List { .. }
        | SduiNodeKind::EditorView { .. } => &[],
    };

    for child_id in children {
        if !known_ids.contains(child_id) {
            return Err(SduiValidationError::UnknownChild {
                parent_id: node.id,
                child_id: *child_id,
            });
        }
    }
    Ok(())
}

fn validate_editor_bindings(
    tree: &SduiTree,
    expected_document_id: DocumentId,
) -> Result<(), SduiValidationError> {
    for node in &tree.nodes {
        validate_editor_binding(node, expected_document_id)?;
    }
    Ok(())
}

fn validate_editor_binding(
    node: &SduiNode,
    expected_document_id: DocumentId,
) -> Result<(), SduiValidationError> {
    if let SduiNodeKind::EditorView { binding } = &node.kind
        && binding.document_id != expected_document_id
    {
        return Err(SduiValidationError::UnknownEditorDocument {
            node_id: node.id,
            document_id: binding.document_id,
        });
    }
    Ok(())
}

fn tree_declares_action_command(tree: &SduiTree, command_id: &str) -> bool {
    tree.nodes.iter().any(|node| match &node.kind {
        SduiNodeKind::Button { action, .. } => action.command_id == command_id,
        SduiNodeKind::List { items } => items
            .iter()
            .filter_map(|item| item.action.as_ref())
            .any(|action| action.command_id == command_id),
        SduiNodeKind::Panel { .. }
        | SduiNodeKind::Label { .. }
        | SduiNodeKind::EditorView { .. }
        | SduiNodeKind::Flex { .. }
        | SduiNodeKind::Stack { .. } => false,
    })
}

fn apply_update(tree: &mut SduiTree, update: &SduiTreeUpdate) {
    for operation in &update.operations {
        match operation {
            SduiTreeOperation::ReplaceRoot { root_id } => tree.root_id = *root_id,
            SduiTreeOperation::ReplaceNode { node } => {
                if let Some(existing) = tree
                    .nodes
                    .iter_mut()
                    .find(|existing| existing.id == node.id)
                {
                    *existing = node.clone();
                } else {
                    tree.nodes.push(node.clone());
                }
            }
            SduiTreeOperation::RemoveNode { node_id } => {
                tree.nodes.retain(|node| node.id != *node_id);
            }
        }
    }
    tree.ui_version = update.new_ui_version;
}

pub(crate) fn sdui_action_response(
    state: &StaticSduiState,
    intent: &SduiActionIntent,
) -> Option<ServerMessage> {
    state
        .validate_action(intent)
        .err()
        .map(|error| error.into_message())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_sdui_state_publishes_no_snapshot() {
        let state = StaticSduiState::empty_for_document(7);

        assert_eq!(state.snapshot_message(1), None);
    }

    #[test]
    fn default_sdui_tree_is_valid_and_static() {
        let tree = default_document_tree(7, 3);

        validate_static_tree(&tree).unwrap();
        validate_editor_bindings(&tree, 7).unwrap();
        assert_eq!(tree.ui_version, 1);
        assert_eq!(tree.root_id, ROOT_ID);
        assert!(tree.nodes.iter().any(|node| matches!(
            node.kind,
            SduiNodeKind::EditorView {
                binding: SduiEditorBinding {
                    document_id: 7,
                    expected_version: Some(3),
                }
            }
        )));
    }

    #[test]
    fn default_sdui_contains_editor_and_panel_regions() {
        let tree = default_document_tree(7, 3);
        let root = tree.nodes.iter().find(|node| node.id == ROOT_ID).unwrap();

        assert!(matches!(
            &root.kind,
            SduiNodeKind::Flex {
                direction: SduiFlexDirection::Row,
                children
            } if children == &vec![SIDEBAR_ID, EDITOR_ID]
        ));
        assert!(
            tree.nodes
                .iter()
                .any(|node| matches!(node.kind, SduiNodeKind::Panel { .. }))
        );
        assert!(
            tree.nodes
                .iter()
                .any(|node| matches!(node.kind, SduiNodeKind::List { .. }))
        );
        assert!(
            tree.nodes
                .iter()
                .any(|node| matches!(node.kind, SduiNodeKind::EditorView { .. }))
        );
    }

    #[test]
    fn sdui_update_rejects_unknown_node_id() {
        let mut state = StaticSduiState::for_document(7, 1);
        let error = state
            .update_message(SduiTreeUpdate {
                base_ui_version: 1,
                new_ui_version: 2,
                operations: vec![SduiTreeOperation::RemoveNode {
                    node_id: SduiNodeId(999),
                }],
            })
            .unwrap_err();

        assert_eq!(
            error,
            SduiValidationError::UnknownUpdateNode(SduiNodeId(999))
        );
    }

    #[test]
    fn sdui_action_validation_rejects_unknown_command() {
        let state = StaticSduiState::for_document(7, 1);
        let error = state
            .validate_action(&SduiActionIntent::command(
                "shell.run",
                SduiActionSource::Button {
                    node_id: REFRESH_BUTTON_ID,
                },
            ))
            .unwrap_err();

        assert_eq!(
            error,
            SduiValidationError::UnknownActionCommand("shell.run".to_string())
        );
    }

    #[test]
    fn editor_view_requires_known_document_binding() {
        let mut state = StaticSduiState::for_document(7, 1);
        let error = state
            .update_message(SduiTreeUpdate {
                base_ui_version: 1,
                new_ui_version: 2,
                operations: vec![SduiTreeOperation::ReplaceNode {
                    node: SduiNode::new(
                        EDITOR_ID,
                        SduiNodeKind::EditorView {
                            binding: SduiEditorBinding {
                                document_id: 999,
                                expected_version: Some(1),
                            },
                        },
                    ),
                }],
            })
            .unwrap_err();

        assert_eq!(
            error,
            SduiValidationError::UnknownEditorDocument {
                node_id: EDITOR_ID,
                document_id: 999,
            }
        );
    }
}
