#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 9 workspace model is defined before protocol dispatch integration"
    )
)]

use std::{
    collections::HashMap,
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
    string::FromUtf8Error,
    sync::Arc,
};

use tokio::{fs as tokio_fs, sync::Mutex};

use crate::protocol::{ClientId, DocumentAccess, DocumentId};

use super::document::DocumentState;

pub(crate) type WorkspaceRootId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceRoot {
    id: WorkspaceRootId,
    canonical_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileDocumentState {
    workspace_root_id: WorkspaceRootId,
    canonical_path: PathBuf,
    workspace_relative_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct OpenDocument {
    file_state: FileDocumentState,
    document: Arc<Mutex<DocumentState>>,
}

#[derive(Debug, Clone)]
pub(crate) struct OpenDocumentLease {
    pub(crate) document_id: DocumentId,
    pub(crate) access: DocumentAccess,
    pub(crate) file_state: FileDocumentState,
    pub(crate) document: Arc<Mutex<DocumentState>>,
}

#[derive(Debug)]
pub(crate) struct WorkspaceState {
    roots: HashMap<WorkspaceRootId, WorkspaceRoot>,
    documents: HashMap<DocumentId, OpenDocument>,
    path_to_document: HashMap<PathBuf, DocumentId>,
    next_root_id: WorkspaceRootId,
    next_document_id: DocumentId,
}

impl WorkspaceState {
    pub(crate) fn new() -> Self {
        Self {
            roots: HashMap::new(),
            documents: HashMap::new(),
            path_to_document: HashMap::new(),
            next_root_id: 1,
            next_document_id: 1,
        }
    }

    pub(crate) fn add_root(
        &mut self,
        root: impl AsRef<Path>,
    ) -> Result<WorkspaceRootId, WorkspaceError> {
        let canonical_path =
            fs::canonicalize(root.as_ref()).map_err(|source| WorkspaceError::RootUnavailable {
                path: root.as_ref().to_path_buf(),
                source,
            })?;
        let metadata =
            fs::metadata(&canonical_path).map_err(|source| WorkspaceError::RootUnavailable {
                path: canonical_path.clone(),
                source,
            })?;
        if !metadata.is_dir() {
            return Err(WorkspaceError::RootNotDirectory {
                path: canonical_path,
            });
        }

        let id = self.next_root_id;
        self.next_root_id = self.next_root_id.saturating_add(1);
        self.roots.insert(id, WorkspaceRoot { id, canonical_path });
        Ok(id)
    }

    pub(crate) async fn open_existing_file(
        &mut self,
        root_id: WorkspaceRootId,
        file_path: impl AsRef<Path>,
        client_id: ClientId,
    ) -> Result<OpenDocumentLease, WorkspaceError> {
        let file_state = self.canonical_file_state(root_id, file_path.as_ref())?;
        if let Some(existing) = self.existing_document_lease(&file_state, client_id).await {
            return Ok(existing);
        }

        let bytes = tokio_fs::read(&file_state.canonical_path)
            .await
            .map_err(|source| WorkspaceError::FileUnavailable {
                path: file_state.workspace_relative_path.clone(),
                source,
            })?;
        let text = String::from_utf8(bytes).map_err(|source| WorkspaceError::InvalidUtf8 {
            path: file_state.workspace_relative_path.clone(),
            source,
        })?;
        self.register_canonical_file(file_state, text, client_id)
            .await
    }

    pub(crate) async fn register_loaded_file(
        &mut self,
        root_id: WorkspaceRootId,
        file_path: impl AsRef<Path>,
        text: String,
        client_id: ClientId,
    ) -> Result<OpenDocumentLease, WorkspaceError> {
        let file_state = self.canonical_file_state(root_id, file_path.as_ref())?;
        if let Some(existing) = self.existing_document_lease(&file_state, client_id).await {
            return Ok(existing);
        }
        self.register_canonical_file(file_state, text, client_id)
            .await
    }

    async fn existing_document_lease(
        &self,
        file_state: &FileDocumentState,
        client_id: ClientId,
    ) -> Option<OpenDocumentLease> {
        let document_id = self
            .path_to_document
            .get(&file_state.canonical_path)
            .copied()?;
        let open_document = self
            .documents
            .get(&document_id)
            .expect("path index and document registry must stay in sync");
        let access = open_document
            .document
            .lock()
            .await
            .acquire_access(client_id);
        Some(OpenDocumentLease {
            document_id,
            access,
            file_state: open_document.file_state.clone(),
            document: Arc::clone(&open_document.document),
        })
    }

    async fn register_canonical_file(
        &mut self,
        file_state: FileDocumentState,
        text: String,
        client_id: ClientId,
    ) -> Result<OpenDocumentLease, WorkspaceError> {
        let document_id = self.next_document_id;
        self.next_document_id = self.next_document_id.saturating_add(1);
        let document = Arc::new(Mutex::new(DocumentState::new(
            document_id,
            text,
            DocumentAccess::ReadOnly,
        )));
        let access = document.lock().await.acquire_access(client_id);
        let open_document = OpenDocument {
            file_state: file_state.clone(),
            document: Arc::clone(&document),
        };
        self.path_to_document
            .insert(file_state.canonical_path.clone(), document_id);
        self.documents.insert(document_id, open_document);
        Ok(OpenDocumentLease {
            document_id,
            access,
            file_state,
            document,
        })
    }

    fn canonical_file_state(
        &self,
        root_id: WorkspaceRootId,
        file_path: &Path,
    ) -> Result<FileDocumentState, WorkspaceError> {
        let root = self
            .roots
            .get(&root_id)
            .ok_or(WorkspaceError::UnknownRoot { root_id })?;
        let joined = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            root.canonical_path.join(file_path)
        };
        let canonical_path =
            fs::canonicalize(&joined).map_err(|source| WorkspaceError::FileUnavailable {
                path: file_path.to_path_buf(),
                source,
            })?;
        if !canonical_path.starts_with(&root.canonical_path) {
            return Err(WorkspaceError::OutsideRoot);
        }
        let metadata =
            fs::metadata(&canonical_path).map_err(|source| WorkspaceError::FileUnavailable {
                path: file_path.to_path_buf(),
                source,
            })?;
        if metadata.is_dir() {
            return Err(WorkspaceError::DirectoryOpen);
        }
        if !metadata.is_file() {
            return Err(WorkspaceError::UnsupportedFileType);
        }
        let relative_path = canonical_path
            .strip_prefix(&root.canonical_path)
            .map_err(|_| WorkspaceError::OutsideRoot)?
            .to_path_buf();
        Ok(FileDocumentState {
            workspace_root_id: root_id,
            canonical_path,
            workspace_relative_path: relative_path,
        })
    }
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub(crate) enum WorkspaceError {
    UnknownRoot {
        root_id: WorkspaceRootId,
    },
    RootUnavailable {
        path: PathBuf,
        source: io::Error,
    },
    RootNotDirectory {
        path: PathBuf,
    },
    FileUnavailable {
        path: PathBuf,
        source: io::Error,
    },
    InvalidUtf8 {
        path: PathBuf,
        source: FromUtf8Error,
    },
    OutsideRoot,
    DirectoryOpen,
    UnsupportedFileType,
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRoot { root_id } => write!(formatter, "unknown workspace root {root_id}"),
            Self::RootUnavailable { path, source } => {
                write!(
                    formatter,
                    "workspace root {} is unavailable: {source}",
                    path.display()
                )
            }
            Self::RootNotDirectory { path } => {
                write!(
                    formatter,
                    "workspace root {} is not a directory",
                    path.display()
                )
            }
            Self::FileUnavailable { path, source } => {
                write!(
                    formatter,
                    "workspace file {} is unavailable: {source}",
                    path.display()
                )
            }
            Self::InvalidUtf8 { path, source } => {
                write!(
                    formatter,
                    "workspace file {} is not valid UTF-8 text: {source}",
                    path.display()
                )
            }
            Self::OutsideRoot => write!(formatter, "workspace file is outside the authorized root"),
            Self::DirectoryOpen => write!(formatter, "workspace document path is a directory"),
            Self::UnsupportedFileType => {
                write!(formatter, "workspace document path is not a regular file")
            }
        }
    }
}

