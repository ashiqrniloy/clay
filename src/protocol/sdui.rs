use super::{DocumentId, DocumentVersion};

pub type SduiVersion = u64;

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
pub struct SduiNodeId(pub u64);

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SduiTree {
    pub ui_version: SduiVersion,
    pub root_id: SduiNodeId,
    pub nodes: Vec<SduiNode>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SduiNode {
    pub id: SduiNodeId,
    pub kind: SduiNodeKind,
}

impl SduiNode {
    pub const fn new(id: SduiNodeId, kind: SduiNodeKind) -> Self {
        Self { id, kind }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SduiNodeKind {
    Panel {
        title: String,
        children: Vec<SduiNodeId>,
    },
    Label {
        text: String,
    },
    Button {
        label: String,
        action: SduiActionIntent,
    },
    List {
        items: Vec<SduiListItem>,
    },
    EditorView {
        binding: SduiEditorBinding,
    },
    Flex {
        direction: SduiFlexDirection,
        children: Vec<SduiNodeId>,
    },
    Stack {
        children: Vec<SduiNodeId>,
    },
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SduiListItem {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
    pub action: Option<SduiActionIntent>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SduiEditorBinding {
    pub document_id: DocumentId,
    pub expected_version: Option<DocumentVersion>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SduiFlexDirection {
    Row,
    Column,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SduiActionIntent {
    pub command_id: String,
    pub source: SduiActionSource,
    pub arguments: Vec<SduiActionArgument>,
}

impl SduiActionIntent {
    pub fn command(command_id: impl Into<String>, source: SduiActionSource) -> Self {
        Self {
            command_id: command_id.into(),
            source,
            arguments: Vec::new(),
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SduiActionSource {
    Button {
        node_id: SduiNodeId,
    },
    ListItem {
        node_id: SduiNodeId,
        item_id: String,
    },
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SduiActionArgument {
    pub name: String,
    pub value: SduiActionValue,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SduiActionValue {
    String(String),
    Bool(bool),
    I64(i64),
    U64(u64),
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SduiTreeUpdate {
    pub base_ui_version: SduiVersion,
    pub new_ui_version: SduiVersion,
    pub operations: Vec<SduiTreeOperation>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SduiTreeOperation {
    ReplaceRoot { root_id: SduiNodeId },
    ReplaceNode { node: SduiNode },
    RemoveNode { node_id: SduiNodeId },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdui_schema_represents_initial_widget_kinds() {
        let root_id = SduiNodeId(1);
        let sidebar_id = SduiNodeId(2);
        let label_id = SduiNodeId(3);
        let button_id = SduiNodeId(4);
        let list_id = SduiNodeId(5);
        let editor_id = SduiNodeId(6);
        let stack_id = SduiNodeId(7);

        let button_action = SduiActionIntent::command(
            "workspace.refresh",
            SduiActionSource::Button { node_id: button_id },
        );
        let list_action = SduiActionIntent::command(
            "document.open_recent",
            SduiActionSource::ListItem {
                node_id: list_id,
                item_id: "recent-main".to_string(),
            },
        );

        let tree = SduiTree {
            ui_version: 1,
            root_id,
            nodes: vec![
                SduiNode::new(
                    root_id,
                    SduiNodeKind::Flex {
                        direction: SduiFlexDirection::Row,
                        children: vec![sidebar_id, editor_id],
                    },
                ),
                SduiNode::new(
                    sidebar_id,
                    SduiNodeKind::Panel {
                        title: "Workspace".to_string(),
                        children: vec![stack_id],
                    },
                ),
                SduiNode::new(
                    stack_id,
                    SduiNodeKind::Stack {
                        children: vec![label_id, button_id, list_id],
                    },
                ),
                SduiNode::new(
                    label_id,
                    SduiNodeKind::Label {
                        text: "Open files".to_string(),
                    },
                ),
                SduiNode::new(
                    button_id,
                    SduiNodeKind::Button {
                        label: "Refresh".to_string(),
                        action: button_action,
                    },
                ),
                SduiNode::new(
                    list_id,
                    SduiNodeKind::List {
                        items: vec![SduiListItem {
                            id: "recent-main".to_string(),
                            label: "main.rs".to_string(),
                            detail: Some("src/main.rs".to_string()),
                            action: Some(list_action),
                        }],
                    },
                ),
                SduiNode::new(
                    editor_id,
                    SduiNodeKind::EditorView {
                        binding: SduiEditorBinding {
                            document_id: 42,
                            expected_version: Some(7),
                        },
                    },
                ),
            ],
        };

        assert_eq!(tree.root_id, root_id);
        assert_eq!(tree.nodes.len(), 7);
        assert!(matches!(tree.nodes[0].kind, SduiNodeKind::Flex { .. }));
        assert!(matches!(tree.nodes[1].kind, SduiNodeKind::Panel { .. }));
        assert!(matches!(tree.nodes[2].kind, SduiNodeKind::Stack { .. }));
        assert!(matches!(tree.nodes[3].kind, SduiNodeKind::Label { .. }));
        assert!(matches!(tree.nodes[4].kind, SduiNodeKind::Button { .. }));
        assert!(matches!(tree.nodes[5].kind, SduiNodeKind::List { .. }));
        assert!(matches!(
            tree.nodes[6].kind,
            SduiNodeKind::EditorView { .. }
        ));
    }

    #[test]
    fn sdui_editor_view_uses_document_binding_not_text_payload() {
        let node = SduiNode::new(
            SduiNodeId(10),
            SduiNodeKind::EditorView {
                binding: SduiEditorBinding {
                    document_id: 99,
                    expected_version: None,
                },
            },
        );

        assert_eq!(
            node,
            SduiNode {
                id: SduiNodeId(10),
                kind: SduiNodeKind::EditorView {
                    binding: SduiEditorBinding {
                        document_id: 99,
                        expected_version: None,
                    },
                },
            }
        );
    }

    #[test]
    fn sdui_actions_are_server_routed_intents() {
        let node_id = SduiNodeId(11);
        let intent = SduiActionIntent {
            command_id: "workspace.refresh".to_string(),
            source: SduiActionSource::Button { node_id },
            arguments: vec![SduiActionArgument {
                name: "force".to_string(),
                value: SduiActionValue::Bool(true),
            }],
        };

        let node = SduiNode::new(
            node_id,
            SduiNodeKind::Button {
                label: "Refresh".to_string(),
                action: intent.clone(),
            },
        );

        assert_eq!(intent.command_id, "workspace.refresh");
        assert_eq!(intent.source, SduiActionSource::Button { node_id });
        assert_eq!(intent.arguments.len(), 1);
        assert!(matches!(node.kind, SduiNodeKind::Button { .. }));
    }

    #[test]
    fn sdui_updates_target_stable_node_ids() {
        let updated = SduiNode::new(
            SduiNodeId(3),
            SduiNodeKind::Label {
                text: "Updated".to_string(),
            },
        );

        let update = SduiTreeUpdate {
            base_ui_version: 1,
            new_ui_version: 2,
            operations: vec![
                SduiTreeOperation::ReplaceNode {
                    node: updated.clone(),
                },
                SduiTreeOperation::RemoveNode {
                    node_id: SduiNodeId(4),
                },
            ],
        };

        assert_eq!(update.base_ui_version, 1);
        assert_eq!(update.new_ui_version, 2);
        assert_eq!(
            update.operations[0],
            SduiTreeOperation::ReplaceNode { node: updated }
        );
        assert_eq!(
            update.operations[1],
            SduiTreeOperation::RemoveNode {
                node_id: SduiNodeId(4)
            }
        );
    }
}
