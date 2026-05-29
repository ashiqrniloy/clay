#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 9 workspace exposes internal server state-machine helpers before all UI/API callers exist"
    )
)]

use std::{
    collections::HashMap,
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
    string::FromUtf8Error,
    sync::Arc,
    time::SystemTime,
};

use tokio::{fs as tokio_fs, sync::Mutex};

use crate::protocol::{
    ClientId, DocumentAccess, DocumentId, DocumentMetadata, DocumentVersion, FileErrorCode,
};

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
    last_known_metadata: FileMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileMetadata {
    len: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SaveDocumentOutcome {
    pub(crate) document_id: DocumentId,
    pub(crate) version: DocumentVersion,
    pub(crate) dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReloadDocumentOutcome {
    pub(crate) document_id: DocumentId,
    pub(crate) version: DocumentVersion,
    pub(crate) text: String,
    pub(crate) dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceDiagnostic {
    pub(crate) code: FileErrorCode,
    pub(crate) message: String,
    pub(crate) hint: Option<String>,
}

impl WorkspaceDiagnostic {
    fn new(code: FileErrorCode, message: impl Into<String>, hint: Option<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hint,
        }
    }
}

impl fmt::Display for WorkspaceDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.hint {
            Some(hint) => write!(formatter, "{} Hint: {hint}", self.message),
            None => formatter.write_str(&self.message),
        }
    }
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

    pub(crate) fn reserve_document_ids_from(&mut self, next_document_id: DocumentId) {
        self.next_document_id = self.next_document_id.max(next_document_id);
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

    pub(crate) fn document_handle(
        &self,
        document_id: DocumentId,
    ) -> Option<Arc<Mutex<DocumentState>>> {
        self.documents
            .get(&document_id)
            .map(|open_document| Arc::clone(&open_document.document))
    }

    pub(crate) async fn document_metadata(
        &self,
        document_id: DocumentId,
        client_id: ClientId,
    ) -> Result<DocumentMetadata, WorkspaceError> {
        let open_document = self
            .documents
            .get(&document_id)
            .ok_or(WorkspaceError::UnknownDocument { document_id })?;
        metadata_for_open_document(document_id, open_document, client_id).await
    }

    pub(crate) async fn list_documents(
        &self,
        client_id: ClientId,
    ) -> Result<Vec<DocumentMetadata>, WorkspaceError> {
        let mut entries = Vec::with_capacity(self.documents.len());
        for (&document_id, open_document) in &self.documents {
            entries.push(metadata_for_open_document(document_id, open_document, client_id).await?);
        }
        entries.sort_by_key(|metadata| metadata.document_id);
        Ok(entries)
    }

    pub(crate) async fn release_client_access(&self, client_id: ClientId) {
        for open_document in self.documents.values() {
            open_document
                .document
                .lock()
                .await
                .release_access(client_id);
        }
    }

    pub(crate) async fn save_document(
        &mut self,
        document_id: DocumentId,
    ) -> Result<SaveDocumentOutcome, WorkspaceError> {
        let (canonical_path, relative_path, expected_metadata, document) = {
            let open_document = self
                .documents
                .get(&document_id)
                .ok_or(WorkspaceError::UnknownDocument { document_id })?;
            (
                open_document.file_state.canonical_path.clone(),
                open_document.file_state.workspace_relative_path.clone(),
                open_document.file_state.last_known_metadata.clone(),
                Arc::clone(&open_document.document),
            )
        };
        let (version, text) = {
            let document = document.lock().await;
            (document.version(), document.text())
        };

        let current_metadata = self.reauthorize_open_file(document_id)?;
        if current_metadata != expected_metadata {
            return Err(WorkspaceError::StaleFileMetadata {
                path: relative_path,
            });
        }

        tokio_fs::write(&canonical_path, text.as_bytes())
            .await
            .map_err(|source| WorkspaceError::WriteFailed {
                path: relative_path.clone(),
                source,
            })?;
        let saved_metadata = tokio_fs::metadata(&canonical_path)
            .await
            .map_err(|source| WorkspaceError::FileUnavailable {
                path: relative_path.clone(),
                source,
            })?;
        let saved_metadata = FileMetadata::from_fs_metadata(&saved_metadata);
        if let Some(open_document) = self.documents.get_mut(&document_id) {
            open_document.file_state.last_known_metadata = saved_metadata;
        }

        let dirty = {
            let mut document = document.lock().await;
            !document.mark_clean_if_version(version)
        };
        Ok(SaveDocumentOutcome {
            document_id,
            version,
            dirty,
        })
    }

    pub(crate) async fn reload_document(
        &mut self,
        document_id: DocumentId,
        force: bool,
    ) -> Result<ReloadDocumentOutcome, WorkspaceError> {
        let (canonical_path, relative_path, document) = {
            let open_document = self
                .documents
                .get(&document_id)
                .ok_or(WorkspaceError::UnknownDocument { document_id })?;
            (
                open_document.file_state.canonical_path.clone(),
                open_document.file_state.workspace_relative_path.clone(),
                Arc::clone(&open_document.document),
            )
        };
        if document.lock().await.is_dirty() && !force {
            return Err(WorkspaceError::DirtyDocument { document_id });
        }

        self.reauthorize_open_file(document_id)?;
        let bytes = tokio_fs::read(&canonical_path).await.map_err(|source| {
            WorkspaceError::FileUnavailable {
                path: relative_path.clone(),
                source,
            }
        })?;
        let text = String::from_utf8(bytes).map_err(|source| WorkspaceError::InvalidUtf8 {
            path: relative_path.clone(),
            source,
        })?;
        let reloaded_metadata = tokio_fs::metadata(&canonical_path)
            .await
            .map_err(|source| WorkspaceError::FileUnavailable {
                path: relative_path.clone(),
                source,
            })?;
        let reloaded_metadata = FileMetadata::from_fs_metadata(&reloaded_metadata);

        let version = {
            let mut document = document.lock().await;
            document.replace_text_from_storage(text.clone());
            document.version()
        };
        if let Some(open_document) = self.documents.get_mut(&document_id) {
            open_document.file_state.last_known_metadata = reloaded_metadata;
        }
        Ok(ReloadDocumentOutcome {
            document_id,
            version,
            text,
            dirty: false,
        })
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
        validate_regular_file_metadata(&metadata)?;
        let relative_path = canonical_path
            .strip_prefix(&root.canonical_path)
            .map_err(|_| WorkspaceError::OutsideRoot)?
            .to_path_buf();
        Ok(FileDocumentState {
            workspace_root_id: root_id,
            canonical_path,
            workspace_relative_path: relative_path,
            last_known_metadata: FileMetadata::from_fs_metadata(&metadata),
        })
    }

    fn reauthorize_open_file(
        &self,
        document_id: DocumentId,
    ) -> Result<FileMetadata, WorkspaceError> {
        let open_document = self
            .documents
            .get(&document_id)
            .ok_or(WorkspaceError::UnknownDocument { document_id })?;
        let root = self
            .roots
            .get(&open_document.file_state.workspace_root_id)
            .ok_or(WorkspaceError::UnknownRoot {
                root_id: open_document.file_state.workspace_root_id,
            })?;
        let canonical_path =
            fs::canonicalize(&open_document.file_state.canonical_path).map_err(|source| {
                WorkspaceError::FileUnavailable {
                    path: open_document.file_state.workspace_relative_path.clone(),
                    source,
                }
            })?;
        if !canonical_path.starts_with(&root.canonical_path) {
            return Err(WorkspaceError::OutsideRoot);
        }
        let metadata =
            fs::metadata(&canonical_path).map_err(|source| WorkspaceError::FileUnavailable {
                path: open_document.file_state.workspace_relative_path.clone(),
                source,
            })?;
        validate_regular_file_metadata(&metadata)?;
        Ok(FileMetadata::from_fs_metadata(&metadata))
    }
}

impl FileDocumentState {
    pub(crate) fn display_path(&self) -> String {
        self.workspace_relative_path
            .to_string_lossy()
            .replace('\\', "/")
    }
}

async fn metadata_for_open_document(
    document_id: DocumentId,
    open_document: &OpenDocument,
    client_id: ClientId,
) -> Result<DocumentMetadata, WorkspaceError> {
    let document = open_document.document.lock().await;
    let access = document.access_for_client(client_id);
    Ok(DocumentMetadata {
        document_id,
        version: document.version(),
        lease_id: access.lease_id(),
        access,
        dirty: document.is_dirty(),
        workspace_root_id: open_document.file_state.workspace_root_id,
        path: open_document.file_state.display_path(),
    })
}

impl FileMetadata {
    fn from_fs_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }
}