impl Error for WorkspaceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RootUnavailable { source, .. } | Self::FileUnavailable { source, .. } => {
                Some(source)
            }
            Self::InvalidUtf8 { source, .. } => Some(source),
            Self::UnknownRoot { .. }
            | Self::RootNotDirectory { .. }
            | Self::OutsideRoot
            | Self::DirectoryOpen
            | Self::UnsupportedFileType => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::net::UnixListener;
    use std::{fs, path::PathBuf, time::SystemTime};

    use crate::protocol::{DocumentAccess, EditOperation, ServerMessage};

    use super::{WorkspaceError, WorkspaceState};

    fn temp_workspace(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "clay-workspace-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn duplicate_open_reuses_document_and_preserves_lease_policy() {
        let root = temp_workspace("duplicate-open");
        let file = root.join("main.rs");
        fs::write(&file, "fn main() {}\n").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let first = workspace
            .register_loaded_file(root_id, "main.rs", "fn main() {}\n".to_string(), 1)
            .await
            .unwrap();
        let second = workspace
            .register_loaded_file(root_id, &file, "ignored duplicate text".to_string(), 2)
            .await
            .unwrap();

        assert_eq!(first.document_id, second.document_id);
        assert_eq!(
            first.file_state.workspace_relative_path,
            PathBuf::from("main.rs")
        );
        assert_eq!(first.access, DocumentAccess::Editable { lease_id: 1 });
        assert_eq!(second.access, DocumentAccess::ReadOnly);
        assert!(std::sync::Arc::ptr_eq(&first.document, &second.document));

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn open_existing_file_loads_utf8_text() {
        let root = temp_workspace("open-existing");
        let file = root.join("note.txt");
        fs::write(&file, "hello 🌎\n").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let opened = workspace
            .open_existing_file(root_id, "note.txt", 11)
            .await
            .unwrap();

        assert_eq!(opened.document_id, 1);
        assert_eq!(opened.access, DocumentAccess::Editable { lease_id: 1 });
        let document = opened.document.lock().await;
        assert_eq!(
            document.initial_document_message(opened.access.clone()),
            ServerMessage::InitialDocument {
                document_id: 1,
                version: 1,
                text: "hello 🌎\n".to_string(),
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
            }
        );
        assert!(!document.is_dirty());

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn duplicate_open_reuses_loaded_document_and_lease_policy() {
        let root = temp_workspace("duplicate-open-load");
        let file = root.join("main.rs");
        fs::write(&file, "fn main() {}\n").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let first = workspace
            .open_existing_file(root_id, "main.rs", 1)
            .await
            .unwrap();
        fs::write(&file, "changed on disk after open\n").unwrap();
        let second = workspace
            .open_existing_file(root_id, &file, 2)
            .await
            .unwrap();

        assert_eq!(first.document_id, second.document_id);
        assert_eq!(first.access, DocumentAccess::Editable { lease_id: 1 });
        assert_eq!(second.access, DocumentAccess::ReadOnly);
        assert!(std::sync::Arc::ptr_eq(&first.document, &second.document));
        assert_eq!(
            second
                .document
                .lock()
                .await
                .initial_document_message(second.access.clone()),
            ServerMessage::InitialDocument {
                document_id: first.document_id,
                version: 1,
                text: "fn main() {}\n".to_string(),
                access: DocumentAccess::ReadOnly,
                lease_id: None,
            }
        );

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn open_invalid_utf8_reports_file_io_error_without_document_entry() {
        let root = temp_workspace("invalid-utf8");
        let file = root.join("bad.txt");
        fs::write(&file, [0xff, 0xfe, b'x']).unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let error = workspace
            .open_existing_file(root_id, "bad.txt", 1)
            .await
            .unwrap_err();

        assert!(matches!(error, WorkspaceError::InvalidUtf8 { .. }));
        assert!(error.to_string().contains("not valid UTF-8 text"));
        assert!(workspace.documents.is_empty());
        assert!(workspace.path_to_document.is_empty());

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn workspace_rejects_path_traversal_outside_root() {
        let parent = temp_workspace("path-traversal-parent");
        let root = parent.join("root");
        fs::create_dir(&root).unwrap();
        let outside = parent.join("outside.txt");
        fs::write(&outside, "secret").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let error = workspace
            .register_loaded_file(root_id, "../outside.txt", "secret".to_string(), 1)
            .await
            .unwrap_err();

        assert!(matches!(error, WorkspaceError::OutsideRoot));

        let _ = fs::remove_file(outside);
        let _ = fs::remove_dir(root);
        let _ = fs::remove_dir(parent);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn workspace_rejects_directory_and_special_file_open() {
        let root = temp_workspace("special-files");
        let directory = root.join("subdir");
        fs::create_dir(&directory).unwrap();
        let socket = root.join("document.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let directory_error = workspace
            .register_loaded_file(root_id, "subdir", String::new(), 1)
            .await
            .unwrap_err();
        let special_error = workspace
            .register_loaded_file(root_id, "document.sock", String::new(), 1)
            .await
            .unwrap_err();

        assert!(matches!(directory_error, WorkspaceError::DirectoryOpen));
        assert!(matches!(special_error, WorkspaceError::UnsupportedFileType));

        drop(listener);
        let _ = fs::remove_file(socket);
        let _ = fs::remove_dir(directory);
        let _ = fs::remove_dir(root);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn workspace_canonicalizes_symlink_before_authorization() {
        let parent = temp_workspace("symlink-parent");
        let root = parent.join("root");
        fs::create_dir(&root).unwrap();
        let in_root_target = root.join("actual.txt");
        fs::write(&in_root_target, "inside").unwrap();
        let in_root_link = root.join("link-inside.txt");
        std::os::unix::fs::symlink(&in_root_target, &in_root_link).unwrap();
        let outside_target = parent.join("outside.txt");
        fs::write(&outside_target, "outside").unwrap();
        let outside_link = root.join("link-outside.txt");
        std::os::unix::fs::symlink(&outside_target, &outside_link).unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let inside = workspace
            .register_loaded_file(root_id, "link-inside.txt", "inside".to_string(), 1)
            .await
            .unwrap();
        let outside_error = workspace
            .register_loaded_file(root_id, "link-outside.txt", "outside".to_string(), 2)
            .await
            .unwrap_err();

        assert_eq!(
            inside.file_state.workspace_relative_path,
            PathBuf::from("actual.txt")
        );
        assert!(matches!(outside_error, WorkspaceError::OutsideRoot));

        let _ = fs::remove_file(in_root_link);
        let _ = fs::remove_file(outside_link);
        let _ = fs::remove_file(in_root_target);
        let _ = fs::remove_file(outside_target);
        let _ = fs::remove_dir(root);
        let _ = fs::remove_dir(parent);
    }

    #[tokio::test]
    async fn file_backed_document_dirty_state_tracks_accepted_edits_and_clean_marking() {
        let root = temp_workspace("dirty-state");
        let file = root.join("note.txt");
        fs::write(&file, "hello").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .register_loaded_file(root_id, "note.txt", "hello".to_string(), 7)
            .await
            .unwrap();

        {
            let document = opened.document.lock().await;
            assert!(!document.is_dirty());
        }

        {
            let mut document = opened.document.lock().await;
            assert_eq!(
                document.apply_edit(
                    opened.document_id,
                    7,
                    Some(1),
                    1,
                    55,
                    EditOperation::Insert {
                        byte_offset: 5,
                        text: " world".to_string(),
                    },
                ),
                ServerMessage::EditAck {
                    document_id: opened.document_id,
                    confirmed_version: 2,
                    transaction_id: 55,
                }
            );
            assert!(document.is_dirty());
            document.mark_clean();
            assert!(!document.is_dirty());
        }

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }
}
