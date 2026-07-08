//! Clay-owned file browser UI state.
//!
//! The file browser is a first-party shell surface, not a package
//! contribution. It composes existing primitives:
//! - a left fixed panel (`FixedSlotId::Left`) rendered from inert SDUI state;
//! - a bottom transient fuzzy-open overlay built on `TransientMenuSession`;
//! - `CommandExecution` intents for open/reveal actions.
//!
//! It performs no filesystem reads during paint/layout: all directory data is
//! installed as a bounded listing snapshot before rendering.

#![allow(dead_code)]

use std::path::PathBuf;

use crate::protocol::{
    DocumentId, DocumentVersion, SduiActionArgument, SduiActionIntent, SduiActionSource,
    SduiActionValue, SduiEditorBinding, SduiFlexDirection, SduiListItem, SduiNode, SduiNodeId,
    SduiNodeKind, SduiTree, WorkspaceRootId,
};
use crate::server::workspace::{FileListEntryKind, WorkspaceState};

use super::transient_menu::{
    TransientMenuAction, TransientMenuItem, TransientMenuSession, TransientMenuSessionId,
};

/// Maximum number of file entries the browser will render in the left panel.
const MAX_LEFT_PANEL_ENTRIES: usize = 256;
/// Maximum number of fuzzy-open items shown at once.
const MAX_FUZZY_ITEMS: usize = 64;

pub(crate) const OPEN_FILE_COMMAND_ID: &str = "clay.workspace.openFile";
pub(crate) const REVEAL_IN_TREE_COMMAND_ID: &str = "clay.workspace.revealInTree";
pub(crate) const OPEN_FUZZY_FILE_COMMAND_ID: &str = "clay.workspace.openFuzzyFile";
pub(crate) const OPEN_DIRECTORY_COMMAND_ID: &str = "clay.workspace.openDirectory";
pub(crate) const TOGGLE_FILE_BROWSER_COMMAND_ID: &str = "clay.workspace.toggleFileBrowser";

