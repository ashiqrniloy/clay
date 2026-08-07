#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 9 workspace exposes internal server state-machine helpers before all UI/API callers exist"
    )
)]

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
    string::FromUtf8Error,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::SystemTime,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use tokio::{
    fs as tokio_fs,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};

use crate::perf::budgets::{
    MAX_AUXILIARY_READ_BYTES, MAX_DOCUMENTS_PER_CLIENT, MAX_GITIGNORE_LINES,
    MAX_GITIGNORE_PATTERN_CHARS, MAX_GITIGNORE_PATTERNS, MAX_OPENABLE_FILE_BYTES,
    MAX_SERVER_DOCUMENTS,
};
use crate::protocol::{
    ClientId, DocumentAccess, DocumentId, DocumentMetadata, DocumentVersion, FileErrorCode,
};

use super::document::DocumentState;

pub(crate) type WorkspaceRootId = u64;

/// Closed set of project marker files/directories used for workspace-root
/// discovery. These are checked by presence/metadata only and are never
/// executed or parsed for arbitrary content.
pub(crate) const KNOWN_PROJECT_MARKERS: &[&str] = &[".git", "Cargo.toml", "package.json"];

/// Maximum number of ancestors to walk when discovering a workspace root from
/// an opened file path. Bounded to avoid expensive traversal on deep paths.
const MAX_DISCOVERY_ANCESTRY_DEPTH: usize = 32;

/// Maximum number of workspace roots (directory + single-file grants) the
/// server will hold. Bounded to keep root metadata and authority checks cheap.
const MAX_WORKSPACE_ROOTS: usize = 64;

/// Default names ignored by the file listing service. These are directories
/// or files that are commonly large, generated, or repository-internal and
/// are not useful to show in a general-purpose file browser. Packages cannot
/// extend this set.
pub(crate) const DEFAULT_IGNORED_NAMES: &[&str] = &[".git", "node_modules", "target"];

/// Maximum directory depth for a single listing request. Bounded to prevent
/// deep recursion and keep response sizes predictable.
const MAX_LIST_DIRECTORY_DEPTH: usize = 8;

/// Maximum number of entries returned by a single listing request. Bounded to
/// keep response serialization and UI rendering cheap.
const MAX_LIST_DIRECTORY_ENTRIES: usize = 1000;

/// Maximum number of immediate children to scan when computing
/// `child_count` for a directory entry. Larger directories report the cap.
const MAX_CHILD_COUNT_SCAN: usize = 100;