fn validate_regular_file_metadata(metadata: &fs::Metadata) -> Result<(), WorkspaceError> {
    if metadata.is_dir() {
        return Err(WorkspaceError::DirectoryOpen);
    }
    if !metadata.is_file() {
        return Err(WorkspaceError::UnsupportedFileType);
    }
    Ok(())
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
    UnknownDocument {
        document_id: DocumentId,
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
    WriteFailed {
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
    DirtyDocument {
        document_id: DocumentId,
    },
    StaleFileMetadata {
        path: PathBuf,
    },
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.diagnostic().fmt(formatter)
    }
}

impl WorkspaceError {
    pub(crate) fn diagnostic(&self) -> WorkspaceDiagnostic {
        match self {
            Self::UnknownRoot { root_id } => WorkspaceDiagnostic::new(
                FileErrorCode::UnknownWorkspaceRoot,
                format!("unknown workspace root {root_id}"),
                Some("Use a workspace root id advertised by the Clay server.".to_string()),
            ),
            Self::UnknownDocument { document_id } => WorkspaceDiagnostic::new(
                FileErrorCode::UnknownDocument,
                format!("unknown workspace document {document_id}"),
                Some("Open the document through the server before saving, reloading, or querying it.".to_string()),
            ),
            Self::RootUnavailable { path, source } => {
                let code = io_error_code(source);
                let display_path = display_authorized_path(path);
                match source.kind() {
                    io::ErrorKind::NotFound => WorkspaceDiagnostic::new(
                        code,
                        format!("workspace root {display_path} is missing or is not visible to the Clay server process: {source}"),
                        Some(container_mount_hint()),
                    ),
                    io::ErrorKind::PermissionDenied => WorkspaceDiagnostic::new(
                        code,
                        format!("workspace root {display_path} cannot be accessed by the Clay server process because permission was denied: {source}"),
                        Some(container_permission_hint()),
                    ),
                    _ => WorkspaceDiagnostic::new(
                        code,
                        format!("workspace root {display_path} is unavailable to the Clay server process: {source}"),
                        Some(container_mount_hint()),
                    ),
                }
            }
            Self::RootNotDirectory { path } => WorkspaceDiagnostic::new(
                FileErrorCode::DirectoryOpen,
                format!("workspace root {} is not a directory", display_authorized_path(path)),
                Some("Choose a directory that is visible inside the Clay server environment.".to_string()),
            ),
            Self::FileUnavailable { path, source } => {
                let code = io_error_code(source);
                let display_path = display_workspace_path(path);
                match source.kind() {
                    io::ErrorKind::NotFound => WorkspaceDiagnostic::new(
                        code,
                        format!("workspace file {display_path} was not found or is not visible to the Clay server process: {source}"),
                        Some(container_mount_hint()),
                    ),
                    io::ErrorKind::PermissionDenied => WorkspaceDiagnostic::new(
                        code,
                        format!("permission denied while accessing workspace file {display_path} from the Clay server process: {source}"),
                        Some(container_permission_hint()),
                    ),
                    _ => WorkspaceDiagnostic::new(
                        code,
                        format!("workspace file {display_path} is unavailable to the Clay server process: {source}"),
                        Some(container_mount_hint()),
                    ),
                }
            }
            Self::WriteFailed { path, source } => {
                let code = io_error_code(source);
                let display_path = display_workspace_path(path);
                match source.kind() {
                    io::ErrorKind::PermissionDenied => WorkspaceDiagnostic::new(
                        code,
                        format!("permission denied while saving workspace file {display_path} from the Clay server process: {source}"),
                        Some(container_permission_hint()),
                    ),
                    _ => WorkspaceDiagnostic::new(
                        code,
                        format!("failed to save workspace file {display_path} from the Clay server process: {source}"),
                        Some("Check that the file still exists, is writable, and is mounted read-write in the server environment.".to_string()),
                    ),
                }
            }
            Self::InvalidUtf8 { path, source } => WorkspaceDiagnostic::new(
                FileErrorCode::InvalidUtf8,
                format!("workspace file {} is not valid UTF-8 text: {source}", display_workspace_path(path)),
                Some("Open only UTF-8 text files through Phase 9 workspace documents.".to_string()),
            ),
            Self::OutsideRoot => WorkspaceDiagnostic::new(
                FileErrorCode::OutsideRoot,
                "workspace path is outside the authorized root".to_string(),
                Some("Choose a path inside a configured workspace root visible to the Clay server; unauthorized host paths are not disclosed.".to_string()),
            ),
            Self::DirectoryOpen => WorkspaceDiagnostic::new(
                FileErrorCode::DirectoryOpen,
                "workspace document path is a directory".to_string(),
                Some("Open a regular UTF-8 text file; directory listing is not a Phase 9 document open operation.".to_string()),
            ),
            Self::UnsupportedFileType => WorkspaceDiagnostic::new(
                FileErrorCode::UnsupportedFileType,
                "workspace document path is not a regular file".to_string(),
                Some("Sockets, devices, FIFOs, and other special files are not opened as Clay documents.".to_string()),
            ),
            Self::DirtyDocument { document_id } => WorkspaceDiagnostic::new(
                FileErrorCode::DirtyDocument,
                format!("workspace document {document_id} has unsaved edits"),
                Some("Save the document or explicitly request a forced reload before replacing in-memory edits.".to_string()),
            ),
            Self::StaleFileMetadata { path } => WorkspaceDiagnostic::new(
                FileErrorCode::StaleFileMetadata,
                format!("workspace file {} changed on disk since it was loaded", display_workspace_path(path)),
                Some("Reload or resolve the external change before saving to avoid overwriting data.".to_string()),
            ),
        }
    }
}

fn io_error_code(error: &io::Error) -> FileErrorCode {
    match error.kind() {
        io::ErrorKind::NotFound => FileErrorCode::NotFound,
        io::ErrorKind::PermissionDenied => FileErrorCode::PermissionDenied,
        _ => FileErrorCode::AccessDenied,
    }
}

fn display_authorized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn display_workspace_path(path: &Path) -> String {
    if path.is_absolute() {
        "<requested path>".to_string()
    } else {
        path.to_string_lossy().replace('\\', "/")
    }
}

fn container_mount_hint() -> String {
    "If the Clay server runs in toolbox/distrobox or another container, mount the workspace or choose a root that exists inside that environment.".to_string()
}

fn container_permission_hint() -> String {
    "If the Clay server runs in toolbox/distrobox or another container, verify the mounted workspace is readable/writable by that server process and not mounted read-only.".to_string()
}

impl Error for WorkspaceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RootUnavailable { source, .. }
            | Self::FileUnavailable { source, .. }
            | Self::WriteFailed { source, .. } => Some(source),
            Self::InvalidUtf8 { source, .. } => Some(source),
            Self::UnknownRoot { .. }
            | Self::UnknownDocument { .. }
            | Self::RootNotDirectory { .. }
            | Self::OutsideRoot
            | Self::DirectoryOpen
            | Self::UnsupportedFileType
            | Self::DirtyDocument { .. }
            | Self::StaleFileMetadata { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};
    use std::{fs, path::PathBuf, time::SystemTime};

    use crate::protocol::{DocumentAccess, EditOperation, FileErrorCode, ServerMessage};

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

    #[tokio::test]
    async fn accepted_edit_marks_file_document_dirty_and_save_marks_clean() {
        let root = temp_workspace("save-cleans");
        let file = root.join("note.txt");
        fs::write(&file, "hello").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .open_existing_file(root_id, "note.txt", 7)
            .await
            .unwrap();

        {
            let mut document = opened.document.lock().await;
            assert_eq!(
                document.apply_edit(
                    opened.document_id,
                    7,
                    Some(1),
                    1,
                    56,
                    EditOperation::Insert {
                        byte_offset: 5,
                        text: " world".to_string(),
                    },
                ),
                ServerMessage::EditAck {
                    document_id: opened.document_id,
                    confirmed_version: 2,
                    transaction_id: 56,
                }
            );
            assert!(document.is_dirty());
        }

        let saved = workspace.save_document(opened.document_id).await.unwrap();

        assert_eq!(saved.document_id, opened.document_id);
        assert_eq!(saved.version, 2);
        assert!(!saved.dirty);
        assert_eq!(fs::read_to_string(&file).unwrap(), "hello world");
        assert!(!opened.document.lock().await.is_dirty());

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn save_writes_canonical_rope_text_to_disk() {
        let root = temp_workspace("save-text");
        let file = root.join("note.txt");
        fs::write(&file, "abc").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .open_existing_file(root_id, "note.txt", 9)
            .await
            .unwrap();

        {
            let mut document = opened.document.lock().await;
            let response = document.apply_edit(
                opened.document_id,
                9,
                Some(1),
                1,
                57,
                EditOperation::Replace {
                    start: 1,
                    end: 2,
                    text: "é".to_string(),
                },
            );
            assert!(matches!(response, ServerMessage::EditAck { .. }));
        }

        workspace.save_document(opened.document_id).await.unwrap();

        assert_eq!(fs::read_to_string(&file).unwrap(), "aéc");

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn reload_dirty_document_requires_force_or_rejects() {
        let root = temp_workspace("reload-dirty");
        let file = root.join("note.txt");
        fs::write(&file, "disk").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .open_existing_file(root_id, "note.txt", 3)
            .await
            .unwrap();
        {
            let mut document = opened.document.lock().await;
            let response = document.apply_edit(
                opened.document_id,
                3,
                Some(1),
                1,
                58,
                EditOperation::Insert {
                    byte_offset: 4,
                    text: " dirty".to_string(),
                },
            );
            assert!(matches!(response, ServerMessage::EditAck { .. }));
        }
        fs::write(&file, "changed on disk").unwrap();

        let rejected = workspace
            .reload_document(opened.document_id, false)
            .await
            .unwrap_err();
        assert!(matches!(rejected, WorkspaceError::DirtyDocument { .. }));
        assert_eq!(opened.document.lock().await.text(), "disk dirty");

        let reloaded = workspace
            .reload_document(opened.document_id, true)
            .await
            .unwrap();

        assert_eq!(reloaded.text, "changed on disk");
        assert!(!opened.document.lock().await.is_dirty());

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn reload_clean_document_refreshes_disk_text_and_marks_clean() {
        let root = temp_workspace("reload-clean");
        let file = root.join("note.txt");
        fs::write(&file, "old").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .open_existing_file(root_id, "note.txt", 4)
            .await
            .unwrap();
        fs::write(&file, "new text").unwrap();

        let reloaded = workspace
            .reload_document(opened.document_id, false)
            .await
            .unwrap();

        assert_eq!(reloaded.document_id, opened.document_id);
        assert_eq!(reloaded.text, "new text");
        assert_eq!(opened.document.lock().await.text(), "new text");
        assert!(!opened.document.lock().await.is_dirty());

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn save_missing_file_returns_typed_error_and_keeps_dirty() {
        let root = temp_workspace("save-missing");
        let file = root.join("note.txt");
        fs::write(&file, "hello").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .open_existing_file(root_id, "note.txt", 5)
            .await
            .unwrap();
        {
            let mut document = opened.document.lock().await;
            let response = document.apply_edit(
                opened.document_id,
                5,
                Some(1),
                1,
                59,
                EditOperation::Insert {
                    byte_offset: 5,
                    text: "!".to_string(),
                },
            );
            assert!(matches!(response, ServerMessage::EditAck { .. }));
        }
        fs::remove_file(&file).unwrap();

        let error = workspace
            .save_document(opened.document_id)
            .await
            .unwrap_err();

        assert!(matches!(error, WorkspaceError::FileUnavailable { .. }));
        assert!(opened.document.lock().await.is_dirty());

        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn save_stale_metadata_returns_typed_error_and_keeps_dirty() {
        let root = temp_workspace("save-stale");
        let file = root.join("note.txt");
        fs::write(&file, "hello").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .open_existing_file(root_id, "note.txt", 6)
            .await
            .unwrap();
        {
            let mut document = opened.document.lock().await;
            let response = document.apply_edit(
                opened.document_id,
                6,
                Some(1),
                1,
                60,
                EditOperation::Insert {
                    byte_offset: 5,
                    text: " server".to_string(),
                },
            );
            assert!(matches!(response, ServerMessage::EditAck { .. }));
        }
        fs::write(&file, "external change with different length").unwrap();

        let error = workspace
            .save_document(opened.document_id)
            .await
            .unwrap_err();

        assert!(matches!(error, WorkspaceError::StaleFileMetadata { .. }));
        assert!(opened.document.lock().await.is_dirty());
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "external change with different length"
        );

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn workspace_diagnostic_for_missing_root_is_actionable() {
        let root = temp_workspace("missing-root-parent");
        let missing_root = root.join("missing");
        let mut workspace = WorkspaceState::new();

        let error = workspace.add_root(&missing_root).unwrap_err();
        let diagnostic = error.diagnostic();

        assert_eq!(diagnostic.code, FileErrorCode::NotFound);
        assert!(diagnostic.message.contains("workspace root"));
        assert!(diagnostic.message.contains("missing or is not visible"));
        assert!(
            diagnostic
                .hint
                .as_deref()
                .unwrap()
                .contains("toolbox/distrobox")
        );

        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn workspace_diagnostic_sanitizes_unauthorized_paths() {
        let parent = temp_workspace("diagnostic-sanitize-parent");
        let root = parent.join("root");
        fs::create_dir(&root).unwrap();
        let outside = parent.join("outside.txt");
        fs::write(&outside, "secret").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let error = workspace
            .open_existing_file(root_id, &outside, 1)
            .await
            .unwrap_err();
        let diagnostic = error.diagnostic();
        let rendered = diagnostic.to_string();

        assert_eq!(diagnostic.code, FileErrorCode::OutsideRoot);
        assert!(!rendered.contains(outside.to_string_lossy().as_ref()));
        assert!(rendered.contains("outside the authorized root"));

        let _ = fs::remove_file(outside);
        let _ = fs::remove_dir(root);
        let _ = fs::remove_dir(parent);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn workspace_permission_denied_keeps_document_dirty() {
        let root = temp_workspace("permission-denied");
        let file = root.join("note.txt");
        fs::write(&file, "hello").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .open_existing_file(root_id, "note.txt", 8)
            .await
            .unwrap();
        {
            let mut document = opened.document.lock().await;
            let response = document.apply_edit(
                opened.document_id,
                8,
                Some(1),
                1,
                61,
                EditOperation::Insert {
                    byte_offset: 5,
                    text: "!".to_string(),
                },
            );
            assert!(matches!(response, ServerMessage::EditAck { .. }));
        }

        let mut permissions = fs::metadata(&file).unwrap().permissions();
        let original_mode = permissions.mode();
        permissions.set_mode(0o444);
        fs::set_permissions(&file, permissions).unwrap();

        let error = workspace
            .save_document(opened.document_id)
            .await
            .unwrap_err();
        let diagnostic = error.diagnostic();

        assert_eq!(diagnostic.code, FileErrorCode::PermissionDenied);
        assert!(diagnostic.to_string().contains("permission denied"));
        assert!(opened.document.lock().await.is_dirty());

        let mut permissions = fs::metadata(&file).unwrap().permissions();
        permissions.set_mode(original_mode);
        fs::set_permissions(&file, permissions).unwrap();
        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }
}