/// Inert snapshot of file listing data used to render the file browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileBrowserState {
    root_id: WorkspaceRootId,
    root_path_display: String,
    current_directory: PathBuf,
    entries: Vec<FileBrowserEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileBrowserEntry {
    pub(crate) name: String,
    pub(crate) relative_path: PathBuf,
    pub(crate) kind: FileBrowserEntryKind,
    pub(crate) child_count: Option<usize>,
    pub(crate) root_id: WorkspaceRootId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileBrowserEntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl From<FileListEntryKind> for FileBrowserEntryKind {
    fn from(kind: FileListEntryKind) -> Self {
        match kind {
            FileListEntryKind::Directory => Self::Directory,
            FileListEntryKind::File => Self::File,
            FileListEntryKind::Symlink => Self::Symlink,
            FileListEntryKind::Other => Self::Other,
        }
    }
}

impl FileBrowserState {
    /// Build a browser state from a workspace listing. Performs a bounded,
    /// synchronous read of the directory tree via the server listing service.
    /// Callers should run this off the paint/layout hot path.
    pub(crate) fn from_workspace(
        workspace: &WorkspaceState,
        root_id: WorkspaceRootId,
    ) -> Result<Self, FileBrowserError> {
        Self::from_workspace_at(workspace, root_id, PathBuf::new())
    }

    pub(crate) fn from_workspace_at(
        workspace: &WorkspaceState,
        root_id: WorkspaceRootId,
        relative_path: PathBuf,
    ) -> Result<Self, FileBrowserError> {
        let page = workspace.list_directory(
            crate::server::workspace::FileListRequest {
                root_id,
                relative_path: relative_path.clone(),
                max_depth: 1,
                max_entries: MAX_LEFT_PANEL_ENTRIES,
            },
            None,
        )?;

        let root_metadata = workspace
            .list_root_metadata()
            .into_iter()
            .find(|root| root.workspace_root_id == root_id)
            .ok_or(FileBrowserError::UnknownRoot(root_id))?;

        let entries = page
            .entries
            .into_iter()
            .map(|entry| FileBrowserEntry {
                name: entry.name,
                relative_path: entry.relative_path,
                kind: entry.kind.into(),
                child_count: entry.child_count,
                root_id,
            })
            .collect();

        Ok(Self {
            root_id,
            root_path_display: root_metadata.display_name,
            current_directory: relative_path,
            entries,
        })
    }

    pub(crate) fn root_id(&self) -> WorkspaceRootId {
        self.root_id
    }

    pub(crate) fn entries(&self) -> &[FileBrowserEntry] {
        &self.entries
    }

    /// Produce an SDUI tree with a left Workspace panel populated by the
    /// inert file listing and a main editor view. No filesystem access.
    pub(crate) fn to_sdui_tree(
        &self,
        document_id: DocumentId,
        document_version: DocumentVersion,
    ) -> SduiTree {
        let root_id = SduiNodeId(1);
        let sidebar_id = SduiNodeId(2);
        let sidebar_stack_id = SduiNodeId(3);
        let title_label_id = SduiNodeId(4);
        let file_list_id = SduiNodeId(5);
        let editor_id = SduiNodeId(6);

        let title = if self.current_directory.as_os_str().is_empty() {
            format!("Workspace · {}", self.root_path_display)
        } else {
            format!(
                "Workspace · {} · {}",
                self.root_path_display,
                self.current_directory.display()
            )
        };
        let title_label = SduiNode::new(title_label_id, SduiNodeKind::Label { text: title });

        let mut list_items: Vec<SduiListItem> = Vec::new();
        if let Some(parent) = self.current_directory.parent() {
            list_items.push(parent_directory_item(file_list_id, self.root_id, parent));
        }
        list_items.extend(
            self.entries
                .iter()
                .map(|entry| entry.to_sdui_list_item(file_list_id)),
        );
        let file_list = SduiNode::new(file_list_id, SduiNodeKind::List { items: list_items });

        let sidebar_stack = SduiNode::new(
            sidebar_stack_id,
            SduiNodeKind::Stack {
                children: vec![title_label_id, file_list_id],
            },
        );

        let sidebar = SduiNode::new(
            sidebar_id,
            SduiNodeKind::Panel {
                title: "Workspace".to_string(),
                children: vec![sidebar_stack_id],
            },
        );

        let editor = SduiNode::new(
            editor_id,
            SduiNodeKind::EditorView {
                binding: SduiEditorBinding {
                    document_id,
                    expected_version: Some(document_version),
                },
            },
        );

        let root = SduiNode::new(
            root_id,
            SduiNodeKind::Flex {
                direction: SduiFlexDirection::Row,
                children: vec![sidebar_id, editor_id],
            },
        );

        SduiTree {
            ui_version: 1,
            root_id,
            nodes: vec![root, sidebar, sidebar_stack, title_label, file_list, editor],
        }
    }

    /// Produce a bottom transient fuzzy-open menu session from the installed
    /// file entries. Filtering happens locally against this bounded snapshot;
    /// no server round-trip is required to update the query.
    pub(crate) fn fuzzy_session(
        &self,
        session_id: TransientMenuSessionId,
        query: &str,
    ) -> TransientMenuSession {
        let query_lower = query.to_lowercase();
        let filtered: Vec<&FileBrowserEntry> = self
            .entries
            .iter()
            .filter(|entry| entry.name.to_lowercase().contains(&query_lower))
            .take(MAX_FUZZY_ITEMS)
            .collect();

        let items: Vec<TransientMenuItem> = filtered
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let relative = entry.relative_path.to_string_lossy().to_string();
                let action = TransientMenuAction::new(OPEN_FUZZY_FILE_COMMAND_ID).with_arguments(
                    serde_json::json!({
                        "workspaceRootId": self.root_id,
                        "relativePath": relative,
                    }),
                );
                TransientMenuItem::new(index.to_string(), entry.name.clone(), action)
                    .with_detail(entry.kind_label())
            })
            .collect();

        let mut session = TransientMenuSession::new(session_id, "Open File").with_items(items);
        session.update_query(query);
        if self.entries.is_empty() {
            session = session.with_empty_status("No files in workspace");
        }
        session
    }
}

impl FileBrowserEntry {
    fn kind_label(&self) -> String {
        match self.kind {
            FileBrowserEntryKind::Directory => "folder".to_string(),
            FileBrowserEntryKind::File => "file".to_string(),
            FileBrowserEntryKind::Symlink => "link".to_string(),
            FileBrowserEntryKind::Other => "other".to_string(),
        }
    }

    fn to_sdui_list_item(&self, list_node_id: SduiNodeId) -> SduiListItem {
        let relative = self.relative_path.to_string_lossy().to_string();
        let command_id = match self.kind {
            FileBrowserEntryKind::Directory => OPEN_DIRECTORY_COMMAND_ID,
            _ => OPEN_FILE_COMMAND_ID,
        };
        let action = SduiActionIntent {
            command_id: command_id.to_string(),
            source: SduiActionSource::ListItem {
                node_id: list_node_id,
                item_id: relative.clone(),
            },
            arguments: vec![
                SduiActionArgument {
                    name: "workspaceRootId".to_string(),
                    value: SduiActionValue::U64(self.root_id_hint()),
                },
                SduiActionArgument {
                    name: "relativePath".to_string(),
                    value: SduiActionValue::String(relative),
                },
            ],
        };
        SduiListItem {
            id: self.name.clone(),
            label: self.display_label(),
            detail: self.child_count.map(|count| format!("{count} items")),
            action: Some(action),
        }
    }