/// Cancellation token for in-flight directory listings. The bool is set to
/// `true` by `op_clay_workspace_cancel_listing` to request early termination.
pub(crate) type ListingCancelToken = Arc<AtomicBool>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceRoot {
    id: WorkspaceRootId,
    authority: WorkspaceAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WorkspaceAuthority {
    Directory { canonical_path: PathBuf },
    SingleFile { canonical_path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceRootMetadata {
    pub(crate) workspace_root_id: WorkspaceRootId,
    pub(crate) display_name: String,
    pub(crate) display_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceDirectoryRoot {
    pub(crate) workspace_root_id: WorkspaceRootId,
    pub(crate) canonical_path: PathBuf,
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
pub(crate) struct CloseDocumentOutcome {
    pub(crate) document_id: DocumentId,
    pub(crate) version: DocumentVersion,
    /// True when this close removed the last access holder and the registry
    /// entry; the caller must tear down document-scoped coordinator state.
    pub(crate) closed: bool,
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

/// Kind of a file-system entry returned by the bounded listing service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileListEntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

/// Per-entry diagnostic reported by the bounded listing service. A single
/// unreadable directory does not fail the whole request; it is reported as
/// an entry-level diagnostic and listing continues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileListEntryDiagnostic {
    pub(crate) code: FileErrorCode,
    pub(crate) message: String,
}

/// One entry in a bounded directory listing result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileListEntry {
    pub(crate) name: String,
    pub(crate) kind: FileListEntryKind,
    pub(crate) relative_path: PathBuf,
    pub(crate) size_hint: Option<u64>,
    pub(crate) child_count: Option<usize>,
    pub(crate) diagnostic: Option<FileListEntryDiagnostic>,
}

/// Result page returned by the bounded directory listing service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileListPage {
    pub(crate) root_id: WorkspaceRootId,
    pub(crate) entries: Vec<FileListEntry>,
    pub(crate) truncated: bool,
    pub(crate) cancelled: bool,
    pub(crate) diagnostics: Vec<WorkspaceDiagnostic>,
}

/// Request for the bounded directory listing service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileListRequest {
    pub(crate) root_id: WorkspaceRootId,
    pub(crate) relative_path: PathBuf,
    pub(crate) max_depth: usize,
    pub(crate) max_entries: usize,
}

impl Default for FileListRequest {
    fn default() -> Self {
        Self {
            root_id: 0,
            relative_path: PathBuf::new(),
            max_depth: MAX_LIST_DIRECTORY_DEPTH,
            max_entries: MAX_LIST_DIRECTORY_ENTRIES,
        }
    }
}

/// Authority snapshot for one directory traversal. Created while holding the
/// workspace lock, then moved to `spawn_blocking`; no workspace state is read
/// during filesystem traversal.
#[derive(Debug)]
pub(crate) struct DirectoryListingPlan {
    root_id: WorkspaceRootId,
    root_path: PathBuf,
    relative_path: PathBuf,
    max_depth: usize,
    max_entries: usize,
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

impl OpenDocumentLease {
    pub(crate) async fn snapshot(&self, _client_id: ClientId) -> OpenDocumentSnapshot {
        let document = self.document.lock().await;
        let metadata = DocumentMetadata {
            document_id: self.document_id,
            version: document.version(),
            access: self.access.clone(),
            lease_id: self.access.lease_id(),
            dirty: document.is_dirty(),
            workspace_root_id: self.file_state.workspace_root_id,
            path: self
                .file_state
                .workspace_relative_path
                .to_string_lossy()
                .to_string(),
        };
        let text = document.text();
        OpenDocumentSnapshot { metadata, text }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenDocumentSnapshot {
    pub(crate) metadata: DocumentMetadata,
    pub(crate) text: String,
}

/// Prepare/commit split for workspace file I/O.
///
/// Each disk-bearing workspace op (`open_existing_file`, `open_selected_file`,
/// `save_document`, `reload_document`) is broken into a `prepare_*` phase that
/// runs under the workspace mutex (fast filesystem metadata + authority checks
/// and state lookup only), a free `*_io` phase that performs the heavy
/// `tokio::fs` read/write with **no** workspace mutex held, and a `commit_*`
/// phase that reacquires the workspace mutex to mutate the open-document
/// registry. The IpcServer and runtime-op callers use the `*_unlocked`
/// orchestration helpers so concurrent operations on unrelated documents are no
/// longer serialized by a slow disk call; the `&mut self` one-shot methods are
/// thin wrappers used by tests/direct callers (where there is no outer mutex to
/// release).
///
/// Because the mutex is released across disk I/O, every `commit_*` re-validates
/// registry state on reacquire (e.g. an open may find the file was registered by
/// a concurrent open, a reload re-checks dirtiness, a save tolerates a closed
/// document) instead of assuming the registry is unchanged.
struct OpenPlan {
    file_state: FileDocumentState,
    client_id: ClientId,
}

enum OpenPrepare {
    Existing(OpenDocumentLease),
    New(OpenPlan),
}

/// Pieces needed to open a user-selected file; the `SingleFile` root/grant is
/// allocated in `commit` (under the workspace mutex) rather than `prepare`, so
/// a concurrent open that wins the registry race does not leave an orphan root.
struct SelectedOpenPlan {
    canonical_path: PathBuf,
    display_path: PathBuf,
    client_id: ClientId,
}

enum SelectedOpenPrepare {
    Existing(OpenDocumentLease),
    New(SelectedOpenPlan),
}

struct SavePlan {
    document_id: DocumentId,
    canonical_path: PathBuf,
    relative_path: PathBuf,
    document: Arc<Mutex<DocumentState>>,
    /// Stable target identity observed at prepare time; revalidated
    /// immediately before the atomic replace so an external change during the
    /// write fails the save instead of being clobbered.
    expected_identity: TargetIdentity,
}

struct SaveIoOutcome {
    prepared_version: DocumentVersion,
    saved_metadata: FileMetadata,
}

struct ReloadPlan {
    document_id: DocumentId,
    canonical_path: PathBuf,
    relative_path: PathBuf,
    document: Arc<Mutex<DocumentState>>,
    force: bool,
}

struct ReloadIoOutcome {
    text: String,
    reloaded_metadata: FileMetadata,
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

        // Deduplicate directory roots by canonical path.
        if let Some(existing) = self.roots.values().find(|root| {
            matches!(
                &root.authority,
                WorkspaceAuthority::Directory { canonical_path: path } if path == &canonical_path
            )
        }) {
            return Ok(existing.id);
        }

        if self.roots.len() >= MAX_WORKSPACE_ROOTS {
            return Err(WorkspaceError::RootLimitExceeded);
        }

        let id = self.next_root_id;
        self.next_root_id = self.next_root_id.saturating_add(1);
        self.roots.insert(
            id,
            WorkspaceRoot {
                id,
                authority: WorkspaceAuthority::Directory { canonical_path },
            },
        );
        Ok(id)
    }

    pub(crate) fn list_root_metadata(&self) -> Vec<WorkspaceRootMetadata> {
        let mut roots = self
            .directory_roots()
            .into_iter()
            .map(|root| WorkspaceRootMetadata {
                workspace_root_id: root.workspace_root_id,
                display_name: root
                    .canonical_path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| root.canonical_path.to_string_lossy().into_owned()),
                display_path: display_authorized_path(&root.canonical_path),
            })
            .collect::<Vec<_>>();
        roots.sort_by_key(|root| root.workspace_root_id);
        roots
    }

    pub(crate) fn directory_roots(&self) -> Vec<WorkspaceDirectoryRoot> {
        let mut roots = self
            .roots
            .values()
            .filter_map(|root| {
                let WorkspaceAuthority::Directory { canonical_path } = &root.authority else {
                    return None;
                };
                Some(WorkspaceDirectoryRoot {
                    workspace_root_id: root.id,
                    canonical_path: canonical_path.clone(),
                })
            })
            .collect::<Vec<_>>();
        roots.sort_by_key(|root| root.workspace_root_id);
        roots
    }

    /// Add the current working directory as a workspace root when no explicit
    /// roots have been configured. This is the startup cwd/CLI fallback.
    pub(crate) fn add_root_from_cwd(&mut self) -> Result<Option<WorkspaceRootId>, WorkspaceError> {
        if !self.roots.is_empty() {
            return Ok(None);
        }
        let cwd = std::env::current_dir().map_err(|source| WorkspaceError::RootUnavailable {
            path: PathBuf::from("."),
            source,
        })?;
        self.add_root(&cwd).map(Some)
    }

    /// Discover a workspace root for an opened file. If the file is already
    /// covered by an existing directory root, return that root. Otherwise walk
    /// up the file's ancestry looking for a known project marker. If a marker
    /// is found, add that directory as a root. If no marker is found within the
    /// bounded depth, return `None`; the caller should fall back to a
    /// single-file selected-file grant.
    pub(crate) fn discover_root_for_path(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<Option<WorkspaceRootId>, WorkspaceError> {
        let canonical_path =
            fs::canonicalize(path.as_ref()).map_err(|source| WorkspaceError::FileUnavailable {
                path: path.as_ref().to_path_buf(),
                source,
            })?;
        let metadata =
            fs::metadata(&canonical_path).map_err(|source| WorkspaceError::FileUnavailable {
                path: path.as_ref().to_path_buf(),
                source,
            })?;
        if !metadata.is_file() {
            return Err(WorkspaceError::DirectoryOpen);
        }

        if let Some(root_id) = self.find_covering_directory_root(&canonical_path) {
            return Ok(Some(root_id));
        }

        let mut current = canonical_path.parent();
        let mut depth = 0;
        while let Some(dir) = current {
            if depth >= MAX_DISCOVERY_ANCESTRY_DEPTH {
                break;
            }
            for marker in KNOWN_PROJECT_MARKERS {
                if dir.join(marker).exists() {
                    return self.add_root(dir).map(Some);
                }
            }
            current = dir.parent();
            depth += 1;
        }

        Ok(None)
    }

    /// Add an explicit user grant as a workspace root. Directories become
    /// directory roots; files become single-file grants. Deduplicated by
    /// canonical path.
    pub(crate) fn add_explicit_user_grant(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<WorkspaceRootId, WorkspaceError> {
        let canonical_path =
            fs::canonicalize(path.as_ref()).map_err(|source| WorkspaceError::RootUnavailable {
                path: path.as_ref().to_path_buf(),
                source,
            })?;
        let metadata =
            fs::metadata(&canonical_path).map_err(|source| WorkspaceError::RootUnavailable {
                path: path.as_ref().to_path_buf(),
                source,
            })?;

        if metadata.is_dir() {
            return self.add_root(canonical_path);
        }

        if metadata.is_file() {
            // Deduplicate single-file grants by canonical path.
            if let Some(existing) = self.roots.values().find(|root| {
                matches!(
                    &root.authority,
                    WorkspaceAuthority::SingleFile { canonical_path: path } if path == &canonical_path
                )
            }) {
                return Ok(existing.id);
            }
            if self.roots.len() >= MAX_WORKSPACE_ROOTS {
                return Err(WorkspaceError::RootLimitExceeded);
            }
            return self.add_single_file_grant(canonical_path);
        }

        Err(WorkspaceError::UnsupportedFileType)
    }

    fn find_covering_directory_root(&self, canonical_path: &Path) -> Option<WorkspaceRootId> {
        self.roots.values().find_map(|root| {
            let WorkspaceAuthority::Directory {
                canonical_path: root_path,
            } = &root.authority
            else {
                return None;
            };
            if canonical_path.starts_with(root_path) {
                Some(root.id)
            } else {
                None
            }
        })
    }

    /// Snapshot directory authority and request ceilings without traversing
    /// the filesystem. Callers holding `Arc<Mutex<WorkspaceState>>` release
    /// that lock before executing the returned plan.
    pub(crate) fn prepare_directory_listing(
        &self,
        request: FileListRequest,
    ) -> Result<DirectoryListingPlan, WorkspaceError> {
        let root = self
            .roots
            .get(&request.root_id)
            .ok_or(WorkspaceError::UnknownRoot {
                root_id: request.root_id,
            })?;
        let WorkspaceAuthority::Directory {
            canonical_path: root_path,
        } = &root.authority
        else {
            return Err(WorkspaceError::DirectoryOpen);
        };
        Ok(DirectoryListingPlan {
            root_id: request.root_id,
            root_path: root_path.clone(),
            relative_path: request.relative_path,
            max_depth: request.max_depth.min(MAX_LIST_DIRECTORY_DEPTH),
            max_entries: request.max_entries.min(MAX_LIST_DIRECTORY_ENTRIES),
        })
    }

    /// Synchronous compatibility path for direct callers/tests. Async runtime
    /// ops execute the same plan on Tokio's bounded blocking pool.
    pub(crate) fn list_directory(
        &self,
        request: FileListRequest,
        cancel: Option<&ListingCancelToken>,
    ) -> Result<FileListPage, WorkspaceError> {
        traverse_directory(self.prepare_directory_listing(request)?, cancel)
    }
}

/// Traverse a previously authorized directory-listing plan. This performs all
/// blocking filesystem work and reads no mutable workspace state.
pub(crate) fn traverse_directory(
    plan: DirectoryListingPlan,
    cancel: Option<&ListingCancelToken>,
) -> Result<FileListPage, WorkspaceError> {
    if cancel.is_some_and(|token| token.load(Ordering::Relaxed)) {
        return Ok(cancelled_listing_page(plan.root_id));
    }
    let target = plan.root_path.join(&plan.relative_path);
    let canonical_target =
        fs::canonicalize(&target).map_err(|source| WorkspaceError::FileUnavailable {
            path: plan.relative_path.clone(),
            source,
        })?;
    if !canonical_target.starts_with(&plan.root_path) {
        return Err(WorkspaceError::OutsideRoot);
    }
    let metadata =
        fs::metadata(&canonical_target).map_err(|source| WorkspaceError::FileUnavailable {
            path: plan.relative_path.clone(),
            source,
        })?;
    if !metadata.is_dir() {
        return Err(WorkspaceError::DirectoryOpen);
    }
    if cancel.is_some_and(|token| token.load(Ordering::Relaxed)) {
        return Ok(cancelled_listing_page(plan.root_id));
    }

    let ignore_set = load_ignore_set(&plan.root_path);
    if cancel.is_some_and(|token| token.load(Ordering::Relaxed)) {
        return Ok(cancelled_listing_page(plan.root_id));
    }
    let ignore_set = match ignore_set {
        Ok(ignore_set) => ignore_set,
        Err(message) => {
            return Ok(FileListPage {
                root_id: plan.root_id,
                entries: Vec::new(),
                truncated: true,
                cancelled: false,
                diagnostics: vec![WorkspaceDiagnostic::new(
                    FileErrorCode::AccessDenied,
                    format!("cannot apply root .gitignore: {message}"),
                    Some(
                        "Supported rules are names or root-relative paths with '*' and '?', plus an optional trailing '/' for directories."
                            .to_string(),
                    ),
                )],
            });
        }
    };
    let mut entries = Vec::new();
    let mut diagnostics = Vec::new();
    let mut truncated = false;
    let mut cancelled = false;
    list_directory_recursive(
        &canonical_target,
        &plan.relative_path,
        0,
        plan.max_depth,
        plan.max_entries,
        &ignore_set,
        cancel,
        &mut entries,
        &mut diagnostics,
        &mut truncated,
        &mut cancelled,
    );
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    diagnostics.sort_by(|left, right| left.message.cmp(&right.message));
    Ok(FileListPage {
        root_id: plan.root_id,
        entries,
        truncated,
        cancelled,
        diagnostics,
    })
}

fn cancelled_listing_page(root_id: WorkspaceRootId) -> FileListPage {
    FileListPage {
        root_id,
        entries: Vec::new(),
        truncated: false,
        cancelled: true,
        diagnostics: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn list_directory_recursive(
    dir_path: &Path,
    dir_relative: &Path,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    ignore_set: &IgnoreSet,
    cancel: Option<&ListingCancelToken>,
    entries: &mut Vec<FileListEntry>,
    diagnostics: &mut Vec<WorkspaceDiagnostic>,
    truncated: &mut bool,
    cancelled: &mut bool,
) {
    // `depth` is the depth of the directory being processed relative to the
    // requested directory (target = 0). Entries emitted are at depth+1.
    // Recursion stops when the next directory would be at max_depth.
    if depth >= max_depth {
        return;
    }
    if let Some(token) = cancel
        && token.load(Ordering::Relaxed)
    {
        *cancelled = true;
        return;
    }

    let read_dir = match fs::read_dir(dir_path) {
        Ok(read_dir) => read_dir,
        Err(source) => {
            let code = io_error_code(&source);
            diagnostics.push(WorkspaceDiagnostic::new(
                code.clone(),
                format!(
                    "cannot read directory {}: {source}",
                    display_authorized_path(dir_relative)
                ),
                Some(container_permission_hint()),
            ));
            let diagnostic = Some(FileListEntryDiagnostic {
                code,
                message: format!("cannot read directory: {source}"),
            });
            if let Some(entry) = entries
                .iter_mut()
                .find(|entry| entry.relative_path == dir_relative)
            {
                entry.diagnostic = diagnostic;
            } else {
                entries.push(FileListEntry {
                    name: dir_relative
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| ".".to_string()),
                    kind: FileListEntryKind::Directory,
                    relative_path: dir_relative.to_path_buf(),
                    size_hint: None,
                    child_count: None,
                    diagnostic,
                });
            }
            return;
        }
    };

    let mut child_dirs: Vec<(PathBuf, PathBuf)> = Vec::new();
    for dir_entry in read_dir {
        if entries.len() >= max_entries {
            *truncated = true;
            return;
        }
        if let Some(token) = cancel
            && token.load(Ordering::Relaxed)
        {
            *cancelled = true;
            return;
        }

        let dir_entry = match dir_entry {
            Ok(entry) => entry,
            Err(source) => {
                diagnostics.push(WorkspaceDiagnostic::new(
                    io_error_code(&source),
                    format!(
                        "cannot read an entry in directory {}: {source}",
                        display_authorized_path(dir_relative)
                    ),
                    Some(container_permission_hint()),
                ));
                continue;
            }
        };

        let name = dir_entry.file_name();
        let name_str = name.to_string_lossy();
        let entry_path = dir_entry.path();
        let relative_path = if dir_relative.as_os_str().is_empty() {
            PathBuf::from(&name)
        } else {
            dir_relative.join(&name)
        };

        let metadata = match dir_entry.metadata() {
            Ok(metadata) => metadata,
            Err(source) => {
                let code = io_error_code(&source);
                diagnostics.push(WorkspaceDiagnostic::new(
                    code.clone(),
                    format!(
                        "cannot read metadata for {}: {source}",
                        display_authorized_path(&relative_path)
                    ),
                    Some(container_permission_hint()),
                ));
                entries.push(FileListEntry {
                    name: name_str.into_owned(),
                    kind: FileListEntryKind::Other,
                    relative_path,
                    size_hint: None,
                    child_count: None,
                    diagnostic: Some(FileListEntryDiagnostic {
                        code,
                        message: format!("cannot read metadata: {source}"),
                    }),
                });
                continue;
            }
        };

        if ignore_set.is_ignored(&relative_path, metadata.is_dir()) {
            continue;
        }

        let (kind, child_count) = if metadata.is_dir() {
            (
                FileListEntryKind::Directory,
                Some(count_visible_children(
                    &entry_path,
                    &relative_path,
                    ignore_set,
                )),
            )
        } else if metadata.is_file() {
            (FileListEntryKind::File, None)
        } else if metadata.file_type().is_symlink() {
            (FileListEntryKind::Symlink, None)
        } else {
            (FileListEntryKind::Other, None)
        };

        if metadata.is_dir() && depth + 1 < max_depth {
            child_dirs.push((entry_path.clone(), relative_path.clone()));
        }

        entries.push(FileListEntry {
            name: name_str.into_owned(),
            kind,
            relative_path,
            size_hint: if metadata.is_file() {
                Some(metadata.len())
            } else {
                None
            },
            child_count,
            diagnostic: None,
        });
    }

    for (child_path, child_relative) in child_dirs {
        if entries.len() >= max_entries {
            *truncated = true;
            return;
        }
        list_directory_recursive(
            &child_path,
            &child_relative,
            depth + 1,
            max_depth,
            max_entries,
            ignore_set,
            cancel,
            entries,
            diagnostics,
            truncated,
            cancelled,
        );
    }
}

impl WorkspaceState {
    pub(crate) async fn open_existing_file(
        &mut self,
        root_id: WorkspaceRootId,
        file_path: impl AsRef<Path>,
        client_id: ClientId,
    ) -> Result<OpenDocumentLease, WorkspaceError> {
        match self
            .prepare_open_existing(root_id, file_path.as_ref(), client_id)
            .await?
        {
            OpenPrepare::Existing(lease) => Ok(lease),
            OpenPrepare::New(plan) => {
                let (text, observed_metadata) = open_io(
                    &plan.file_state.canonical_path,
                    plan.file_state.workspace_relative_path.clone(),
                )
                .await?;
                let mut file_state = plan.file_state;
                file_state.last_known_metadata = observed_metadata;
                self.register_canonical_file(file_state, text, plan.client_id)
                    .await
            }
        }
    }

    pub(crate) async fn open_selected_file(
        &mut self,
        selected_path: impl AsRef<Path>,
        client_id: ClientId,
    ) -> Result<OpenDocumentLease, WorkspaceError> {
        match self
            .prepare_open_selected(selected_path.as_ref(), client_id)
            .await?
        {
            SelectedOpenPrepare::Existing(lease) => Ok(lease),
            SelectedOpenPrepare::New(plan) => {
                let (text, observed_metadata) =
                    open_io(&plan.canonical_path, plan.display_path.clone()).await?;
                self.register_selected_file(plan, text, observed_metadata)
                    .await
            }
        }
    }

    async fn prepare_open_existing(
        &self,
        root_id: WorkspaceRootId,
        file_path: &Path,
        client_id: ClientId,
    ) -> Result<OpenPrepare, WorkspaceError> {
        let file_state = self.canonical_file_state(root_id, file_path)?;
        if let Some(existing) = self.existing_document_lease(&file_state, client_id).await {
            return existing.map(OpenPrepare::Existing);
        }
        check_openable_size(&file_state.last_known_metadata, file_path)?;
        Ok(OpenPrepare::New(OpenPlan {
            file_state,
            client_id,
        }))
    }

    async fn prepare_open_selected(
        &self,
        selected_path: &Path,
        client_id: ClientId,
    ) -> Result<SelectedOpenPrepare, WorkspaceError> {
        let (canonical_path, metadata, display_path) = canonical_selected_file(selected_path)?;
        if let Some(existing) = self
            .existing_document_lease_by_canonical_path(&canonical_path, client_id)
            .await
        {
            return existing.map(SelectedOpenPrepare::Existing);
        }
        check_openable_size(&metadata, selected_path)?;
        Ok(SelectedOpenPrepare::New(SelectedOpenPlan {
            canonical_path,
            display_path,
            client_id,
        }))
    }

    async fn register_selected_file(
        &mut self,
        plan: SelectedOpenPlan,
        text: String,
        observed_metadata: FileMetadata,
    ) -> Result<OpenDocumentLease, WorkspaceError> {
        // The selected file may have been opened by a concurrent selected/open
        // call while the workspace mutex was released during the disk read.
        // Re-check (no grant allocated yet) and hand back the existing lease
        // instead of registering a duplicate document entry or orphan root.
        if let Some(existing) = self
            .existing_document_lease_by_canonical_path(&plan.canonical_path, plan.client_id)
            .await
        {
            return existing;
        }
        self.enforce_document_ceilings(plan.client_id).await?;
        let root_id = self.add_single_file_grant(plan.canonical_path.clone())?;
        let file_state = FileDocumentState {
            workspace_root_id: root_id,
            canonical_path: plan.canonical_path,
            workspace_relative_path: plan.display_path,
            last_known_metadata: observed_metadata,
        };
        self.register_canonical_file(file_state, text, plan.client_id)
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
            return existing;
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
        // Fail closed like an unknown document: a connection that never
        // opened this document must not learn that it exists (Plan 060 T4).
        if !open_document.document.lock().await.has_access(client_id) {
            return Err(WorkspaceError::UnknownDocument { document_id });
        }
        metadata_for_open_document(document_id, open_document, client_id).await
    }

    pub(crate) async fn list_documents(
        &self,
        client_id: ClientId,
    ) -> Result<Vec<DocumentMetadata>, WorkspaceError> {
        let mut entries = Vec::with_capacity(self.documents.len());
        for (&document_id, open_document) in &self.documents {
            if !open_document.document.lock().await.has_access(client_id) {
                continue;
            }
            entries.push(metadata_for_open_document(document_id, open_document, client_id).await?);
        }
        entries.sort_by_key(|metadata| metadata.document_id);
        Ok(entries)
    }

    pub(crate) async fn open_document_snapshots(
        &self,
        client_id: ClientId,
    ) -> Result<Vec<OpenDocumentSnapshot>, WorkspaceError> {
        let mut entries = Vec::with_capacity(self.documents.len());
        for (&document_id, open_document) in &self.documents {
            let metadata =
                metadata_for_open_document(document_id, open_document, client_id).await?;
            let text = open_document.document.lock().await.text();
            entries.push(OpenDocumentSnapshot { metadata, text });
        }
        entries.sort_by_key(|snapshot| snapshot.metadata.document_id);
        Ok(entries)
    }

    /// Release every access grant held by `client_id` (disconnect). Returns
    /// the IDs and final versions of documents whose last holder left, so the
    /// caller can tear down document-scoped coordinator state.
    pub(crate) async fn release_client_access(
        &mut self,
        client_id: ClientId,
    ) -> Vec<(DocumentId, DocumentVersion)> {
        let mut finalized = Vec::new();
        for (&document_id, open_document) in &self.documents {
            let mut document = open_document.document.lock().await;
            document.release_access(client_id);
            if document.access_holder_count() == 0 {
                finalized.push((document_id, document.version()));
            }
        }
        for &(document_id, _) in &finalized {
            if let Some(open_document) = self.documents.remove(&document_id) {
                self.path_to_document
                    .remove(&open_document.file_state.canonical_path);
            }
        }
        finalized
    }

    /// Explicit close: release `client_id`'s access after the access, and
    /// dirty policy checks pass. A dirty document requires `force` so close
    /// intent is explicit about discarding unsaved editor state. When the
    /// last holder leaves, the registry entry is removed and the outcome
    /// reports `closed: true` so the caller can tear down document-scoped
    /// coordinator state (Plan 060 T6, P1-4).
    pub(crate) async fn close_document(
        &mut self,
        document_id: DocumentId,
        client_id: ClientId,
        force: bool,
    ) -> Result<CloseDocumentOutcome, WorkspaceError> {
        let open_document = self
            .documents
            .get(&document_id)
            .ok_or(WorkspaceError::UnknownDocument { document_id })?;
        let mut document = open_document.document.lock().await;
        if !document.has_access(client_id) {
            return Err(WorkspaceError::UnknownDocument { document_id });
        }
        if document.is_dirty() && !force {
            return Err(WorkspaceError::DirtyDocument { document_id });
        }
        document.release_access(client_id);
        let version = document.version();
        let closed = document.access_holder_count() == 0;
        drop(document);
        if closed {
            let open_document = self
                .documents
                .remove(&document_id)
                .expect("document checked above");
            self.path_to_document
                .remove(&open_document.file_state.canonical_path);
        }
        Ok(CloseDocumentOutcome {
            document_id,
            version,
            closed,
        })
    }

    /// Per-client and server-wide open-document ceilings, checked before a
    /// new access grant or registry entry is created (Plan 060 T6, P1-4).
    async fn enforce_document_ceilings(&self, client_id: ClientId) -> Result<(), WorkspaceError> {
        if self.documents.len() >= MAX_SERVER_DOCUMENTS {
            return Err(WorkspaceError::DocumentLimitExceeded {
                limit: MAX_SERVER_DOCUMENTS,
            });
        }
        let mut held = 0;
        for open_document in self.documents.values() {
            if open_document.document.lock().await.has_access(client_id) {
                held += 1;
            }
        }
        if held >= MAX_DOCUMENTS_PER_CLIENT {
            return Err(WorkspaceError::DocumentLimitExceeded {
                limit: MAX_DOCUMENTS_PER_CLIENT,
            });
        }
        Ok(())
    }

    pub(crate) async fn save_document(
        &mut self,
        document_id: DocumentId,
        client_id: ClientId,
        known_version: DocumentVersion,
    ) -> Result<SaveDocumentOutcome, WorkspaceError> {
        self.authorize_save(document_id, client_id, known_version)
            .await?;
        let plan = self.prepare_save(document_id)?;
        let io = save_io(&plan).await?;
        self.commit_save(plan, io).await
    }

    pub(crate) async fn reload_document(
        &mut self,
        document_id: DocumentId,
        client_id: ClientId,
        force: bool,
    ) -> Result<ReloadDocumentOutcome, WorkspaceError> {
        self.authorize_document_access(document_id, client_id)
            .await?;
        let plan = self.prepare_reload(document_id, force).await?;
        let io = reload_io(&plan).await?;
        self.commit_reload(plan, io).await
    }

    /// Fail closed when `client_id` never acquired access to `document_id`
    /// through an authorized open path. Returns the same `UnknownDocument`
    /// error as a missing document so unauthorized probes cannot distinguish
    /// existence from denial (Plan 060 T4, P0-2).
    async fn authorize_document_access(
        &self,
        document_id: DocumentId,
        client_id: ClientId,
    ) -> Result<(), WorkspaceError> {
        let open_document = self
            .documents
            .get(&document_id)
            .ok_or(WorkspaceError::UnknownDocument { document_id })?;
        if !open_document.document.lock().await.has_access(client_id) {
            return Err(WorkspaceError::UnknownDocument { document_id });
        }
        Ok(())
    }

    /// Save authorization: the caller must hold the editable lease (read-only
    /// saves fail closed) and must not claim a version newer than the server
    /// has confirmed. `known_version <= current` is accepted because only the
    /// lease holder can edit, so any gap is the caller's own in-flight edits
    /// ordered before the save on the same connection.
    async fn authorize_save(
        &self,
        document_id: DocumentId,
        client_id: ClientId,
        known_version: DocumentVersion,
    ) -> Result<(), WorkspaceError> {
        let open_document = self
            .documents
            .get(&document_id)
            .ok_or(WorkspaceError::UnknownDocument { document_id })?;
        let document = open_document.document.lock().await;
        if !document.has_access(client_id) {
            return Err(WorkspaceError::UnknownDocument { document_id });
        }
        // The server-internal runtime identity (0) saves with full authority;
        // remote connections must hold the editable lease.
        if client_id != 0
            && !matches!(
                document.access_for_client(client_id),
                crate::protocol::DocumentAccess::Editable { .. }
            )
        {
            return Err(WorkspaceError::ReadOnlySave { document_id });
        }
        let current_version = document.version();
        if known_version > current_version {
            return Err(WorkspaceError::StaleSaveVersion {
                document_id,
                known_version,
                current_version,
            });
        }
        Ok(())
    }

    /// Gather the owned state needed to write `document_id` to disk: canonical
    /// path, registry-relative path, the document handle, and a staleness
    /// reauthorization against the current on-disk metadata. Runs under the
    /// workspace mutex; the heavy `tokio::fs::write` happens in [`save_io`]
    /// after the mutex is released.
    fn prepare_save(&self, document_id: DocumentId) -> Result<SavePlan, WorkspaceError> {
        let open_document = self
            .documents
            .get(&document_id)
            .ok_or(WorkspaceError::UnknownDocument { document_id })?;
        let canonical_path = open_document.file_state.canonical_path.clone();
        let relative_path = open_document.file_state.workspace_relative_path.clone();
        let expected_metadata = open_document.file_state.last_known_metadata.clone();
        let document = Arc::clone(&open_document.document);

        let current_identity = self.reauthorize_open_file(document_id)?;
        if current_identity.metadata() != expected_metadata {
            return Err(WorkspaceError::StaleFileMetadata {
                path: relative_path,
            });
        }
        Ok(SavePlan {
            document_id,
            canonical_path,
            relative_path,
            document,
            expected_identity: current_identity,
        })
    }

    async fn commit_save(
        &mut self,
        plan: SavePlan,
        io: SaveIoOutcome,
    ) -> Result<SaveDocumentOutcome, WorkspaceError> {
        // The document may have been closed by another connection while the
        // workspace mutex was released during the write. The bytes are already
        // on disk; update metadata only if the registry entry still exists.
        if let Some(open_document) = self.documents.get_mut(&plan.document_id) {
            open_document.file_state.last_known_metadata = io.saved_metadata;
        }
        let dirty = {
            let mut document = plan.document.lock().await;
            !document.mark_clean_if_version(io.prepared_version)
        };
        Ok(SaveDocumentOutcome {
            document_id: plan.document_id,
            version: io.prepared_version,
            dirty,
        })
    }

    /// Gather the owned state needed to reload `document_id` from disk: a
    /// dirty pre-check (unless `force`), a reauthorization against the current
    /// on-disk metadata, and the openable-size gate. Runs under the workspace
    /// mutex; the `tokio::fs::read` happens in [`reload_io`] after the mutex is
    /// released.
    async fn prepare_reload(
        &self,
        document_id: DocumentId,
        force: bool,
    ) -> Result<ReloadPlan, WorkspaceError> {
        let open_document = self
            .documents
            .get(&document_id)
            .ok_or(WorkspaceError::UnknownDocument { document_id })?;
        let canonical_path = open_document.file_state.canonical_path.clone();
        let relative_path = open_document.file_state.workspace_relative_path.clone();
        let document = Arc::clone(&open_document.document);
        if document.lock().await.is_dirty() && !force {
            return Err(WorkspaceError::DirtyDocument { document_id });
        }
        let pre_read_identity = self.reauthorize_open_file(document_id)?;
        check_openable_size(&pre_read_identity.metadata(), &relative_path)?;
        Ok(ReloadPlan {
            document_id,
            canonical_path,
            relative_path,
            document,
            force,
        })
    }

    async fn commit_reload(
        &mut self,
        plan: ReloadPlan,
        io: ReloadIoOutcome,
    ) -> Result<ReloadDocumentOutcome, WorkspaceError> {
        // Re-check dirtiness on reacquire: the document may have been edited by
        // another connection during the unlocked read. Don't clobber unsaved
        // edits unless the caller asked to force the reload.
        if plan.document.lock().await.is_dirty() && !plan.force {
            return Err(WorkspaceError::DirtyDocument {
                document_id: plan.document_id,
            });
        }
        let version = {
            let mut document = plan.document.lock().await;
            document.replace_text_from_storage(io.text.clone());
            document.version()
        };
        if let Some(open_document) = self.documents.get_mut(&plan.document_id) {
            open_document.file_state.last_known_metadata = io.reloaded_metadata;
        }
        Ok(ReloadDocumentOutcome {
            document_id: plan.document_id,
            version,
            text: io.text,
            dirty: false,
        })
    }

    async fn existing_document_lease(
        &self,
        file_state: &FileDocumentState,
        client_id: ClientId,
    ) -> Option<Result<OpenDocumentLease, WorkspaceError>> {
        self.existing_document_lease_by_canonical_path(&file_state.canonical_path, client_id)
            .await
    }

    async fn existing_document_lease_by_canonical_path(
        &self,
        canonical_path: &Path,
        client_id: ClientId,
    ) -> Option<Result<OpenDocumentLease, WorkspaceError>> {
        let document_id = self.path_to_document.get(canonical_path).copied()?;
        let open_document = self
            .documents
            .get(&document_id)
            .expect("path index and document registry must stay in sync");
        if !open_document.document.lock().await.has_access(client_id)
            && let Err(error) = self.enforce_document_ceilings(client_id).await
        {
            return Some(Err(error));
        }
        let access = open_document
            .document
            .lock()
            .await
            .acquire_access(client_id);
        Some(Ok(OpenDocumentLease {
            document_id,
            access,
            file_state: open_document.file_state.clone(),
            document: Arc::clone(&open_document.document),
        }))
    }

    fn add_single_file_grant(
        &mut self,
        canonical_path: PathBuf,
    ) -> Result<WorkspaceRootId, WorkspaceError> {
        if self.roots.len() >= MAX_WORKSPACE_ROOTS {
            return Err(WorkspaceError::RootLimitExceeded);
        }
        let id = self.next_root_id;
        self.next_root_id = self.next_root_id.saturating_add(1);
        self.roots.insert(
            id,
            WorkspaceRoot {
                id,
                authority: WorkspaceAuthority::SingleFile { canonical_path },
            },
        );
        Ok(id)
    }

    async fn register_canonical_file(
        &mut self,
        file_state: FileDocumentState,
        text: String,
        client_id: ClientId,
    ) -> Result<OpenDocumentLease, WorkspaceError> {
        // The unlock-across-IO orchestration can let two callers finish reading
        // the same canonical path before either commits. Re-check under the
        // workspace mutex and prefer the existing document instead of inserting
        // a duplicate registry entry (the caller's read text is discarded).
        if let Some(existing) = self
            .existing_document_lease_by_canonical_path(&file_state.canonical_path, client_id)
            .await
        {
            return existing;
        }
        self.enforce_document_ceilings(client_id).await?;
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
        match &root.authority {
            WorkspaceAuthority::Directory {
                canonical_path: root_path,
            } => {
                let joined = if file_path.is_absolute() {
                    file_path.to_path_buf()
                } else {
                    root_path.join(file_path)
                };
                let canonical_path = fs::canonicalize(&joined).map_err(|source| {
                    WorkspaceError::FileUnavailable {
                        path: file_path.to_path_buf(),
                        source,
                    }
                })?;
                if !canonical_path.starts_with(root_path) {
                    return Err(WorkspaceError::OutsideRoot);
                }
                let metadata = fs::metadata(&canonical_path).map_err(|source| {
                    WorkspaceError::FileUnavailable {
                        path: file_path.to_path_buf(),
                        source,
                    }
                })?;
                validate_regular_file_metadata(&metadata)?;
                let relative_path = canonical_path
                    .strip_prefix(root_path)
                    .map_err(|_| WorkspaceError::OutsideRoot)?
                    .to_path_buf();
                Ok(FileDocumentState {
                    workspace_root_id: root_id,
                    canonical_path,
                    workspace_relative_path: relative_path,
                    last_known_metadata: FileMetadata::from_fs_metadata(&metadata),
                })
            }
            WorkspaceAuthority::SingleFile {
                canonical_path: granted_path,
            } => {
                let requested = if file_path.is_absolute() {
                    file_path.to_path_buf()
                } else {
                    granted_path
                        .parent()
                        .map_or_else(|| file_path.to_path_buf(), |parent| parent.join(file_path))
                };
                let canonical_path = fs::canonicalize(&requested).map_err(|source| {
                    WorkspaceError::FileUnavailable {
                        path: file_path.to_path_buf(),
                        source,
                    }
                })?;
                if canonical_path != *granted_path {
                    return Err(WorkspaceError::OutsideRoot);
                }
                let metadata = fs::metadata(&canonical_path).map_err(|source| {
                    WorkspaceError::FileUnavailable {
                        path: file_path.to_path_buf(),
                        source,
                    }
                })?;
                validate_regular_file_metadata(&metadata)?;
                Ok(FileDocumentState {
                    workspace_root_id: root_id,
                    workspace_relative_path: selected_file_display_path(&canonical_path),
                    canonical_path,
                    last_known_metadata: FileMetadata::from_fs_metadata(&metadata),
                })
            }
        }
    }

    fn reauthorize_open_file(
        &self,
        document_id: DocumentId,
    ) -> Result<TargetIdentity, WorkspaceError> {
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
        match &root.authority {
            WorkspaceAuthority::Directory {
                canonical_path: root_path,
            } => {
                if !canonical_path.starts_with(root_path) {
                    return Err(WorkspaceError::OutsideRoot);
                }
            }
            WorkspaceAuthority::SingleFile {
                canonical_path: granted_path,
            } => {
                if canonical_path != *granted_path {
                    return Err(WorkspaceError::OutsideRoot);
                }
            }
        }
        let metadata =
            fs::metadata(&canonical_path).map_err(|source| WorkspaceError::FileUnavailable {
                path: open_document.file_state.workspace_relative_path.clone(),
                source,
            })?;
        validate_regular_file_metadata(&metadata)?;
        Ok(TargetIdentity::from_metadata(&metadata))
    }
}

impl FileDocumentState {
    pub(crate) fn workspace_root_id(&self) -> WorkspaceRootId {
        self.workspace_root_id
    }

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

    /// File size in bytes as last observed from the filesystem.
    fn len(&self) -> u64 {
        self.len
    }
}

fn canonical_selected_file(
    selected_path: &Path,
) -> Result<(PathBuf, FileMetadata, PathBuf), WorkspaceError> {
    let canonical_path =
        fs::canonicalize(selected_path).map_err(|source| WorkspaceError::FileUnavailable {
            path: selected_path.to_path_buf(),
            source,
        })?;
    let metadata =
        fs::metadata(&canonical_path).map_err(|source| WorkspaceError::FileUnavailable {
            path: selected_path.to_path_buf(),
            source,
        })?;
    validate_regular_file_metadata(&metadata)?;
    Ok((
        canonical_path.clone(),
        FileMetadata::from_fs_metadata(&metadata),
        selected_file_display_path(&canonical_path),
    ))
}

fn selected_file_display_path(canonical_path: &Path) -> PathBuf {
    canonical_path
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("selected-file"))
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

/// Reject a file whose observed size exceeds the openable-file budget *before*
/// the bounded read allocates. Returns a typed
/// [`WorkspaceError::FileTooLarge`] so oversized files cannot be used as a
/// memory-exhaustion vector or silently fail later at frame encode.
fn check_openable_size(metadata: &FileMetadata, path: &Path) -> Result<(), WorkspaceError> {
    let len = metadata.len();
    if len as usize > MAX_OPENABLE_FILE_BYTES {
        return Err(WorkspaceError::FileTooLarge {
            path: path.to_path_buf(),
            len,
            max: MAX_OPENABLE_FILE_BYTES,
        });
    }
    Ok(())
}

/// Bounded read of one file through a single opened handle, performed with the
/// workspace mutex released. Handle-based metadata closes the
/// swap-between-validation-and-read window (the type/size checks describe the
/// exact file being read, not a path that could be replaced in between), and
/// the `take` ceiling caps allocation at `max_bytes + 1` even if the file
/// grows after the metadata check. `error_path` is the registry-relative path
/// used in diagnostics so callers can pass either the workspace-relative path
/// (root-scoped open) or the selected-file display path.
async fn read_file_bounded(
    canonical_path: &Path,
    error_path: &Path,
    max_bytes: usize,
) -> Result<(String, FileMetadata), WorkspaceError> {
    let unavailable = |source: io::Error| WorkspaceError::FileUnavailable {
        path: error_path.to_path_buf(),
        source,
    };
    let file = tokio_fs::File::open(canonical_path)
        .await
        .map_err(unavailable)?;
    let metadata = file.metadata().await.map_err(unavailable)?;
    validate_regular_file_metadata(&metadata)?;
    let observed = FileMetadata::from_fs_metadata(&metadata);
    check_openable_size(&observed, error_path)?;
    #[cfg(test)]
    {
        let mut hooks = BETWEEN_METADATA_AND_READ_HOOKS
            .lock()
            .expect("between-metadata-and-read hooks poisoned");
        if let Some(position) = hooks.iter().rposition(|(path, _)| path == canonical_path) {
            let (_, hook) = hooks.remove(position);
            drop(hooks);
            hook();
        }
    }
    let mut bytes = Vec::new();
    file.take((max_bytes as u64) + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(unavailable)?;
    if bytes.len() > max_bytes {
        return Err(WorkspaceError::FileTooLarge {
            path: error_path.to_path_buf(),
            len: bytes.len() as u64,
            max: max_bytes,
        });
    }
    let text = String::from_utf8(bytes).map_err(|source| WorkspaceError::InvalidUtf8 {
        path: error_path.to_path_buf(),
        source,
    })?;
    Ok((text, observed))
}

/// Heavy disk read for file open, performed with the workspace mutex released.
async fn open_io(
    canonical_path: &Path,
    error_path: PathBuf,
) -> Result<(String, FileMetadata), WorkspaceError> {
    read_file_bounded(canonical_path, &error_path, MAX_OPENABLE_FILE_BYTES).await
}

/// Stable on-disk identity of the save target captured when the save was
/// authorized. Revalidating this immediately before the atomic replace closes
/// the TOCTOU window in which an external process (or another editor doing an
/// atomic rename-write) could replace the target between the staleness check
/// and the rename: the save then fails closed instead of silently clobbering
/// someone else's bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TargetIdentity {
    /// Platform stable file identity: `(dev, ino)` on Unix, `(volume serial,
    /// file index)` on Windows. `None` when the platform cannot report one;
    /// matching then falls back to content metadata only.
    stable_id: Option<(u64, u64)>,
    len: u64,
    modified: Option<SystemTime>,
}

impl TargetIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        #[cfg(unix)]
        let stable_id = {
            use std::os::unix::fs::MetadataExt;
            Some((metadata.dev(), metadata.ino()))
        };
        #[cfg(windows)]
        let stable_id = {
            use std::os::windows::fs::MetadataExt;
            metadata.volume_serial_number().zip(metadata.file_index())
        };
        #[cfg(not(any(unix, windows)))]
        let stable_id = None;
        Self {
            stable_id,
            len: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }

    fn capture(path: &Path) -> io::Result<Self> {
        fs::metadata(path).map(|metadata| Self::from_metadata(&metadata))
    }

    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            len: self.len,
            modified: self.modified,
        }
    }

    /// True when the current on-disk target is the same file with the same
    /// content metadata as when this identity was captured. A missing target
    /// is an error (fail closed), a replaced file or any content change —
    /// including a same-length edit that bumps `modified` — is a mismatch.
    fn matches_current(&self, path: &Path) -> io::Result<bool> {
        let metadata = fs::metadata(path)?;
        let current = Self::from_metadata(&metadata);
        Ok(current.stable_id == self.stable_id
            && current.len == self.len
            && current.modified == self.modified)
    }
}

/// Process-wide counter for unique atomic-save temp file names so concurrent
/// saves of the *same* canonical path do not collide on a shared temp name.
static ATOMIC_SAVE_TEMP_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Test-only queue of forced temp file names scoped by target directory (one
/// entry popped per atomic save in that directory) so tests can stage
/// pre-created temp collisions and symlink attacks without cross-test races.
#[cfg(test)]
static TEST_TEMP_NAMES: std::sync::Mutex<Vec<(PathBuf, String)>> =
    std::sync::Mutex::new(Vec::new());

/// Path-scoped test hook fired once at a staged filesystem hazard window.
#[cfg(test)]
type TestHook = (PathBuf, Box<dyn FnOnce() + Send>);

/// Test-only hook invoked inside the atomic save for a specific target path
/// after the temp is written and fsynced but before the target identity is
/// revalidated and replaced, so tests can stage external target
/// modification/replacement in that exact window.
#[cfg(test)]
static BEFORE_REVALIDATE_HOOKS: std::sync::Mutex<Vec<TestHook>> = std::sync::Mutex::new(Vec::new());

/// Test-only hooks invoked inside the bounded read of a specific path after
/// the handle metadata is validated but before the contents are read, so
/// tests can stage external growth/replacement in that exact window.
#[cfg(test)]
static BETWEEN_METADATA_AND_READ_HOOKS: std::sync::Mutex<Vec<TestHook>> =
    std::sync::Mutex::new(Vec::new());

/// Build an unpredictable temp-file path next to `target` for atomic save. The
/// temp lives in the same directory so the `rename` is a same-filesystem atomic
/// replace (POSIX rename overwrites atomically; on Windows Rust's
/// `std::fs::rename` uses `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`). The random
/// component comes from a process-random `RandomState` seed (std's OS-entropy
/// hash keys) mixed with a unique counter, so an attacker who can pre-create
/// files in the directory cannot predict the name and pre-place a symlink.
fn atomic_temp_path(target: &Path) -> PathBuf {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    #[cfg(test)]
    {
        let mut queue = TEST_TEMP_NAMES
            .lock()
            .expect("test temp-name queue poisoned");
        if let Some(position) = queue.iter().rposition(|(dir, _)| dir == parent) {
            let (_, name) = queue.remove(position);
            return parent.join(name);
        }
    }
    let nonce = ATOMIC_SAVE_TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let random = {
        use std::hash::{BuildHasher, Hasher};
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u64(nonce);
        hasher.finish()
    };
    let stem = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "clay-save".to_string());
    parent.join(format!(".{stem}.clay-save-{random:016x}"))
}

/// Failure modes of the atomic save that callers must distinguish: ordinary IO
/// failures map to `WriteFailed`, while a target that changed identity or
/// content during the save maps to the typed staleness error.
#[derive(Debug)]
enum AtomicSaveError {
    Io(io::Error),
    TargetChanged,
}

impl From<io::Error> for AtomicSaveError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Create the exclusive temp file next to `target`, retrying with a fresh
/// unpredictable name on collision. `create_new` refuses to follow or truncate
/// a pre-created file/symlink at the temp path, closing the predictable-temp
/// precreation attack. On Unix the temp starts `0o600` regardless of umask.
async fn create_exclusive_temp(target: &Path) -> io::Result<(PathBuf, tokio_fs::File)> {
    const MAX_TEMP_CREATE_ATTEMPTS: usize = 8;
    let mut attempt = 0;
    loop {
        let temp_path = atomic_temp_path(target);
        let mut options = tokio_fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&temp_path).await {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                attempt += 1;
                if attempt >= MAX_TEMP_CREATE_ATTEMPTS {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
}

/// Atomically write `bytes` to `target` via a temp file + rename, returning the
/// metadata of the saved file. Steps: create an exclusive unpredictable temp
/// file in `target`'s directory, write all bytes, `fsync` (durability),
/// restore the original file's permissions (fail closed on Unix when that
/// fails), revalidate the target's stable identity against `expected`
/// immediately before the replace, then `rename` over the target. Every
/// failure path removes the temp file and leaves the target untouched.
async fn atomic_write_file(
    target: &Path,
    bytes: &[u8],
    expected: Option<&TargetIdentity>,
) -> Result<FileMetadata, AtomicSaveError> {
    #[cfg(unix)]
    if fs::metadata(target)
        .map(|metadata| metadata.permissions().mode() & 0o222 == 0)
        .unwrap_or(false)
    {
        return Err(
            io::Error::new(io::ErrorKind::PermissionDenied, "target file is read-only").into(),
        );
    }

    // Preserve the original file's permissions (Unix mode). Metadata of a
    // missing target is ignored; the temp then keeps its `0o600` start mode,
    // which is a safe default for a brand-new file.
    let original_permissions = fs::metadata(target)
        .ok()
        .map(|metadata| metadata.permissions());

    let (temp_path, mut file) = create_exclusive_temp(target).await?;
    let write_result = async {
        file.write_all(bytes).await?;
        // Flush the kernel buffer and fsync so the new content is on disk
        // before the atomic rename; this is what makes the post-rename file
        // durable.
        file.sync_all().await
    }
    .await;
    drop(file);

    // ponytail: directory fsync of the parent would make the rename itself
    // durable across power loss; skipped here because the atomic rename already
    // guarantees the target is never torn, and cross-platform dir fsync needs
    // platform-specific code. Add if durability-of-the-rename becomes a
    // requirement.

    // From here on every failure must remove the orphaned temp file.
    let result = async {
        write_result?;

        #[cfg(unix)]
        if let Some(perms) = original_permissions {
            // Fail closed: a save that cannot preserve the target's required
            // permissions must not silently loosen them.
            fs::set_permissions(&temp_path, perms)?;
        }
        #[cfg(not(unix))]
        {
            // Windows: file permissions are coarser and the temp already
            // inherits the directory ACL; nothing practical to copy here.
            let _ = original_permissions;
        }

        #[cfg(test)]
        {
            let mut hooks = BEFORE_REVALIDATE_HOOKS
                .lock()
                .expect("before-revalidate hooks poisoned");
            if let Some(position) = hooks.iter().rposition(|(path, _)| path == target) {
                let (_, hook) = hooks.remove(position);
                drop(hooks);
                hook();
            }
        }

        // Revalidate the target's stable identity immediately before the
        // replace: an external edit, atomic-replace, or symlink swap during
        // the temp write fails the save instead of being silently clobbered.
        if let Some(expected) = expected {
            match expected.matches_current(target) {
                Ok(true) => {}
                Ok(false) => return Err(AtomicSaveError::TargetChanged),
                Err(error) => return Err(AtomicSaveError::Io(error)),
            }
        }

        tokio_fs::rename(&temp_path, target).await?;
        let metadata = tokio_fs::metadata(target).await?;
        Ok(FileMetadata::from_fs_metadata(&metadata))
    }
    .await;
    if result.is_err() {
        let _ = tokio_fs::remove_file(&temp_path).await;
    }
    result
}

/// Heavy disk write + post-write metadata read for `save_document`, performed
/// with the workspace mutex released. Captures the in-memory document version
/// and text (via the per-document mutex, not the workspace mutex) so the commit
/// phase can detect a concurrent edit through `mark_clean_if_version`.
async fn save_io(plan: &SavePlan) -> Result<SaveIoOutcome, WorkspaceError> {
    let (prepared_version, text) = {
        let document = plan.document.lock().await;
        (document.version(), document.text())
    };
    // Atomic save: write an exclusive unpredictable temp file in the target's
    // directory, fsync it, restore the original file's permissions, revalidate
    // the target's stable identity, then `rename` over the target. A crash or
    // power loss during the write leaves the original file intact (only the
    // temp is partial); the rename is atomic so the target is either the old
    // or the new content, never a torn write.
    let saved_metadata = atomic_write_file(
        &plan.canonical_path,
        text.as_bytes(),
        Some(&plan.expected_identity),
    )
    .await
    .map_err(|error| match error {
        AtomicSaveError::TargetChanged => WorkspaceError::StaleFileMetadata {
            path: plan.relative_path.clone(),
        },
        AtomicSaveError::Io(source) => WorkspaceError::WriteFailed {
            path: plan.relative_path.clone(),
            source,
        },
    })?;
    Ok(SaveIoOutcome {
        prepared_version,
        saved_metadata,
    })
}

/// Bounded disk read for `reload_document` through one opened handle,
/// performed with the workspace mutex released. The returned metadata comes
/// from the same handle that produced the text, so the registry records
/// exactly what was read.
async fn reload_io(plan: &ReloadPlan) -> Result<ReloadIoOutcome, WorkspaceError> {
    let (text, reloaded_metadata) = read_file_bounded(
        &plan.canonical_path,
        &plan.relative_path,
        MAX_OPENABLE_FILE_BYTES,
    )
    .await?;
    Ok(ReloadIoOutcome {
        text,
        reloaded_metadata,
    })
}

/// Open-scoped workspace orchestration that releases the workspace mutex during
/// the heavy disk I/O. Used by the IpcServer connection handlers and the Clay
/// JS document ops so concurrent operations on unrelated documents are not
/// serialized by a slow disk call. Each helper locks only for the `prepare`
/// fast phase (filesystem metadata + authority + registry lookup), drops the
/// guard, performs the `tokio::fs` read/write, then reacquires to `commit`.
/// The commit re-validates registry state on reacquire (concurrent-open dedup,
/// reload dirty re-check, save tolerates a closed document).
pub(crate) async fn open_existing_file_unlocked(
    workspace: &Arc<Mutex<WorkspaceState>>,
    root_id: WorkspaceRootId,
    file_path: impl AsRef<Path>,
    client_id: ClientId,
) -> Result<OpenDocumentLease, WorkspaceError> {
    let plan = {
        let workspace = workspace.lock().await;
        workspace
            .prepare_open_existing(root_id, file_path.as_ref(), client_id)
            .await?
    };
    match plan {
        OpenPrepare::Existing(lease) => Ok(lease),
        OpenPrepare::New(plan) => {
            let (text, observed_metadata) = open_io(
                &plan.file_state.canonical_path,
                plan.file_state.workspace_relative_path.clone(),
            )
            .await?;
            let mut file_state = plan.file_state;
            // Record the metadata of the handle actually read, not the
            // prepare-time stat, so the first save's staleness baseline
            // matches the bytes in the document.
            file_state.last_known_metadata = observed_metadata;
            let mut workspace = workspace.lock().await;
            workspace
                .register_canonical_file(file_state, text, plan.client_id)
                .await
        }
    }
}

pub(crate) async fn open_selected_file_unlocked(
    workspace: &Arc<Mutex<WorkspaceState>>,
    selected_path: impl AsRef<Path>,
    client_id: ClientId,
) -> Result<OpenDocumentLease, WorkspaceError> {
    let plan = {
        let workspace = workspace.lock().await;
        workspace
            .prepare_open_selected(selected_path.as_ref(), client_id)
            .await?
    };
    match plan {
        SelectedOpenPrepare::Existing(lease) => Ok(lease),
        SelectedOpenPrepare::New(plan) => {
            let (text, observed_metadata) =
                open_io(&plan.canonical_path, plan.display_path.clone()).await?;
            let mut workspace = workspace.lock().await;
            workspace
                .register_selected_file(plan, text, observed_metadata)
                .await
        }
    }
}

pub(crate) async fn save_document_unlocked(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document_id: DocumentId,
    client_id: ClientId,
    known_version: DocumentVersion,
) -> Result<SaveDocumentOutcome, WorkspaceError> {
    {
        let workspace = workspace.lock().await;
        workspace
            .authorize_save(document_id, client_id, known_version)
            .await?;
    }
    let plan = {
        let workspace = workspace.lock().await;
        workspace.prepare_save(document_id)?
    };
    let io = save_io(&plan).await?;
    let mut workspace = workspace.lock().await;
    workspace.commit_save(plan, io).await
}

pub(crate) async fn reload_document_unlocked(
    workspace: &Arc<Mutex<WorkspaceState>>,
    document_id: DocumentId,
    client_id: ClientId,
    force: bool,
) -> Result<ReloadDocumentOutcome, WorkspaceError> {
    {
        let workspace = workspace.lock().await;
        workspace
            .authorize_document_access(document_id, client_id)
            .await?;
    }
    let plan = {
        let workspace = workspace.lock().await;
        workspace.prepare_reload(document_id, force).await?
    };
    let io = reload_io(&plan).await?;
    let mut workspace = workspace.lock().await;
    workspace.commit_reload(plan, io).await
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
    ReadOnlySave {
        document_id: DocumentId,
    },
    DocumentLimitExceeded {
        limit: usize,
    },
    StaleSaveVersion {
        document_id: DocumentId,
        known_version: DocumentVersion,
        current_version: DocumentVersion,
    },
    StaleFileMetadata {
        path: PathBuf,
    },
    FileTooLarge {
        path: PathBuf,
        len: u64,
        max: usize,
    },
    RootLimitExceeded,
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
            Self::DocumentLimitExceeded { limit } => WorkspaceDiagnostic::new(
                FileErrorCode::WorkspaceLimitExceeded,
                format!("open-document limit of {limit} reached"),
                Some("Close documents you no longer need before opening more.".to_string()),
            ),
            Self::ReadOnlySave { document_id } => WorkspaceDiagnostic::new(
                FileErrorCode::AccessDenied,
                format!("workspace document {document_id} is read-only for this connection"),
                Some("Only the connection holding the editable lease can save a document.".to_string()),
            ),
            Self::StaleSaveVersion {
                document_id,
                known_version,
                current_version,
            } => WorkspaceDiagnostic::new(
                FileErrorCode::StaleFileMetadata,
                format!("save for workspace document {document_id} claims version {known_version} newer than the confirmed server version {current_version}"),
                Some("Resync the document before saving so no confirmed edits are overwritten.".to_string()),
            ),
            Self::StaleFileMetadata { path } => WorkspaceDiagnostic::new(
                FileErrorCode::StaleFileMetadata,
                format!("workspace file {} changed on disk since it was loaded", display_workspace_path(path)),
                Some("Reload or resolve the external change before saving to avoid overwriting data.".to_string()),
            ),
            Self::FileTooLarge { path, len, max } => WorkspaceDiagnostic::new(
                FileErrorCode::FileTooLarge,
                format!(
                    "workspace file {} is {} bytes which exceeds the {} byte openable-file limit",
                    display_workspace_path(path),
                    len,
                    max
                ),
                Some(format!(
                    "Open files smaller than {max} bytes. Chunked/viewport-first loading for larger files is planned but not yet available."
                )),
            ),
            Self::RootLimitExceeded => WorkspaceDiagnostic::new(
                FileErrorCode::WorkspaceLimitExceeded,
                format!(
                    "workspace root limit of {MAX_WORKSPACE_ROOTS} reached; cannot add another root or single-file grant"
                ),
                Some("Close unused roots or revoke single-file grants before adding more.".to_string()),
            ),
        }
    }
}

/// Global registry of in-flight listing cancellation tokens. Tokens are
/// removed when the listing finishes or is cancelled.
static LISTING_CANCELLATIONS: LazyLock<std::sync::Mutex<HashMap<String, ListingCancelToken>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

/// Register a caller-supplied cancellation token id, returning the existing
/// token when the caller created it first. Reusing the token prevents a cancel
/// racing list startup from being lost through replacement.
pub(crate) fn register_listing_cancel_token(id: String) -> ListingCancelToken {
    let mut map = LISTING_CANCELLATIONS.lock().unwrap();
    map.entry(id)
        .or_insert_with(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Return a new unique cancellation token id without registering active
/// state. The create-token API can therefore be called speculatively without
/// leaking process-lifetime entries; registration starts with the listing.
pub(crate) fn create_listing_cancel_token_id() -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    NEXT_ID.fetch_add(1, Ordering::Relaxed).to_string()
}

/// Allocate and register a token for a listing that starts immediately.
pub(crate) fn create_listing_cancel_token() -> (String, ListingCancelToken) {
    let id = create_listing_cancel_token_id();
    (id.clone(), register_listing_cancel_token(id))
}

/// Cancel a registered listing by token id. Returns true if the token existed.
pub(crate) fn cancel_listing(token_id: &str) -> bool {
    let map = LISTING_CANCELLATIONS.lock().unwrap();
    if let Some(token) = map.get(token_id) {
        token.store(true, Ordering::Relaxed);
        true
    } else {
        false
    }
}

/// Remove a registered cancellation token. Called when the listing ends.
pub(crate) fn remove_listing_cancel_token(token_id: &str) {
    let mut map = LISTING_CANCELLATIONS.lock().unwrap();
    map.remove(token_id);
}

/// Drop guard for every listing exit path, including op cancellation and a
/// panicking `spawn_blocking` task. Dropping requests cooperative cancellation
/// before withdrawing the public token from the registry.
pub(crate) struct ListingCancellationGuard {
    token_id: String,
    token: ListingCancelToken,
}

impl ListingCancellationGuard {
    pub(crate) fn new(token_id: String, token: ListingCancelToken) -> Self {
        Self { token_id, token }
    }
}

impl Drop for ListingCancellationGuard {
    fn drop(&mut self) {
        self.token.store(true, Ordering::Relaxed);
        remove_listing_cancel_token(&self.token_id);
    }
}

/// Closed set of names and the intentionally small root-ignore grammar.
struct IgnoreSet {
    names: HashSet<String>,
    patterns: Vec<IgnorePattern>,
}

struct IgnorePattern {
    segments: Vec<Vec<char>>,
    matches_path: bool,
    directory_only: bool,
}

impl IgnoreSet {
    fn defaults() -> Self {
        Self {
            names: DEFAULT_IGNORED_NAMES
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            patterns: Vec::new(),
        }
    }

    fn is_ignored(&self, relative_path: &Path, is_directory: bool) -> bool {
        let name = relative_path
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        if self.names.contains(name.as_ref()) {
            return true;
        }
        if self.patterns.is_empty() {
            return false;
        }
        let name: Vec<_> = name.chars().collect();
        let path_segments: Vec<Vec<char>> = relative_path
            .components()
            .map(|component| component.as_os_str().to_string_lossy().chars().collect())
            .collect();
        self.patterns.iter().any(|pattern| {
            if pattern.directory_only && !is_directory {
                return false;
            }
            if !pattern.matches_path {
                return glob_matches(&name, &pattern.segments[0]);
            }
            path_segments.len() == pattern.segments.len()
                && path_segments
                    .iter()
                    .zip(&pattern.segments)
                    .all(|(text, pattern)| glob_matches(text, pattern))
        })
    }
}

fn load_ignore_set(root_path: &Path) -> Result<IgnoreSet, String> {
    let Some(contents) =
        read_auxiliary_file_bounded(&root_path.join(".gitignore"), MAX_AUXILIARY_READ_BYTES)?
    else {
        return Ok(IgnoreSet::defaults());
    };
    build_ignore_set(&contents)
}

fn build_ignore_set(contents: &str) -> Result<IgnoreSet, String> {
    let mut ignore_set = IgnoreSet::defaults();
    for (index, line) in contents.lines().enumerate() {
        let line_number = index + 1;
        if line_number > MAX_GITIGNORE_LINES {
            return Err(format!(
                ".gitignore exceeds the {MAX_GITIGNORE_LINES}-line limit"
            ));
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.chars().count() > MAX_GITIGNORE_PATTERN_CHARS {
            return Err(format!(
                ".gitignore line {line_number} exceeds the {MAX_GITIGNORE_PATTERN_CHARS}-character rule limit"
            ));
        }
        if ignore_set.patterns.len() >= MAX_GITIGNORE_PATTERNS {
            return Err(format!(
                ".gitignore exceeds the {MAX_GITIGNORE_PATTERNS}-rule limit"
            ));
        }
        let directory_only = line.ends_with('/');
        let pattern = line.strip_suffix('/').unwrap_or(line);
        let matches_path = pattern.starts_with('/') || pattern.contains('/');
        let pattern = pattern.strip_prefix('/').unwrap_or(pattern);
        let unsupported = if line.starts_with('!') {
            Some("negation")
        } else if line.contains('\\') {
            Some("escaping")
        } else if line.contains('[') || line.contains(']') {
            Some("character classes")
        } else if pattern.contains("**") {
            Some("double-star patterns")
        } else if pattern.split('/').any(str::is_empty) {
            Some("empty path components")
        } else if pattern.chars().any(char::is_control) {
            Some("control characters")
        } else {
            None
        };
        if let Some(feature) = unsupported {
            return Err(format!(
                ".gitignore line {line_number} uses unsupported {feature}"
            ));
        }
        ignore_set.patterns.push(IgnorePattern {
            segments: pattern
                .split('/')
                .map(|segment| segment.chars().collect())
                .collect(),
            matches_path,
            directory_only,
        });
    }
    Ok(ignore_set)
}

/// Match one path segment in the documented `*`/`?` grammar with greedy-star
/// backtracking. Callers split root-relative path patterns before matching, so
/// wildcards never cross directory separators.
fn glob_matches(text: &[char], pattern: &[char]) -> bool {
    let (mut text_index, mut pattern_index) = (0, 0);
    let (mut star_index, mut star_text_index) = (None, 0);
    while text_index < text.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == '?' || pattern[pattern_index] == text[text_index])
        {
            text_index += 1;
            pattern_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == '*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_text_index = text_index;
        } else if let Some(star) = star_index {
            star_text_index += 1;
            text_index = star_text_index;
            pattern_index = star + 1;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == '*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

/// Read the optional root ignore file through one handle and retain at most
/// `max_bytes + 1`. Missing means no user rules; every other failure aborts
/// listing visibly so invalid rules never broaden traversal.
fn read_auxiliary_file_bounded(path: &Path, max_bytes: usize) -> Result<Option<String>, String> {
    use std::io::Read;
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(".gitignore cannot be opened".to_string()),
    };
    let metadata = file
        .metadata()
        .map_err(|_| ".gitignore metadata cannot be read".to_string())?;
    if !metadata.is_file() {
        return Err(".gitignore is not a regular file".to_string());
    }
    if metadata.len() > max_bytes as u64 {
        return Err(format!(".gitignore exceeds the {max_bytes}-byte limit"));
    }
    let mut bytes = Vec::new();
    file.take((max_bytes as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ".gitignore cannot be read".to_string())?;
    if bytes.len() > max_bytes {
        return Err(format!(".gitignore exceeds the {max_bytes}-byte limit"));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| ".gitignore is not valid UTF-8".to_string())
}

fn count_visible_children(dir_path: &Path, relative_path: &Path, ignore_set: &IgnoreSet) -> usize {
    let read_dir = match fs::read_dir(dir_path) {
        Ok(read_dir) => read_dir,
        Err(_) => return 0,
    };
    let mut count = 0;
    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let is_directory = entry.file_type().is_ok_and(|kind| kind.is_dir());
        if ignore_set.is_ignored(&relative_path.join(&name), is_directory) {
            continue;
        }
        count += 1;
        if count >= MAX_CHILD_COUNT_SCAN {
            return count;
        }
    }
    count
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
            | Self::ReadOnlySave { .. }
            | Self::DocumentLimitExceeded { .. }
            | Self::StaleSaveVersion { .. }
            | Self::StaleFileMetadata { .. }
            | Self::FileTooLarge { .. }
            | Self::RootLimitExceeded => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, LazyLock, Mutex as StdMutex, atomic::AtomicBool},
        time::SystemTime,
    };

    use crate::protocol::{DocumentAccess, EditOperation, FileErrorCode, ServerMessage};

    use super::super::super::perf::budgets::MAX_DOCUMENTS_PER_CLIENT;
    use super::{
        AtomicSaveError, BEFORE_REVALIDATE_HOOKS, BETWEEN_METADATA_AND_READ_HOOKS,
        FileListEntryKind, FileListRequest, FileMetadata, SaveIoOutcome, TEST_TEMP_NAMES,
        TargetIdentity, WorkspaceError, WorkspaceState, atomic_write_file, build_ignore_set,
        open_existing_file_unlocked, save_document_unlocked,
    };
    use tokio::sync::Mutex;

    static CWD_TEST_LOCK: LazyLock<StdMutex<()>> = LazyLock::new(|| StdMutex::new(()));

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
            document.initial_document_message(opened.access.clone(), "/tmp/root".to_string()),
            ServerMessage::InitialDocument {
                document_id: 1,
                version: 1,
                text: "hello 🌎\n".to_string(),
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
                workspace_root: "/tmp/root".to_string(),
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
                .initial_document_message(second.access.clone(), "/tmp/root".to_string()),
            ServerMessage::InitialDocument {
                document_id: first.document_id,
                version: 1,
                text: "fn main() {}\n".to_string(),
                access: DocumentAccess::ReadOnly,
                lease_id: None,
                workspace_root: "/tmp/root".to_string(),
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
    async fn selected_file_open_grants_only_the_selected_file() {
        let root = temp_workspace("selected-single-grant");
        let selected = root.join("note.md");
        let sibling = root.join("sibling.md");
        fs::write(&selected, "# selected\n").unwrap();
        fs::write(&sibling, "# sibling\n").unwrap();
        let mut workspace = WorkspaceState::new();

        let opened = workspace.open_selected_file(&selected, 1).await.unwrap();

        assert_eq!(opened.document_id, 1);
        assert_eq!(
            opened.file_state.workspace_relative_path,
            PathBuf::from("note.md")
        );
        assert_eq!(opened.access, DocumentAccess::Editable { lease_id: 1 });
        assert!(workspace.list_root_metadata().is_empty());

        let sibling_error = workspace
            .open_existing_file(opened.file_state.workspace_root_id, &sibling, 2)
            .await
            .unwrap_err();
        assert!(matches!(sibling_error, WorkspaceError::OutsideRoot));

        let duplicate = workspace.open_selected_file(&selected, 2).await.unwrap();
        assert_eq!(duplicate.document_id, opened.document_id);
        assert_eq!(duplicate.access, DocumentAccess::ReadOnly);

        let _ = fs::remove_file(selected);
        let _ = fs::remove_file(sibling);
        let _ = fs::remove_dir(root);
    }

    #[tokio::test]
    async fn selected_file_open_rejects_directory_and_invalid_utf8_without_document_entry() {
        let root = temp_workspace("selected-rejections");
        let directory = root.join("folder");
        let invalid = root.join("bad.md");
        fs::create_dir(&directory).unwrap();
        fs::write(&invalid, [0xff, 0xfe, b'x']).unwrap();
        let mut workspace = WorkspaceState::new();

        let directory_error = workspace
            .open_selected_file(&directory, 1)
            .await
            .unwrap_err();
        let invalid_error = workspace.open_selected_file(&invalid, 1).await.unwrap_err();

        assert!(matches!(directory_error, WorkspaceError::DirectoryOpen));
        assert!(matches!(invalid_error, WorkspaceError::InvalidUtf8 { .. }));
        assert!(workspace.documents.is_empty());
        assert!(workspace.path_to_document.is_empty());
        assert!(workspace.list_root_metadata().is_empty());

        let _ = fs::remove_file(invalid);
        let _ = fs::remove_dir(directory);
        let _ = fs::remove_dir(root);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn selected_file_open_rejects_special_file_without_document_entry() {
        let root = temp_workspace("selected-special-file");
        let socket = root.join("document.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let mut workspace = WorkspaceState::new();

        let error = workspace.open_selected_file(&socket, 1).await.unwrap_err();

        assert!(matches!(error, WorkspaceError::UnsupportedFileType));
        assert!(workspace.documents.is_empty());
        assert!(workspace.path_to_document.is_empty());
        assert!(workspace.list_root_metadata().is_empty());

        drop(listener);
        let _ = fs::remove_file(socket);
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

        let saved = workspace
            .save_document(opened.document_id, 7, 2)
            .await
            .unwrap();

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

        workspace
            .save_document(opened.document_id, 9, 2)
            .await
            .unwrap();

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
            .reload_document(opened.document_id, 3, false)
            .await
            .unwrap_err();
        assert!(matches!(rejected, WorkspaceError::DirtyDocument { .. }));
        assert_eq!(opened.document.lock().await.text(), "disk dirty");

        let reloaded = workspace
            .reload_document(opened.document_id, 3, true)
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
            .reload_document(opened.document_id, 4, false)
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
            .save_document(opened.document_id, 5, 2)
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
            .save_document(opened.document_id, 6, 2)
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
            .save_document(opened.document_id, 8, 2)
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

    /// A file larger than `MAX_OPENABLE_FILE_BYTES` is rejected with a typed
    /// `FileTooLarge` error before the bounded read allocates.
    #[tokio::test]
    async fn open_existing_file_rejects_oversized_file() {
        let root = temp_workspace("oversized-open");
        let file = root.join("big.txt");
        // One byte over the limit. Use a repeated ASCII byte so the size in
        // bytes equals the length, regardless of UTF-8.
        let oversized = "!".repeat(crate::perf::budgets::MAX_OPENABLE_FILE_BYTES + 1);
        fs::write(&file, &oversized).unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let error = workspace
            .open_existing_file(root_id, "big.txt", 1)
            .await
            .unwrap_err();
        let diagnostic = error.diagnostic();

        assert_eq!(diagnostic.code, FileErrorCode::FileTooLarge);
        assert!(diagnostic.message.contains("exceeds the"));
        assert!(diagnostic.message.contains("openable-file limit"));
        // No document was registered for the rejected file.
        assert!(workspace.document_handle(1).is_none());

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    /// The selected-file open path also enforces the openable-file size gate
    /// before reading the picked file from disk.
    #[tokio::test]
    async fn open_selected_file_rejects_oversized_file() {
        let root = temp_workspace("oversized-selected");
        let file = root.join("picked.md");
        let oversized = "!".repeat(crate::perf::budgets::MAX_OPENABLE_FILE_BYTES + 1);
        fs::write(&file, &oversized).unwrap();
        let mut workspace = WorkspaceState::new();

        let error = workspace.open_selected_file(&file, 1).await.unwrap_err();
        let diagnostic = error.diagnostic();

        assert_eq!(diagnostic.code, FileErrorCode::FileTooLarge);
        assert!(workspace.document_handle(1).is_none());

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    /// Reloading a document whose file grew past the limit fails with
    /// `FileTooLarge` before re-reading the full contents.
    #[tokio::test]
    async fn reload_document_rejects_oversized_file() {
        let root = temp_workspace("oversized-reload");
        let file = root.join("note.txt");
        fs::write(&file, "small").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .open_existing_file(root_id, "note.txt", 1)
            .await
            .unwrap();

        // Grow the file past the limit on disk after it was opened.
        let oversized = "!".repeat(crate::perf::budgets::MAX_OPENABLE_FILE_BYTES + 1);
        fs::write(&file, &oversized).unwrap();

        let error = workspace
            .reload_document(opened.document_id, 1, true)
            .await
            .unwrap_err();
        let diagnostic = error.diagnostic();
        assert_eq!(diagnostic.code, FileErrorCode::FileTooLarge);

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    /// A file exactly at the limit opens successfully (boundary is inclusive).
    #[tokio::test]
    async fn open_file_at_limit_boundary_succeeds() {
        let root = temp_workspace("limit-boundary");
        let file = root.join("at_limit.txt");
        let at_limit = "a".repeat(crate::perf::budgets::MAX_OPENABLE_FILE_BYTES);
        fs::write(&file, &at_limit).unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let opened = workspace
            .open_existing_file(root_id, "at_limit.txt", 1)
            .await;
        assert!(opened.is_ok(), "file exactly at the limit must open");

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    /// Two concurrent saves of *different* documents must complete without
    /// serializing on the workspace mutex during disk writes. This exercises the
    /// `save_document_unlocked` orchestration: each save holds the workspace
    /// mutex only for the fast `prepare_save` phase, releases it across the
    /// `tokio::fs::write`, then reacquires to `commit_save`.
    #[tokio::test]
    async fn concurrent_save_different_documents() {
        let root = temp_workspace("concurrent-save-diff");
        let file_a = root.join("a.txt");
        let file_b = root.join("b.txt");
        fs::write(&file_a, "alpha").unwrap();
        fs::write(&file_b, "beta").unwrap();
        let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
        let root_id = workspace.lock().await.add_root(&root).unwrap();

        let opened_a = open_existing_file_unlocked(&workspace, root_id, "a.txt", 1)
            .await
            .unwrap();
        let opened_b = open_existing_file_unlocked(&workspace, root_id, "b.txt", 2)
            .await
            .unwrap();

        // Mark both dirty so save actually writes new content.
        {
            opened_a
                .document
                .lock()
                .await
                .replace_text_from_storage("alpha-edited".to_string());
            opened_b
                .document
                .lock()
                .await
                .replace_text_from_storage("beta-edited".to_string());
        }

        let ws_a = Arc::clone(&workspace);
        let ws_b = Arc::clone(&workspace);
        let save_a =
            tokio::spawn(
                async move { save_document_unlocked(&ws_a, opened_a.document_id, 1, 1).await },
            );
        let save_b =
            tokio::spawn(
                async move { save_document_unlocked(&ws_b, opened_b.document_id, 2, 1).await },
            );
        let (outcome_a, outcome_b) = tokio::join!(save_a, save_b);
        let outcome_a = outcome_a.unwrap().unwrap();
        let outcome_b = outcome_b.unwrap().unwrap();
        assert!(
            !outcome_a.dirty,
            "clean save of doc a must report not-dirty"
        );
        assert!(
            !outcome_b.dirty,
            "clean save of doc b must report not-dirty"
        );

        assert_eq!(fs::read_to_string(&file_a).unwrap(), "alpha-edited");
        assert_eq!(fs::read_to_string(&file_b).unwrap(), "beta-edited");

        let _ = fs::remove_file(file_a);
        let _ = fs::remove_file(file_b);
        let _ = fs::remove_dir(root);
    }

    /// If the document is edited during the unlocked write window, `commit_save`
    /// must detect the version mismatch via `mark_clean_if_version` and leave
    /// the document dirty instead of falsely marking it clean. This is the
    /// "re-validate on reacquire" contract for the released-mutex I/O path.
    #[tokio::test]
    async fn save_version_mismatch_after_io_leaves_document_dirty() {
        let root = temp_workspace("save-version-mismatch");
        let file = root.join("note.txt");
        fs::write(&file, "original").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .open_existing_file(root_id, "note.txt", 1)
            .await
            .unwrap();
        let document_id = opened.document_id;

        let prepared_version = opened.document.lock().await.version();
        let plan = workspace.prepare_save(document_id).unwrap();

        // Simulate a concurrent edit landing during the unlocked write: bump the
        // in-memory version past the version captured at write time.
        opened
            .document
            .lock()
            .await
            .replace_text_from_storage("concurrent edit".to_string());

        // The I/O phase captured the pre-edit version; commit must notice the
        // mismatch and refuse to mark the document clean.
        let io = SaveIoOutcome {
            prepared_version,
            saved_metadata: FileMetadata {
                len: 0,
                modified: None,
            },
        };
        let outcome = workspace.commit_save(plan, io).await.unwrap();
        assert!(
            outcome.dirty,
            "a concurrent edit during save must not be falsely marked clean"
        );

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    /// Two concurrent opens of the *same* canonical path through the unlocked
    /// orchestration must not create duplicate document registry entries. The
    /// slow reader that commits second re-checks the canonical path under the
    /// workspace mutex and returns the existing lease instead of inserting a
    /// duplicate.
    #[tokio::test]
    async fn concurrent_open_same_file_dedups_registry() {
        let root = temp_workspace("concurrent-open-dedup");
        let file = root.join("shared.txt");
        fs::write(&file, "shared body").unwrap();
        let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
        let root_id = workspace.lock().await.add_root(&root).unwrap();

        let ws_a = Arc::clone(&workspace);
        let ws_b = Arc::clone(&workspace);
        let open_a = tokio::spawn(async move {
            open_existing_file_unlocked(&ws_a, root_id, "shared.txt", 1).await
        });
        let open_b = tokio::spawn(async move {
            open_existing_file_unlocked(&ws_b, root_id, "shared.txt", 2).await
        });
        let (lease_a, lease_b) = tokio::join!(open_a, open_b);
        let lease_a = lease_a.unwrap().unwrap();
        let lease_b = lease_b.unwrap().unwrap();

        assert_eq!(
            lease_a.document_id, lease_b.document_id,
            "concurrent opens of the same file must resolve to one document id"
        );
        // Exactly one registry entry exists for the path.
        let ws = workspace.lock().await;
        assert!(ws.document_handle(lease_a.document_id).is_some());
        let _ = ws; // release

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    /// Atomic save writes to a temp file in the same directory, fsyncs, then
    /// renames over the target. On success the target holds the new content and
    /// no `.clay-save-*` temp file is left behind.
    #[tokio::test]
    async fn atomic_save_replaces_target_and_leaves_no_temp() {
        let root = temp_workspace("atomic-save-replace");
        let file = root.join("note.txt");
        fs::write(&file, "original").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .open_existing_file(root_id, "note.txt", 9)
            .await
            .unwrap();

        opened
            .document
            .lock()
            .await
            .replace_text_from_storage("replaced atomically".to_string());
        workspace
            .save_document(opened.document_id, 9, 1)
            .await
            .unwrap();

        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "replaced atomically",
            "target file must hold the new content after atomic save"
        );
        // No leftover temp files in the directory.
        let leftover_temps = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".clay-save-"))
            .count();
        assert_eq!(
            leftover_temps, 0,
            "atomic save must rename the temp over the target, leaving no temp behind"
        );

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    /// If the write fails (here: the target directory is not writable so the
    /// temp file cannot be created), the original file is left intact and the
    /// save returns a typed `WriteFailed` error.
    #[cfg(unix)]
    #[tokio::test]
    async fn atomic_save_preserves_original_on_write_failure() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_workspace("atomic-save-failure");
        let file = root.join("note.txt");
        fs::write(&file, "untouched").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        let opened = workspace
            .open_existing_file(root_id, "note.txt", 9)
            .await
            .unwrap();
        opened
            .document
            .lock()
            .await
            .replace_text_from_storage("should never be written".to_string());

        // Make the directory non-writable so the atomic-save temp file cannot
        // be created. The original file stays readable (r-- on the file, r-x on
        // the dir) so reauthorization still succeeds before the write fails.
        let original_dir_mode = fs::metadata(&root).unwrap().permissions().mode();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o555)).unwrap();

        let error = workspace
            .save_document(opened.document_id, 9, 1)
            .await
            .unwrap_err();
        assert!(
            matches!(error, WorkspaceError::WriteFailed { .. }),
            "save to a non-writable directory must fail with WriteFailed, got {error:?}"
        );
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "untouched",
            "original file content must be preserved when the atomic save fails"
        );
        // No partial temp file leaked into the non-writable directory.
        let leaked_temps = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".clay-save-"))
            .count();
        assert_eq!(leaked_temps, 0);

        // Restore writability so cleanup can remove the directory.
        fs::set_permissions(&root, fs::Permissions::from_mode(original_dir_mode)).unwrap();
        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    /// Portable rename-failure exercise for the atomic-save helper: renaming a
    /// file over an existing directory fails on every platform, so the helper
    /// must return the error and remove the orphaned temp file rather than
    /// leaving a torn write or litter. Runs on Windows and Unix.
    #[tokio::test]
    async fn atomic_write_file_rename_failure_returns_error_and_cleans_temp() {
        let root = temp_workspace("atomic-rename-fail");
        // `blocker` is a directory, so `rename(temp, blocker)` cannot succeed.
        let blocker = root.join("blocker");
        fs::create_dir(&blocker).unwrap();

        let error = atomic_write_file(&blocker, b"new content", None).await;
        assert!(
            error.is_err(),
            "renaming a temp file over a directory must fail, not silently succeed"
        );

        let leaked_temps = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".clay-save-"))
            .count();
        assert_eq!(
            leaked_temps, 0,
            "a failed rename must remove the temp file, leaving no litter"
        );

        let _ = fs::remove_dir(&blocker);
        let _ = fs::remove_dir(root);
    }

    /// A file that passes the prepare-time and handle-metadata size checks but
    /// grows before the contents are read must still be bounded: the read
    /// allocates at most the ceiling plus one byte and rejects with
    /// `FileTooLarge` instead of opening the oversized file (Plan 060 T5).
    #[tokio::test]
    async fn bounded_read_rejects_file_that_grows_between_validation_and_read() {
        let root = temp_workspace("grow-during-read");
        let file = root.join("grow.txt");
        let at_limit = "a".repeat(crate::perf::budgets::MAX_OPENABLE_FILE_BYTES);
        fs::write(&file, &at_limit).unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let grow_path = file.clone();
        BETWEEN_METADATA_AND_READ_HOOKS
            .lock()
            .expect("hook lock")
            .push((
                grow_path.clone(),
                Box::new(move || {
                    fs::write(&grow_path, format!("{at_limit}b")).unwrap();
                }),
            ));

        let error = workspace
            .open_existing_file(root_id, "grow.txt", 1)
            .await
            .unwrap_err();
        assert_eq!(error.diagnostic().code, FileErrorCode::FileTooLarge);
        assert!(workspace.document_handle(1).is_none());

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    /// A pre-created file at a predicted temp name must not be truncated or
    /// followed: the exclusive create collides, the save retries with a fresh
    /// unpredictable name, and the staged file is left untouched (Plan 060 T5).
    #[tokio::test]
    async fn atomic_save_ignores_precreated_temp_file() {
        let root = temp_workspace("precreated-temp");
        let target = root.join("target.txt");
        fs::write(&target, "original").unwrap();
        let staged = root.join("staged-collision");
        fs::write(&staged, "sentinel").unwrap();
        TEST_TEMP_NAMES
            .lock()
            .expect("temp-name queue lock")
            .push((root.clone(), "staged-collision".to_string()));

        let saved = atomic_write_file(&target, b"new content", None)
            .await
            .expect("collision with a pre-created temp must retry with a fresh name");
        assert_eq!(saved.len(), 11);
        assert_eq!(fs::read_to_string(&target).unwrap(), "new content");
        assert_eq!(
            fs::read_to_string(&staged).unwrap(),
            "sentinel",
            "pre-created temp file must never be truncated or overwritten"
        );

        let _ = fs::remove_file(target);
        let _ = fs::remove_file(staged);
        let _ = fs::remove_dir(root);
    }

    /// A pre-created symlink at a predicted temp name must not be followed:
    /// the exclusive create refuses it and the symlink's victim keeps its
    /// contents (Plan 060 T5).
    #[cfg(unix)]
    #[tokio::test]
    async fn atomic_save_does_not_follow_precreated_temp_symlink() {
        let root = temp_workspace("symlink-temp");
        let target = root.join("target.txt");
        fs::write(&target, "original").unwrap();
        let victim = root.join("victim.txt");
        fs::write(&victim, "victim-sentinel").unwrap();
        std::os::unix::fs::symlink(&victim, root.join("staged-symlink")).unwrap();
        TEST_TEMP_NAMES
            .lock()
            .expect("temp-name queue lock")
            .push((root.clone(), "staged-symlink".to_string()));

        atomic_write_file(&target, b"new content", None)
            .await
            .expect("symlink collision must retry with a fresh name");
        assert_eq!(fs::read_to_string(&target).unwrap(), "new content");
        assert_eq!(
            fs::read_to_string(&victim).unwrap(),
            "victim-sentinel",
            "the temp symlink's target must never be written through"
        );

        let _ = fs::remove_file(root.join("staged-symlink"));
        let _ = fs::remove_file(target);
        let _ = fs::remove_file(victim);
        let _ = fs::remove_dir(root);
    }

    /// Collision retries are bounded: when every forced temp name is
    /// pre-created the save fails closed, leaves the target untouched, and
    /// removes nothing it did not create (Plan 060 T5).
    #[tokio::test]
    async fn atomic_save_fails_closed_on_bounded_collision_exhaustion() {
        let root = temp_workspace("collision-exhaustion");
        let target = root.join("target.txt");
        fs::write(&target, "original").unwrap();
        {
            let mut queue = TEST_TEMP_NAMES.lock().expect("temp-name queue lock");
            // Queue is popped one per attempt; push all 8 collision names.
            for index in 0..8 {
                let name = format!("collision-{index}");
                fs::write(root.join(&name), "staged").unwrap();
                queue.push((root.clone(), name));
            }
        }

        let error = atomic_write_file(&target, b"new content", None)
            .await
            .expect_err("every temp name colliding must exhaust the bounded retry");
        assert!(matches!(error, AtomicSaveError::Io(_)));
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "original",
            "target must be unchanged when temp creation never succeeds"
        );

        let _ = fs::remove_file(target);
        for index in 0..8 {
            let _ = fs::remove_file(root.join(format!("collision-{index}")));
        }
        let _ = fs::remove_dir(root);
    }

    /// An external atomic replacement of the target (new inode, as other
    /// editors do) during the temp write must fail the save closed instead of
    /// silently clobbering the replacement (Plan 060 T5).
    #[tokio::test]
    async fn atomic_save_fails_closed_when_target_is_replaced_before_rename() {
        let root = temp_workspace("target-replaced");
        let target = root.join("target.txt");
        fs::write(&target, "original").unwrap();
        let replacement = root.join("replacement.txt");
        fs::write(&replacement, "external replacement").unwrap();
        let identity = TargetIdentity::capture(&target).unwrap();

        let hook_root = root.clone();
        BEFORE_REVALIDATE_HOOKS.lock().expect("hook lock").push((
            target.clone(),
            Box::new(move || {
                fs::rename(
                    hook_root.join("replacement.txt"),
                    hook_root.join("target.txt"),
                )
                .unwrap();
            }),
        ));

        let error = atomic_write_file(&target, b"editor content", Some(&identity))
            .await
            .expect_err("target replacement during save must fail closed");
        assert!(matches!(error, AtomicSaveError::TargetChanged));
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "external replacement",
            "the external replacement must be preserved, not clobbered"
        );
        let leaked_temps = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".clay-save-"))
            .count();
        assert_eq!(leaked_temps, 0, "failed save must clean up its temp file");

        let _ = fs::remove_file(target);
        let _ = fs::remove_dir(root);
    }

    /// A same-length external edit during the temp write changes `modified`
    /// and must fail the save closed, preserving the external bytes (Plan 060
    /// T5). This is the case pure size checks cannot catch.
    #[tokio::test]
    async fn atomic_save_fails_closed_on_same_length_external_edit() {
        let root = temp_workspace("same-length-edit");
        let target = root.join("target.txt");
        fs::write(&target, "aaaa").unwrap();
        let identity = TargetIdentity::capture(&target).unwrap();

        let edit_path = target.clone();
        BEFORE_REVALIDATE_HOOKS.lock().expect("hook lock").push((
            target.clone(),
            Box::new(move || {
                fs::write(&edit_path, "bbbb").unwrap();
            }),
        ));

        let error = atomic_write_file(&target, b"cccc", Some(&identity))
            .await
            .expect_err("same-length external edit must fail closed");
        assert!(matches!(error, AtomicSaveError::TargetChanged));
        assert_eq!(fs::read_to_string(&target).unwrap(), "bbbb");

        let _ = fs::remove_file(target);
        let _ = fs::remove_dir(root);
    }

    /// End-to-end: a target modified between `prepare_save`'s staleness check
    /// and the atomic replace surfaces the typed staleness error (not a
    /// generic write failure) and preserves the external bytes (Plan 060 T5).
    #[tokio::test]
    async fn save_document_reports_stale_when_target_changes_during_write() {
        let root = temp_workspace("stale-during-write");
        let file = root.join("doc.txt");
        fs::write(&file, "original").unwrap();
        let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
        let root_id = workspace.lock().await.add_root(&root).unwrap();
        let opened = open_existing_file_unlocked(&workspace, root_id, "doc.txt", 1)
            .await
            .unwrap();
        opened
            .document
            .lock()
            .await
            .replace_text_from_storage("editor content".to_string());

        let edit_path = file.clone();
        BEFORE_REVALIDATE_HOOKS.lock().expect("hook lock").push((
            file.clone(),
            Box::new(move || {
                fs::write(&edit_path, "external edit").unwrap();
            }),
        ));

        let error = save_document_unlocked(&workspace, opened.document_id, 1, 1)
            .await
            .unwrap_err();
        assert_eq!(error.diagnostic().code, FileErrorCode::StaleFileMetadata);
        assert_eq!(fs::read_to_string(&file).unwrap(), "external edit");

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    /// Unix: a brand-new saved file starts owner-only (`0o600`) because the
    /// exclusive temp does, and a save of an existing file preserves its
    /// original mode across the atomic replace (Plan 060 T5).
    #[cfg(unix)]
    #[tokio::test]
    async fn atomic_save_starts_owner_only_and_preserves_existing_mode() {
        let root = temp_workspace("save-modes");
        let new_file = root.join("new.txt");
        atomic_write_file(&new_file, b"fresh", None).await.unwrap();
        let new_mode = fs::metadata(&new_file).unwrap().permissions().mode() & 0o777;
        assert_eq!(new_mode, 0o600, "new file must start owner-only");

        let existing = root.join("existing.txt");
        fs::write(&existing, "old").unwrap();
        fs::set_permissions(&existing, fs::Permissions::from_mode(0o640)).unwrap();
        let identity = TargetIdentity::capture(&existing).unwrap();
        atomic_write_file(&existing, b"updated", Some(&identity))
            .await
            .unwrap();
        let kept_mode = fs::metadata(&existing).unwrap().permissions().mode() & 0o777;
        assert_eq!(kept_mode, 0o640, "existing mode must survive the replace");

        let _ = fs::remove_file(new_file);
        let _ = fs::remove_file(existing);
        let _ = fs::remove_dir(root);
    }

    /// Per-client open-document ceiling: the 65th open by one client fails
    /// closed with `WorkspaceLimitExceeded` while another client is
    /// unaffected (Plan 060 T6, P1-4).
    #[tokio::test]
    async fn open_documents_enforce_per_client_ceiling() {
        let root = temp_workspace("client-ceiling");
        let total = MAX_DOCUMENTS_PER_CLIENT + 1;
        for index in 0..total {
            fs::write(root.join(format!("doc-{index}.txt")), "x").unwrap();
        }
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();
        for index in 0..MAX_DOCUMENTS_PER_CLIENT {
            workspace
                .open_existing_file(root_id, format!("doc-{index}.txt"), 1)
                .await
                .unwrap_or_else(|error| panic!("open {index} must succeed: {error}"));
        }
        let error = workspace
            .open_existing_file(root_id, format!("doc-{}.txt", MAX_DOCUMENTS_PER_CLIENT), 1)
            .await
            .unwrap_err();
        assert_eq!(
            error.diagnostic().code,
            FileErrorCode::WorkspaceLimitExceeded
        );
        // A different client has its own budget.
        workspace
            .open_existing_file(root_id, format!("doc-{}.txt", MAX_DOCUMENTS_PER_CLIENT), 2)
            .await
            .expect("other client must have its own budget");
        for index in 0..total {
            let _ = fs::remove_file(root.join(format!("doc-{index}.txt")));
        }
        let _ = fs::remove_dir(root);
    }

    /// Explicit close: dirty close requires `force`, a non-holder fails closed
    /// as `UnknownDocument`, the last holder's close removes the registry
    /// entry, and a shared document survives until its final holder closes
    /// (Plan 060 T6, P1-4).
    #[tokio::test]
    async fn close_document_lifecycle() {
        let root = temp_workspace("close-lifecycle");
        fs::write(root.join("a.txt"), "alpha").unwrap();
        fs::write(root.join("shared.txt"), "shared").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        // Dirty close requires force.
        let opened = workspace
            .open_existing_file(root_id, "a.txt", 1)
            .await
            .unwrap();
        {
            let mut document = opened.document.lock().await;
            let access = document.access_for_client(1);
            let crate::protocol::DocumentAccess::Editable { lease_id } = access else {
                panic!("opener must hold the editable lease");
            };
            let (response, _) = document.apply_edit_with_parse_input(
                opened.document_id,
                1,
                Some(lease_id),
                1,
                1,
                crate::protocol::EditOperation::Insert {
                    byte_offset: 0,
                    text: "dirty ".to_string(),
                },
            );
            assert!(matches!(response, ServerMessage::EditAck { .. }));
        }
        let error = workspace
            .close_document(opened.document_id, 1, false)
            .await
            .unwrap_err();
        assert_eq!(error.diagnostic().code, FileErrorCode::DirtyDocument);
        assert!(workspace.document_handle(opened.document_id).is_some());

        // Non-holder close fails closed like an unknown document.
        let error = workspace
            .close_document(opened.document_id, 2, true)
            .await
            .unwrap_err();
        assert_eq!(error.diagnostic().code, FileErrorCode::UnknownDocument);

        // Forced close by the only holder removes the registry entry.
        let outcome = workspace
            .close_document(opened.document_id, 1, true)
            .await
            .unwrap();
        assert!(outcome.closed);
        assert!(workspace.document_handle(opened.document_id).is_none());

        // Shared document: first close releases only that holder.
        let shared = workspace
            .open_existing_file(root_id, "shared.txt", 1)
            .await
            .unwrap();
        workspace
            .open_existing_file(root_id, "shared.txt", 2)
            .await
            .unwrap();
        let first = workspace
            .close_document(shared.document_id, 1, false)
            .await
            .unwrap();
        assert!(!first.closed);
        assert!(workspace.document_handle(shared.document_id).is_some());
        let second = workspace
            .close_document(shared.document_id, 2, false)
            .await
            .unwrap();
        assert!(second.closed);
        assert!(workspace.document_handle(shared.document_id).is_none());

        // Disconnect release finalizes documents with no remaining holders.
        let reopened = workspace
            .open_existing_file(root_id, "a.txt", 7)
            .await
            .unwrap();
        let finalized = workspace.release_client_access(7).await;
        assert_eq!(finalized.len(), 1);
        assert_eq!(finalized[0].0, reopened.document_id);
        assert!(workspace.document_handle(reopened.document_id).is_none());

        let _ = fs::remove_file(root.join("a.txt"));
        let _ = fs::remove_file(root.join("shared.txt"));
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn bounded_ignore_grammar_supports_root_paths_and_rejects_unsupported_rules() {
        let rules = build_ignore_set("*ab\n?.rs\nbuild/\n").unwrap();
        assert!(rules.is_ignored(std::path::Path::new("aab"), false));
        assert!(rules.is_ignored(std::path::Path::new("é.rs"), false));
        assert!(!rules.is_ignored(std::path::Path::new("ab.rs"), false));
        assert!(rules.is_ignored(std::path::Path::new("build"), true));
        assert!(!rules.is_ignored(std::path::Path::new("build"), false));

        let path_rules = build_ignore_set("/cache\n/packages/markdown/node_modules/\n").unwrap();
        assert!(path_rules.is_ignored(std::path::Path::new("cache"), true));
        assert!(
            path_rules.is_ignored(std::path::Path::new("packages/markdown/node_modules"), true)
        );
        assert!(!path_rules.is_ignored(std::path::Path::new("src/cache"), true));

        for unsupported in [
            "!secret", "foo\\*", "[xy]", "foo//", "**.log", "/", "foo\0bar",
        ] {
            assert!(
                build_ignore_set(unsupported).is_err(),
                "unsupported rule was accepted: {unsupported}"
            );
        }
    }

    #[test]
    fn ignore_rule_line_pattern_and_character_counts_are_bounded() {
        let too_many_lines = "#\n".repeat(crate::perf::budgets::MAX_GITIGNORE_LINES + 1);
        assert!(build_ignore_set(&too_many_lines).is_err());

        let too_many_patterns = "x\n".repeat(crate::perf::budgets::MAX_GITIGNORE_PATTERNS + 1);
        assert!(build_ignore_set(&too_many_patterns).is_err());

        let too_long = "x".repeat(crate::perf::budgets::MAX_GITIGNORE_PATTERN_CHARS + 1);
        assert!(build_ignore_set(&too_long).is_err());
    }

    #[test]
    fn add_root_deduplicates_by_canonical_path() {
        let root = temp_workspace("dedup-root");
        let mut workspace = WorkspaceState::new();
        let first = workspace.add_root(&root).unwrap();
        let second = workspace.add_root(&root).unwrap();
        assert_eq!(first, second);
        assert_eq!(workspace.list_root_metadata().len(), 1);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn add_root_from_cwd_adds_current_directory_when_no_roots_exist() {
        let _guard = CWD_TEST_LOCK.lock().unwrap();
        let root = temp_workspace("cwd-root");
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(&root).unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root_from_cwd().unwrap();
        assert!(root_id.is_some());
        assert_eq!(workspace.list_root_metadata().len(), 1);
        std::env::set_current_dir(previous).unwrap();
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn add_root_from_cwd_is_noop_when_roots_already_configured() {
        let _guard = CWD_TEST_LOCK.lock().unwrap();
        let root = temp_workspace("cwd-root-noop");
        let other = temp_workspace("cwd-root-noop-other");
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(&root).unwrap();
        let mut workspace = WorkspaceState::new();
        let configured = workspace.add_root(&other).unwrap();
        let cwd = workspace.add_root_from_cwd().unwrap();
        assert_eq!(cwd, None);
        assert_eq!(workspace.list_root_metadata().len(), 1);
        assert_eq!(
            workspace.list_root_metadata()[0].workspace_root_id,
            configured
        );
        std::env::set_current_dir(previous).unwrap();
        let _ = fs::remove_dir(root);
        let _ = fs::remove_dir(other);
    }

    #[test]
    fn discover_root_for_path_finds_marker_ancestor() {
        let root = temp_workspace("discover-marker");
        fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        let nested = root.join("src").join("nested");
        fs::create_dir_all(&nested).unwrap();
        let file = nested.join("lib.rs");
        fs::write(&file, "fn main() {}").unwrap();

        let mut workspace = WorkspaceState::new();
        let root_id = workspace.discover_root_for_path(&file).unwrap();
        assert!(root_id.is_some());
        assert_eq!(workspace.list_root_metadata().len(), 1);
        assert!(
            workspace.list_root_metadata()[0]
                .display_name
                .contains("discover-marker")
        );

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn discover_root_for_path_returns_existing_root_when_already_covered() {
        let root = temp_workspace("discover-covered");
        let nested = root.join("deep");
        fs::create_dir_all(&nested).unwrap();
        let file = nested.join("note.txt");
        fs::write(&file, "hello").unwrap();

        let mut workspace = WorkspaceState::new();
        let existing = workspace.add_root(&root).unwrap();
        let discovered = workspace.discover_root_for_path(&file).unwrap();
        assert_eq!(discovered, Some(existing));
        assert_eq!(workspace.list_root_metadata().len(), 1);

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn discover_root_for_path_without_marker_returns_none() {
        let root = temp_workspace("discover-no-marker");
        let file = root.join("note.txt");
        fs::write(&file, "hello").unwrap();

        let mut workspace = WorkspaceState::new();
        let discovered = workspace.discover_root_for_path(&file).unwrap();
        assert_eq!(discovered, None);
        assert!(workspace.list_root_metadata().is_empty());

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn discover_root_for_path_ignores_unknown_marker() {
        let root = temp_workspace("discover-unknown-marker");
        fs::write(root.join("myproject.marker"), "").unwrap();
        let file = root.join("note.txt");
        fs::write(&file, "hello").unwrap();

        let mut workspace = WorkspaceState::new();
        let discovered = workspace.discover_root_for_path(&file).unwrap();
        assert_eq!(discovered, None);

        let _ = fs::remove_file(file);
        let _ = fs::remove_file(root.join("myproject.marker"));
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn explicit_user_grant_adds_directory_root() {
        let root = temp_workspace("grant-dir");
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_explicit_user_grant(&root).unwrap();
        assert_eq!(workspace.list_root_metadata().len(), 1);
        assert_eq!(workspace.list_root_metadata()[0].workspace_root_id, root_id);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn explicit_user_grant_adds_file_as_single_file_grant() {
        let root = temp_workspace("grant-file");
        let file = root.join("note.md");
        fs::write(&file, "# note").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_explicit_user_grant(&file).unwrap();
        // Single-file grants are not listed by list_root_metadata.
        assert!(workspace.list_root_metadata().is_empty());
        assert_eq!(root_id, 1);
        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn explicit_user_grant_deduplicates_single_file_grant() {
        let root = temp_workspace("grant-file-dedup");
        let file = root.join("note.md");
        fs::write(&file, "# note").unwrap();
        let mut workspace = WorkspaceState::new();
        let first = workspace.add_explicit_user_grant(&file).unwrap();
        let second = workspace.add_explicit_user_grant(&file).unwrap();
        assert_eq!(first, second);
        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn explicit_user_grant_rejects_missing_path() {
        let root = temp_workspace("grant-missing");
        let missing = root.join("missing");
        let mut workspace = WorkspaceState::new();
        let error = workspace.add_explicit_user_grant(&missing).unwrap_err();
        assert!(matches!(error, WorkspaceError::RootUnavailable { .. }));
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn discover_root_for_path_rejects_directory() {
        let root = temp_workspace("discover-dir");
        let mut workspace = WorkspaceState::new();
        let error = workspace.discover_root_for_path(&root).unwrap_err();
        assert!(matches!(error, WorkspaceError::DirectoryOpen));
        let _ = fs::remove_dir(root);
    }

    #[test]
    fn list_directory_returns_immediate_children() {
        let root = temp_workspace("list-children");
        fs::write(root.join("a.txt"), "a").unwrap();
        fs::write(root.join("b.md"), "b").unwrap();
        fs::create_dir(root.join("dir")).unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let page = workspace
            .list_directory(
                FileListRequest {
                    root_id,
                    relative_path: PathBuf::new(),
                    max_depth: 1,
                    max_entries: 100,
                },
                None,
            )
            .unwrap();

        assert_eq!(page.root_id, root_id);
        assert!(!page.truncated);
        assert!(!page.cancelled);
        let names: Vec<_> = page.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.txt"));
        assert!(names.contains(&"b.md"));
        assert!(names.contains(&"dir"));
        let dir = page.entries.iter().find(|e| e.name == "dir").unwrap();
        assert_eq!(dir.kind, FileListEntryKind::Directory);
        assert_eq!(dir.child_count, Some(0));
        let file = page.entries.iter().find(|e| e.name == "a.txt").unwrap();
        assert_eq!(file.kind, FileListEntryKind::File);
        assert_eq!(file.size_hint, Some(1));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_directory_respects_max_depth() {
        let root = temp_workspace("list-depth");
        fs::create_dir_all(root.join("d1").join("d2")).unwrap();
        fs::write(root.join("d1").join("f.txt"), "x").unwrap();
        fs::write(root.join("d1").join("d2").join("g.txt"), "y").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let page = workspace
            .list_directory(
                FileListRequest {
                    root_id,
                    relative_path: PathBuf::new(),
                    max_depth: 1,
                    max_entries: 100,
                },
                None,
            )
            .unwrap();

        let names: Vec<_> = page.entries.iter().map(|e| e.name.clone()).collect();
        assert!(names.contains(&"d1".to_string()));
        assert!(!names.iter().any(|n| n == "d2"));
        assert!(!names.iter().any(|n| n == "f.txt"));
        assert!(!names.iter().any(|n| n == "g.txt"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_directory_truncates_at_max_entries() {
        let root = temp_workspace("list-truncate");
        for index in 0..5 {
            fs::write(root.join(format!("{index}.txt")), "x").unwrap();
        }
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let page = workspace
            .list_directory(
                FileListRequest {
                    root_id,
                    relative_path: PathBuf::new(),
                    max_depth: 1,
                    max_entries: 2,
                },
                None,
            )
            .unwrap();

        assert_eq!(page.entries.len(), 2);
        assert!(page.truncated);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_directory_ignores_default_ignored_names() {
        let root = temp_workspace("list-ignore");
        fs::create_dir(root.join(".git")).unwrap();
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::create_dir(root.join("target")).unwrap();
        fs::create_dir(root.join("src")).unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let page = workspace
            .list_directory(
                FileListRequest {
                    root_id,
                    relative_path: PathBuf::new(),
                    max_depth: 1,
                    max_entries: 100,
                },
                None,
            )
            .unwrap();

        let names: Vec<_> = page.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"src"));
        assert!(!names.contains(&".git"));
        assert!(!names.contains(&"node_modules"));
        assert!(!names.contains(&"target"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_directory_reads_root_gitignore() {
        let root = temp_workspace("list-gitignore");
        fs::write(root.join(".gitignore"), "/debug.log\n/build/\n").unwrap();
        fs::write(root.join("app.txt"), "x").unwrap();
        fs::write(root.join("debug.log"), "x").unwrap();
        fs::create_dir(root.join("build")).unwrap();
        fs::write(root.join("build").join("out.txt"), "x").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let page = workspace
            .list_directory(
                FileListRequest {
                    root_id,
                    relative_path: PathBuf::new(),
                    max_depth: 2,
                    max_entries: 100,
                },
                None,
            )
            .unwrap();

        let names: Vec<_> = page.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"app.txt"));
        assert!(!names.contains(&"debug.log"));
        assert!(!names.contains(&"build"));
        assert!(!names.contains(&"out.txt"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_or_oversized_gitignore_aborts_listing_with_bounded_diagnostic() {
        for (name, contents) in [
            ("unsupported", "!secret\n".to_string()),
            (
                "oversized",
                "#".repeat(crate::perf::budgets::MAX_AUXILIARY_READ_BYTES + 1),
            ),
        ] {
            let root = temp_workspace(&format!("list-gitignore-{name}"));
            fs::write(root.join("secret"), "x").unwrap();
            fs::write(root.join(".gitignore"), contents).unwrap();
            let mut workspace = WorkspaceState::new();
            let root_id = workspace.add_root(&root).unwrap();

            let page = workspace
                .list_directory(
                    FileListRequest {
                        root_id,
                        relative_path: PathBuf::new(),
                        max_depth: 1,
                        max_entries: 100,
                    },
                    None,
                )
                .unwrap();

            assert!(page.entries.is_empty());
            assert!(page.truncated);
            assert_eq!(page.diagnostics.len(), 1);
            assert!(page.diagnostics[0].message.contains(".gitignore"));
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn list_directory_rejects_unknown_root() {
        let workspace = WorkspaceState::new();
        let error = workspace
            .list_directory(
                FileListRequest {
                    root_id: 42,
                    relative_path: PathBuf::new(),
                    max_depth: 1,
                    max_entries: 100,
                },
                None,
            )
            .unwrap_err();
        assert!(matches!(error, WorkspaceError::UnknownRoot { root_id } if root_id == 42));
    }

    #[test]
    fn list_directory_rejects_traversal_escape() {
        let root = temp_workspace("list-escape");
        fs::create_dir(root.join("sub")).unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let error = workspace
            .list_directory(
                FileListRequest {
                    root_id,
                    relative_path: PathBuf::from("sub/../../.."),
                    max_depth: 1,
                    max_entries: 100,
                },
                None,
            )
            .unwrap_err();
        assert!(matches!(error, WorkspaceError::OutsideRoot));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg(unix)]
    fn list_directory_reports_permission_denied_as_entry_diagnostic() {
        let root = temp_workspace("list-perm");
        fs::create_dir(root.join("locked")).unwrap();
        fs::write(root.join("locked").join("secret.txt"), "x").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let mut permissions = fs::metadata(root.join("locked")).unwrap().permissions();
        let original_mode = permissions.mode();
        permissions.set_mode(0o000);
        fs::set_permissions(root.join("locked"), permissions).unwrap();

        // Some test environments (e.g. root, permissive containers) do not
        // enforce filesystem permissions. Only assert the diagnostic when the
        // underlying read_dir actually fails, otherwise the test documents the
        // code path without failing on the environment.
        let permission_enforced = fs::read_dir(root.join("locked")).is_err();

        let page = workspace
            .list_directory(
                FileListRequest {
                    root_id,
                    relative_path: PathBuf::new(),
                    max_depth: 2,
                    max_entries: 100,
                },
                None,
            )
            .unwrap();

        let locked = page.entries.iter().find(|e| e.name == "locked").unwrap();
        if permission_enforced {
            assert!(locked.diagnostic.is_some());
            assert_eq!(
                locked.diagnostic.as_ref().unwrap().code,
                FileErrorCode::PermissionDenied
            );
        }

        let mut permissions = fs::metadata(root.join("locked")).unwrap().permissions();
        permissions.set_mode(original_mode);
        fs::set_permissions(root.join("locked"), permissions).unwrap();

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_directory_cancellation_stops_early() {
        let root = temp_workspace("list-cancel");
        for index in 0..100 {
            fs::write(root.join(format!("{index}.txt")), "x").unwrap();
        }
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let token = Arc::new(AtomicBool::new(true));
        let page = workspace
            .list_directory(
                FileListRequest {
                    root_id,
                    relative_path: PathBuf::new(),
                    max_depth: 1,
                    max_entries: 1000,
                },
                Some(&token),
            )
            .unwrap();

        assert!(page.cancelled);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_directory_counts_children_for_directories() {
        let root = temp_workspace("list-count");
        fs::create_dir(root.join("parent")).unwrap();
        fs::write(root.join("parent").join("a.txt"), "x").unwrap();
        fs::write(root.join("parent").join("b.txt"), "x").unwrap();
        let mut workspace = WorkspaceState::new();
        let root_id = workspace.add_root(&root).unwrap();

        let page = workspace
            .list_directory(
                FileListRequest {
                    root_id,
                    relative_path: PathBuf::new(),
                    max_depth: 1,
                    max_entries: 100,
                },
                None,
            )
            .unwrap();

        let parent = page.entries.iter().find(|e| e.name == "parent").unwrap();
        assert_eq!(parent.child_count, Some(2));

        let _ = fs::remove_dir_all(root);
    }
}