    fn display_label(&self) -> String {
        match self.kind {
            FileBrowserEntryKind::Directory => format!("{}/", self.name),
            _ => self.name.clone(),
        }
    }

    fn root_id_hint(&self) -> u64 {
        self.root_id
    }
}

fn parent_directory_item(
    list_node_id: SduiNodeId,
    root_id: WorkspaceRootId,
    parent: &std::path::Path,
) -> SduiListItem {
    let relative = parent.to_string_lossy().to_string();
    SduiListItem {
        id: "..".to_string(),
        label: "../".to_string(),
        detail: Some("parent".to_string()),
        action: Some(SduiActionIntent {
            command_id: OPEN_DIRECTORY_COMMAND_ID.to_string(),
            source: SduiActionSource::ListItem {
                node_id: list_node_id,
                item_id: "..".to_string(),
            },
            arguments: vec![
                SduiActionArgument {
                    name: "workspaceRootId".to_string(),
                    value: SduiActionValue::U64(root_id),
                },
                SduiActionArgument {
                    name: "relativePath".to_string(),
                    value: SduiActionValue::String(relative),
                },
            ],
        }),
    }
}

#[derive(Debug)]
pub(crate) enum FileBrowserError {
    UnknownRoot(WorkspaceRootId),
    Workspace(crate::server::workspace::WorkspaceError),
}

impl From<crate::server::workspace::WorkspaceError> for FileBrowserError {
    fn from(error: crate::server::workspace::WorkspaceError) -> Self {
        Self::Workspace(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::workspace::WorkspaceState;
    use std::fs;

    fn temp_workspace(name: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("clay-workspace-{name}-{pid}-{timestamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn file_browser_state_installs_inert_listing_snapshot() {
        let root = temp_workspace("browser-install");
        fs::write(root.join("a.txt"), "a").unwrap();
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src").join("b.rs"), "b").unwrap();

        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let browser = FileBrowserState::from_workspace(&workspace, root_id).unwrap();
        assert_eq!(browser.root_id, root_id);
        assert!(browser.entries.iter().any(|e| e.name == "a.txt"));
        assert!(browser.entries.iter().any(|e| e.name == "src"));
        assert!(
            browser
                .entries
                .iter()
                .find(|e| e.name == "src")
                .unwrap()
                .child_count
                .is_some()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_browser_sdui_tree_has_left_workspace_panel() {
        let root = temp_workspace("browser-sdui");
        fs::write(root.join("main.rs"), "fn main() {}").unwrap();

        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let browser = FileBrowserState::from_workspace(&workspace, root_id).unwrap();

        let tree = browser.to_sdui_tree(7u64, 3u64);
        assert!(
            tree.nodes
                .iter()
                .any(|node| matches!(node.kind, SduiNodeKind::Panel { .. }))
        );
        let list = tree
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                SduiNodeKind::List { items } => Some(items.clone()),
                _ => None,
            })
            .unwrap();
        assert!(list.iter().any(|item| item.label == "main.rs"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_browser_fuzzy_session_filters_locally() {
        let root = temp_workspace("browser-fuzzy");
        fs::write(root.join("alpha.rs"), "").unwrap();
        fs::write(root.join("beta.js"), "").unwrap();
        fs::write(root.join("gamma.rs"), "").unwrap();

        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let browser = FileBrowserState::from_workspace(&workspace, root_id).unwrap();

        let all = browser.fuzzy_session(TransientMenuSessionId(1), "");
        assert_eq!(all.items().len(), 3);

        let filtered = browser.fuzzy_session(TransientMenuSessionId(2), "rs");
        assert_eq!(filtered.items().len(), 2);
        assert!(
            filtered
                .items()
                .iter()
                .all(|item| item.label.contains("rs"))
        );

        let empty = browser.fuzzy_session(TransientMenuSessionId(3), "zzzz");
        assert!(empty.items().is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_browser_fuzzy_actions_carry_open_command() {
        let root = temp_workspace("browser-action");
        fs::write(root.join("doc.md"), "").unwrap();

        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let browser = FileBrowserState::from_workspace(&workspace, root_id).unwrap();

        let session = browser.fuzzy_session(TransientMenuSessionId(4), "");
        let item = session.items().first().unwrap();
        assert_eq!(item.action.command_id, OPEN_FUZZY_FILE_COMMAND_ID);
        assert_eq!(item.action.arguments["relativePath"], "doc.md");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_browser_directory_rows_navigate_instead_of_opening_files() {
        let root = temp_workspace("browser-directory-action");
        fs::create_dir(root.join("src")).unwrap();

        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let browser = FileBrowserState::from_workspace(&workspace, root_id).unwrap();
        let tree = browser.to_sdui_tree(1u64, 1u64);

        let list = tree
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                SduiNodeKind::List { items } => Some(items.clone()),
                _ => None,
            })
            .unwrap();
        let item = list.iter().find(|item| item.label == "src/").unwrap();
        let action = item.action.as_ref().unwrap();

        assert_eq!(action.command_id, OPEN_DIRECTORY_COMMAND_ID);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_browser_list_action_command_is_valid() {
        use crate::packages::commands::CommandRegistry;
        use crate::server::command_execution::{
            CommandExecutionRequest, CommandExecutionTarget, CommandExecutor,
        };

        let root = temp_workspace("browser-valid");
        fs::write(root.join("lib.rs"), "").unwrap();

        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let browser = FileBrowserState::from_workspace(&workspace, root_id).unwrap();
        let tree = browser.to_sdui_tree(1u64, 1u64);

        let list = tree
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                SduiNodeKind::List { items } => Some(items.clone()),
                _ => None,
            })
            .unwrap();
        let item = list.iter().find(|item| item.label == "lib.rs").unwrap();
        let action = item.action.as_ref().unwrap();

        let result = CommandExecutor::new().execute(
            &CommandRegistry::new(),
            CommandExecutionRequest {
                command_id: action.command_id.clone(),
                arguments: serde_json::json!({ "workspaceRootId": root_id, "relativePath": "lib.rs" }),
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            },
        );
        assert!(
            result.is_ok(),
            "expected {OPEN_FILE_COMMAND_ID} to be a valid built-in command"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn file_browser_list_action_opens_file_through_workspace_api() {
        use crate::packages::commands::CommandRegistry;
        use crate::server::command_execution::{
            CommandExecutionRequest, CommandExecutionStatus, CommandExecutionTarget,
            CommandExecutor, WorkspaceActionResult,
        };

        let root = temp_workspace("browser-open");
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let browser =
            FileBrowserState::from_workspace_at(&workspace, root_id, PathBuf::from("src")).unwrap();
        let tree = browser.to_sdui_tree(1u64, 1u64);

        let list = tree
            .nodes
            .iter()
            .find_map(|node| match &node.kind {
                SduiNodeKind::List { items } => Some(items.clone()),
                _ => None,
            })
            .unwrap();
        assert!(list.iter().any(|item| item.label == "../"));
        let item = list
            .iter()
            .find(|item| item.label == "main.rs")
            .expect("main.rs entry in src/");
        let action = item.action.as_ref().unwrap();

        let arguments = {
            let mut obj = serde_json::Map::new();
            for arg in &action.arguments {
                let value = match &arg.value {
                    crate::protocol::SduiActionValue::String(s) => {
                        serde_json::Value::String(s.clone())
                    }
                    crate::protocol::SduiActionValue::Bool(b) => serde_json::Value::Bool(*b),
                    crate::protocol::SduiActionValue::I64(v) => {
                        serde_json::Value::Number((*v).into())
                    }
                    crate::protocol::SduiActionValue::U64(v) => {
                        serde_json::Value::Number((*v).into())
                    }
                };
                obj.insert(arg.name.clone(), value);
            }
            serde_json::Value::Object(obj)
        };

        let result = CommandExecutor::new()
            .execute_workspace(
                &CommandRegistry::new(),
                &mut workspace,
                CommandExecutionRequest {
                    command_id: action.command_id.clone(),
                    arguments,
                    target: CommandExecutionTarget::Global,
                    provenance: None,
                    expected_permissions: Vec::new(),
                },
            )
            .await
            .expect("file browser list action should execute through workspace API");

        let CommandExecutionStatus::Workspace(WorkspaceActionResult::Opened(snapshot)) =
            result.status
        else {
            panic!("expected Opened workspace result, got {:?}", result.status);
        };
        assert_eq!(snapshot.text, "fn main() {}");
        assert!(snapshot.metadata.path.contains("main.rs"));

        let _ = fs::remove_dir_all(root);
    }
}
