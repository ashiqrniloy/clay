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

use crop::{Rope, RopeBuilder};
use tokio::{
    fs as tokio_fs,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};

use crate::perf::budgets::{
    BINARY_SNIFF_BYTES, DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES, MAX_AUXILIARY_READ_BYTES,
    MAX_DOCUMENTS_PER_CLIENT, MAX_GITIGNORE_LINES, MAX_GITIGNORE_PATTERN_CHARS,
    MAX_GITIGNORE_PATTERNS, MAX_SERVER_DOCUMENTS, TRANSIENT_MENU_MAX_ITEMS,
};
use crate::protocol::{
    ClientId, DocumentAccess, DocumentId, DocumentMetadata, DocumentTextHead, DocumentVersion,
    FileErrorCode,
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

/// One depth-1 listing inside the built-in path session. Unlike
/// `FileListRequest` (workspace-root-scoped, root-relative), the target is an
/// absolute or cwd-relative path reached by explicit user navigation: the
/// built-in surface itself is the authorization event, so no workspace root
/// grant is required and none is created. `max_entries` is capped at the
/// transient-menu ceiling so the installed snapshot always fits the menu
/// projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UserBrowseListingPlan {
    pub(crate) target: PathBuf,
    pub(crate) max_entries: usize,
}

impl Default for UserBrowseListingPlan {
    fn default() -> Self {
        Self {
            target: PathBuf::new(),
            max_entries: TRANSIENT_MENU_MAX_ITEMS,
        }
    }
}

/// Kind of a user-browse entry, resolved by following symlinks so the listed
/// activation path is the canonical target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserBrowseEntryKind {
    Directory,
    File,
    Other,
}

/// One inert entry in a user-browse page. `canonical_path` is the
/// symlink-resolved activation path; activation must resolve through this
/// server-held entry and never through a client-supplied path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UserBrowseEntry {
    pub(crate) name: String,
    pub(crate) kind: UserBrowseEntryKind,
    pub(crate) canonical_path: PathBuf,
    pub(crate) size: Option<u64>,
}

/// Result page for one user-browse listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UserBrowsePage {
    pub(crate) canonical_dir: PathBuf,
    pub(crate) entries: Vec<UserBrowseEntry>,
    pub(crate) truncated: bool,
}

/// Failure of one user-browse listing. Browsing never panics and never
/// mutates grants; failures surface as session status text.
#[derive(Debug)]
pub(crate) enum UserBrowseError {
    /// Target does not exist or is not readable.
    Unavailable { path: PathBuf, source: io::Error },
    /// Target exists but is not a directory.
    NotADirectory { path: PathBuf },
    /// The blocking-pool task failed to join.
    Join,
}

impl fmt::Display for UserBrowseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { path, source } => {
                write!(formatter, "cannot browse {}: {source}", path.display())
            }
            Self::NotADirectory { path } => {
                write!(formatter, "{} is not a directory", path.display())
            }
            Self::Join => formatter.write_str("browse listing worker failed"),
        }
    }
}

impl Error for UserBrowseError {}

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
    pub(crate) async fn snapshot(&self, _client_id: ClientId) -> OpenDocumentHead {
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
        let head = document.document_text_head();
        OpenDocumentHead { metadata, head }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenDocumentHead {
    pub(crate) metadata: DocumentMetadata,
    pub(crate) head: DocumentTextHead,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenDocumentRefresh {
    pub(crate) metadata: DocumentMetadata,
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
#[derive(Debug)]
struct DocumentReservation {
    pending: Arc<AtomicU64>,
    bytes: u64,
    current_bytes: u64,
    committed: bool,
}

impl DocumentReservation {
    fn bytes(&self) -> u64 {
        self.bytes
    }

    fn current_bytes(&self) -> u64 {
        self.current_bytes
    }

    fn commit(&mut self) {
        if !self.committed {
            self.pending.fetch_sub(self.bytes, Ordering::AcqRel);
            self.committed = true;
        }
    }
}

impl Drop for DocumentReservation {
    fn drop(&mut self) {
        if !self.committed {
            self.pending.fetch_sub(self.bytes, Ordering::AcqRel);
        }
    }
}

struct OpenPlan {
    file_state: FileDocumentState,
    client_id: ClientId,
    reservation: DocumentReservation,
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
    reservation: DocumentReservation,
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
    reservation: DocumentReservation,
}

struct ReloadIoOutcome {
    text: Rope,
    reloaded_metadata: FileMetadata,
}

#[derive(Debug)]
pub(crate) struct WorkspaceState {
    roots: HashMap<WorkspaceRootId, WorkspaceRoot>,
    /// Directory-root index by canonical path (plan 119 SC-6): one lookup
    /// resolves a session's recorded workspace root to this state's root id,
    /// and `add_root` dedupes through it. Single-file grants stay out (their
    /// canonical path is a file, never a workspace root).
    directory_roots_by_path: HashMap<PathBuf, WorkspaceRootId>,
    documents: HashMap<DocumentId, OpenDocument>,
    path_to_document: HashMap<PathBuf, DocumentId>,
    /// Resident bytes held by canonical file-backed document ropes. The
    /// separate atomic reservation counter closes the race between concurrent
    /// unlocked file reads while keeping cancellation cleanup synchronous.
    resident_document_bytes: u64,
    reserved_document_bytes: Arc<AtomicU64>,
    next_root_id: WorkspaceRootId,
    next_document_id: DocumentId,
    /// Server-created tab workspaces share this allocator so document IDs
    /// remain unique across tabs. Standalone test workspaces keep the local
    /// allocator for deterministic IDs.
    document_id_allocator: Option<Arc<AtomicU64>>,
}

impl WorkspaceState {
    pub(crate) fn new() -> Self {
        Self {
            roots: HashMap::new(),
            directory_roots_by_path: HashMap::new(),
            documents: HashMap::new(),
            path_to_document: HashMap::new(),
            resident_document_bytes: 0,
            reserved_document_bytes: Arc::new(AtomicU64::new(0)),
            next_root_id: 1,
            next_document_id: 1,
            document_id_allocator: None,
        }
    }

    pub(crate) fn with_document_id_allocator(mut self, allocator: Arc<AtomicU64>) -> Self {
        self.document_id_allocator = Some(allocator);
        self
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
        if let Some(existing) = self.directory_roots_by_path.get(&canonical_path) {
            return Ok(*existing);
        }

        if self.roots.len() >= MAX_WORKSPACE_ROOTS {
            return Err(WorkspaceError::RootLimitExceeded);
        }

        let id = self.next_root_id;
        self.next_root_id = self.next_root_id.saturating_add(1);
        self.directory_roots_by_path
            .insert(canonical_path.clone(), id);
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

/// Traverse a user-browse plan on the blocking pool via
/// [`execute_user_browse_listing`]; reads no workspace state and creates no
/// grant. Depth is fixed at 1: only immediate children are listed, no child
/// trees are scanned, and no recursive child counts are computed. Names are
/// collected into a bounded, name-sorted window, so a pathological directory
/// cannot grow memory beyond the cap and truncation is deterministic (the
/// lexicographically-first `max_entries` names survive). Unreadable or
/// non-UTF-8-named children are skipped; a broken symlink child is skipped
/// because its target cannot be canonicalized.
pub(crate) fn traverse_user_browse_directory(
    plan: UserBrowseListingPlan,
) -> Result<UserBrowsePage, UserBrowseError> {
    let canonical_dir =
        fs::canonicalize(&plan.target).map_err(|source| UserBrowseError::Unavailable {
            path: plan.target.clone(),
            source,
        })?;
    let metadata = fs::metadata(&canonical_dir).map_err(|source| UserBrowseError::Unavailable {
        path: canonical_dir.clone(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(UserBrowseError::NotADirectory {
            path: canonical_dir,
        });
    }

    let max_entries = plan.max_entries.max(1);
    let mut names: Vec<String> = Vec::with_capacity(max_entries.min(TRANSIENT_MENU_MAX_ITEMS));
    let mut truncated = false;
    let read_dir = fs::read_dir(&canonical_dir).map_err(|source| UserBrowseError::Unavailable {
        path: canonical_dir.clone(),
        source,
    })?;
    for entry in read_dir.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        match names.binary_search(&name) {
            // Duplicate names cannot occur within one directory; skip.
            Ok(_) => {}
            Err(pos) => {
                if names.len() < max_entries {
                    names.insert(pos, name);
                } else if pos < names.len() {
                    names.insert(pos, name);
                    names.pop();
                    truncated = true;
                } else {
                    truncated = true;
                }
            }
        }
    }

    let mut entries = Vec::with_capacity(names.len());
    for name in names {
        // Follow symlinks so the activation path is the canonical target.
        let Ok(canonical) = fs::canonicalize(canonical_dir.join(&name)) else {
            continue;
        };
        let Ok(metadata) = fs::metadata(&canonical) else {
            continue;
        };
        let kind = if metadata.is_dir() {
            UserBrowseEntryKind::Directory
        } else if metadata.is_file() {
            UserBrowseEntryKind::File
        } else {
            UserBrowseEntryKind::Other
        };
        entries.push(UserBrowseEntry {
            name,
            kind,
            canonical_path: canonical,
            size: (kind == UserBrowseEntryKind::File).then_some(metadata.len()),
        });
    }
    Ok(UserBrowsePage {
        canonical_dir,
        entries,
        truncated,
    })
}

/// Execute a user-browse listing on Tokio's bounded blocking pool. Callers
/// must snapshot authority/state into the plan before awaiting; no
/// workspace/tab/menu lock is held here. Started blocking tasks are not
/// abortable, so the entry cap and depth-1 shape are the boundedness
/// guarantees.
pub(crate) async fn execute_user_browse_listing(
    plan: UserBrowseListingPlan,
) -> Result<UserBrowsePage, UserBrowseError> {
    tokio::task::spawn_blocking(move || traverse_user_browse_directory(plan))
        .await
        .map_err(|_| UserBrowseError::Join)?
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
                    plan.reservation.bytes(),
                    plan.reservation.current_bytes(),
                )
                .await?;
                let mut file_state = plan.file_state;
                file_state.last_known_metadata = observed_metadata;
                self.register_canonical_file(file_state, text, plan.client_id, plan.reservation)
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
                let (text, observed_metadata) = open_io(
                    &plan.canonical_path,
                    plan.display_path.clone(),
                    plan.reservation.bytes(),
                    plan.reservation.current_bytes(),
                )
                .await?;
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
        let reservation =
            self.reserve_document_bytes(file_state.last_known_metadata.len(), file_path)?;
        Ok(OpenPrepare::New(OpenPlan {
            file_state,
            client_id,
            reservation,
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
        let reservation = self.reserve_document_bytes(metadata.len(), &display_path)?;
        Ok(SelectedOpenPrepare::New(SelectedOpenPlan {
            canonical_path,
            display_path,
            client_id,
            reservation,
        }))
    }

    async fn register_selected_file(
        &mut self,
        plan: SelectedOpenPlan,
        text: Rope,
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
        self.register_canonical_file(file_state, text, plan.client_id, plan.reservation)
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
        let reservation =
            self.reserve_document_bytes(text.len() as u64, &file_state.workspace_relative_path)?;
        self.register_canonical_file(file_state, Rope::from(text), client_id, reservation)
            .await
    }

    /// Canonical workspace paths of open documents (agent checkpoint capture).
    pub(crate) fn open_document_canonical_paths(&self) -> Vec<(DocumentId, PathBuf)> {
        self.documents
            .iter()
            .map(|(&document_id, open_document)| {
                (document_id, open_document.file_state.canonical_path.clone())
            })
            .collect()
    }

    pub(crate) fn document_handle(
        &self,
        document_id: DocumentId,
    ) -> Option<Arc<Mutex<DocumentState>>> {
        self.documents
            .get(&document_id)
            .map(|open_document| Arc::clone(&open_document.document))
    }

    /// Open-document lookup by canonical path for read-only agent access
    /// (Phase 1 reverse RPC). Returns the id without acquiring a lease.
    pub(crate) fn find_open_document_by_canonical_path(
        &self,
        canonical_path: &Path,
    ) -> Option<DocumentId> {
        self.path_to_document.get(canonical_path).copied()
    }

    /// The directory root registered for exactly this canonical path (plan
    /// 119 SC-6): agent tool calls resolve the session's recorded workspace
    /// root to the root id of the state the folder is open in. `None` = this
    /// state does not carry that root — the caller fails closed.
    pub(crate) fn root_id_for_canonical_path(
        &self,
        canonical_path: &Path,
    ) -> Option<WorkspaceRootId> {
        self.directory_roots_by_path.get(canonical_path).copied()
    }

    /// Canonicalized, containment-checked path for an existing file
    /// (agent reverse-RPC reads and stats).
    pub(crate) fn contained_existing_path(
        &self,
        root_id: WorkspaceRootId,
        file_path: &Path,
    ) -> Result<PathBuf, WorkspaceError> {
        self.canonical_file_state(root_id, file_path)
            .map(|state| state.canonical_path)
    }

    /// Canonical path for a workspace-relative or absolute file that may not
    /// exist yet (agent write of a new file). Containment is enforced against
    /// the parent directory so a missing leaf cannot smuggle `..` past the
    /// root check.
    pub(crate) fn contained_new_file_path(
        &self,
        root_id: WorkspaceRootId,
        file_path: &Path,
    ) -> Result<PathBuf, WorkspaceError> {
        let root = self
            .roots
            .get(&root_id)
            .ok_or(WorkspaceError::UnknownRoot { root_id })?;
        let WorkspaceAuthority::Directory {
            canonical_path: root_path,
        } = &root.authority
        else {
            return Err(WorkspaceError::OutsideRoot);
        };
        let joined = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            root_path.join(file_path)
        };
        if !joined.starts_with(root_path) {
            return Err(WorkspaceError::OutsideRoot);
        }
        // Walk up to the nearest existing ancestor so new nested files can be
        // created; containment is checked against that ancestor's realpath.
        let mut file_name_parts: Vec<std::ffi::OsString> = Vec::new();
        let mut probe = joined.clone();
        let canonical_ancestor = loop {
            match fs::canonicalize(&probe) {
                Ok(canonical) => {
                    break canonical;
                }
                Err(_) => {
                    let name = match probe.file_name() {
                        Some(name) => name.to_os_string(),
                        None => {
                            return Err(WorkspaceError::FileUnavailable {
                                path: joined.clone(),
                                source: io::Error::new(io::ErrorKind::NotFound, "invalid path"),
                            });
                        }
                    };
                    file_name_parts.push(name);
                    if !probe.pop() {
                        return Err(WorkspaceError::FileUnavailable {
                            path: joined.clone(),
                            source: io::Error::new(io::ErrorKind::NotFound, "root path missing"),
                        });
                    }
                }
            }
        };
        if !canonical_ancestor.starts_with(root_path) {
            return Err(WorkspaceError::OutsideRoot);
        }
        let mut canonical_path = canonical_ancestor;
        for name in file_name_parts.into_iter().rev() {
            canonical_path = canonical_path.join(name);
        }
        if !canonical_path.starts_with(root_path) {
            return Err(WorkspaceError::OutsideRoot);
        }
        Ok(canonical_path)
    }

    /// Canonical path of an open document, used to seed the built-in path
    /// session from the authorized active document's real directory. Works
    /// for workspace-relative and selected-file (`SingleFile` grant) opens
    /// alike because the stored path is the canonical file path.
    pub(crate) fn document_canonical_path(&self, document_id: DocumentId) -> Option<PathBuf> {
        self.documents
            .get(&document_id)
            .map(|open_document| open_document.file_state.canonical_path.clone())
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

    pub(crate) async fn open_document_refreshes(
        &self,
        client_id: ClientId,
    ) -> Result<Vec<OpenDocumentRefresh>, WorkspaceError> {
        let mut entries = Vec::with_capacity(self.documents.len());
        for (&document_id, open_document) in &self.documents {
            let metadata =
                metadata_for_open_document(document_id, open_document, client_id).await?;
            entries.push(OpenDocumentRefresh { metadata });
        }
        entries.sort_by_key(|refresh| refresh.metadata.document_id);
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
                let bytes = open_document.document.lock().await.byte_len() as u64;
                self.release_document_bytes(bytes);
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
            let bytes = open_document.document.lock().await.byte_len() as u64;
            self.release_document_bytes(bytes);
            self.path_to_document
                .remove(&open_document.file_state.canonical_path);
        }
        Ok(CloseDocumentOutcome {
            document_id,
            version,
            closed,
        })
    }

    /// Release one client's access on one document and drop the registry
    /// entry when no access holders remain (agent save cleanup). Best-effort:
    /// unknown documents are ignored. Returns `true` when the registry entry
    /// was removed.
    pub(crate) async fn release_single_document_access(
        &mut self,
        document_id: DocumentId,
        client_id: ClientId,
    ) -> bool {
        let Some(open_document) = self.documents.get(&document_id) else {
            return false;
        };
        // Plan 126 D7: release, holder count, and byte length are read under one
        // guard. Three separate acquisitions let a concurrent access grant slip
        // between the holder check and the registry removal.
        let (holders, bytes) = {
            let mut document = open_document.document.lock().await;
            document.release_access(client_id);
            (document.access_holder_count(), document.byte_len() as u64)
        };
        if holders > 0 {
            return false;
        }
        let Some(open_document) = self.documents.remove(&document_id) else {
            return false;
        };
        self.release_document_bytes(bytes);
        self.path_to_document
            .remove(&open_document.file_state.canonical_path);
        true
    }

    fn reserve_document_bytes(
        &self,
        requested_bytes: u64,
        path: &Path,
    ) -> Result<DocumentReservation, WorkspaceError> {
        let pending = self.reserved_document_bytes.load(Ordering::Acquire);
        let current_bytes = self.resident_document_bytes.saturating_add(pending);
        if requested_bytes > DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES.saturating_sub(current_bytes) {
            return Err(WorkspaceError::DocumentBudgetExceeded {
                path: path.to_path_buf(),
                budget_bytes: DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES,
                current_bytes,
                requested_bytes,
            });
        }
        self.reserved_document_bytes
            .fetch_add(requested_bytes, Ordering::AcqRel);
        Ok(DocumentReservation {
            pending: Arc::clone(&self.reserved_document_bytes),
            bytes: requested_bytes,
            current_bytes: current_bytes.saturating_add(requested_bytes),
            committed: false,
        })
    }

    fn commit_document_bytes(
        &mut self,
        reservation: &mut DocumentReservation,
        actual_bytes: u64,
        path: &Path,
    ) -> Result<(), WorkspaceError> {
        if actual_bytes > reservation.bytes()
            || actual_bytes
                > DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES.saturating_sub(self.resident_document_bytes)
        {
            return Err(WorkspaceError::DocumentBudgetExceeded {
                path: path.to_path_buf(),
                budget_bytes: DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES,
                current_bytes: self.resident_document_bytes,
                requested_bytes: actual_bytes,
            });
        }
        reservation.commit();
        self.resident_document_bytes = self.resident_document_bytes.saturating_add(actual_bytes);
        Ok(())
    }

    fn release_document_bytes(&mut self, bytes: u64) {
        self.resident_document_bytes = self.resident_document_bytes.saturating_sub(bytes);
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
    /// workspace mutex; the heavy chunked `tokio::fs` write happens in [`save_io`]
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
    /// on-disk metadata, and a resident-budget reservation. Runs under the
    /// workspace mutex; streamed UTF-8 IO happens in [`reload_io`] after the
    /// mutex is released.
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
        let reservation =
            self.reserve_document_bytes(pre_read_identity.metadata().len(), &relative_path)?;
        Ok(ReloadPlan {
            document_id,
            canonical_path,
            relative_path,
            document,
            force,
            reservation,
        })
    }

    async fn commit_reload(
        &mut self,
        mut plan: ReloadPlan,
        io: ReloadIoOutcome,
    ) -> Result<ReloadDocumentOutcome, WorkspaceError> {
        let Some(open_document) = self.documents.get(&plan.document_id) else {
            return Err(WorkspaceError::UnknownDocument {
                document_id: plan.document_id,
            });
        };
        if !Arc::ptr_eq(&open_document.document, &plan.document) {
            return Err(WorkspaceError::UnknownDocument {
                document_id: plan.document_id,
            });
        }
        // Re-check dirtiness on reacquire: the document may have been edited by
        // another connection during the unlocked read. Don't clobber unsaved
        // edits unless the caller asked to force the reload.
        if plan.document.lock().await.is_dirty() && !plan.force {
            return Err(WorkspaceError::DirtyDocument {
                document_id: plan.document_id,
            });
        }
        let (version, previous_bytes, actual_bytes) = {
            let mut document = plan.document.lock().await;
            let previous_bytes = document.byte_len() as u64;
            document.replace_rope_from_storage(io.text);
            (
                document.version(),
                previous_bytes,
                document.byte_len() as u64,
            )
        };
        self.commit_document_bytes(&mut plan.reservation, actual_bytes, &plan.relative_path)?;
        self.release_document_bytes(previous_bytes);
        if let Some(open_document) = self.documents.get_mut(&plan.document_id) {
            open_document.file_state.last_known_metadata = io.reloaded_metadata;
        }
        Ok(ReloadDocumentOutcome {
            document_id: plan.document_id,
            version,
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
        text: Rope,
        client_id: ClientId,
        mut reservation: DocumentReservation,
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
        self.commit_document_bytes(
            &mut reservation,
            text.byte_len() as u64,
            &file_state.workspace_relative_path,
        )?;
        let document_id = if let Some(allocator) = &self.document_id_allocator {
            allocator.fetch_add(1, Ordering::Relaxed)
        } else {
            let document_id = self.next_document_id;
            self.next_document_id = self.next_document_id.saturating_add(1);
            document_id
        };
        let document = Arc::new(Mutex::new(DocumentState::from_rope(
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

const FILE_READ_BUFFER_BYTES: usize = 64 * 1024;

/// Maximum bytes of an incomplete UTF-8 scalar carried between reads (a
/// 4-byte scalar can be split with at most three bytes still pending).
const UTF8_CARRY_BYTES: usize = 3;

/// Stream one authorized file into a Crop rope without materializing a
/// document-sized `String`. The reservation is established while the
/// workspace mutex is held; this read enforces it again against the opened
/// handle so growth between prepare and EOF cannot exceed the session budget.
/// UTF-8 is validated incrementally with a maximum three-byte carry, and NUL
/// sniffing is limited to the first `BINARY_SNIFF_BYTES` bytes.
async fn read_file_streamed(
    canonical_path: &Path,
    error_path: &Path,
    max_bytes: u64,
    budget_current_bytes: u64,
) -> Result<(Rope, FileMetadata), WorkspaceError> {
    let unavailable = |source: io::Error| WorkspaceError::FileUnavailable {
        path: error_path.to_path_buf(),
        source,
    };
    let mut file = tokio_fs::File::open(canonical_path)
        .await
        .map_err(unavailable)?;
    let metadata = file.metadata().await.map_err(unavailable)?;
    validate_regular_file_metadata(&metadata)?;
    let observed = FileMetadata::from_fs_metadata(&metadata);
    if observed.len() > max_bytes {
        return Err(WorkspaceError::DocumentBudgetExceeded {
            path: error_path.to_path_buf(),
            budget_bytes: DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES,
            current_bytes: budget_current_bytes,
            requested_bytes: observed.len(),
        });
    }
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

    let mut builder = RopeBuilder::new();
    // One scratch buffer for the whole read (plan 119 P1-1): `pending` bytes
    // at the front are the incomplete UTF-8 scalar carried from the previous
    // read, and the window after them stays exactly `FILE_READ_BUFFER_BYTES`,
    // so the read loop allocates nothing per 64 KiB chunk. A fresh `combined`
    // Vec here cost ~800 allocations on a 50 MiB open.
    let mut buffer = Box::new([0u8; FILE_READ_BUFFER_BYTES + UTF8_CARRY_BYTES]);
    let mut pending = 0usize;
    let mut total_read = 0u64;
    let mut sniffed = 0usize;

    loop {
        let read = file
            .read(&mut buffer[pending..pending + FILE_READ_BUFFER_BYTES])
            .await
            .map_err(unavailable)?;
        if read == 0 {
            break;
        }
        let sniff_len = BINARY_SNIFF_BYTES.saturating_sub(sniffed).min(read);
        if buffer[pending..pending + sniff_len].contains(&0) {
            return Err(WorkspaceError::BinaryFileNotSupported {
                path: error_path.to_path_buf(),
            });
        }
        sniffed = sniffed.saturating_add(sniff_len);

        let next_total = total_read.checked_add(read as u64).ok_or_else(|| {
            WorkspaceError::DocumentBudgetExceeded {
                path: error_path.to_path_buf(),
                budget_bytes: DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES,
                current_bytes: budget_current_bytes,
                requested_bytes: u64::MAX,
            }
        })?;
        if next_total > max_bytes {
            return Err(WorkspaceError::DocumentBudgetExceeded {
                path: error_path.to_path_buf(),
                budget_bytes: DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES,
                current_bytes: budget_current_bytes,
                requested_bytes: next_total,
            });
        }
        total_read = next_total;

        let total = pending + read;
        match std::str::from_utf8(&buffer[..total]) {
            Ok(text) => {
                builder.append(text);
                pending = 0;
            }
            Err(error) if error.error_len().is_none() => {
                let valid_up_to = error.valid_up_to();
                builder.append(
                    std::str::from_utf8(&buffer[..valid_up_to])
                        .expect("UTF-8 prefix before an incomplete scalar is valid"),
                );
                buffer.copy_within(valid_up_to..total, 0);
                pending = total - valid_up_to;
            }
            Err(_) => {
                return Err(WorkspaceError::InvalidUtf8 {
                    path: error_path.to_path_buf(),
                    source: String::from_utf8(buffer[..total].to_vec())
                        .expect_err("invalid UTF-8 was detected"),
                });
            }
        }
    }

    if pending != 0 {
        return Err(WorkspaceError::InvalidUtf8 {
            path: error_path.to_path_buf(),
            source: String::from_utf8(buffer[..pending].to_vec())
                .expect_err("incomplete UTF-8 was detected"),
        });
    }

    let observed = FileMetadata::from_fs_metadata(&file.metadata().await.map_err(unavailable)?);
    Ok((builder.build(), observed))
}

/// Heavy disk read for file open/reload, performed with the workspace mutex
/// released. `max_bytes` is a reserved session-budget slice, not a per-file
/// size ceiling.
async fn open_io(
    canonical_path: &Path,
    error_path: PathBuf,
    max_bytes: u64,
    budget_current_bytes: u64,
) -> Result<(Rope, FileMetadata), WorkspaceError> {
    read_file_streamed(canonical_path, &error_path, max_bytes, budget_current_bytes).await
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

/// Test-only gate that pauses an atomic write after chunks are on the temp
/// file and before `fsync`, so a concurrent edit can run while disk IO is
/// blocked without holding the document mutex.
#[cfg(test)]
struct AtomicWritePause {
    path: PathBuf,
    entered: Option<tokio::sync::oneshot::Sender<()>>,
    release: Arc<tokio::sync::Notify>,
}

#[cfg(test)]
static ATOMIC_WRITE_PAUSES: std::sync::Mutex<Vec<AtomicWritePause>> =
    std::sync::Mutex::new(Vec::new());

#[cfg(test)]
async fn pause_atomic_write(target: &Path) {
    let pause = {
        let mut pauses = ATOMIC_WRITE_PAUSES
            .lock()
            .expect("atomic-write pause lock poisoned");
        pauses
            .iter()
            .rposition(|pause| pause.path == target)
            .map(|index| pauses.remove(index))
    };
    if let Some(pause) = pause {
        if let Some(entered) = pause.entered {
            let _ = entered.send(());
        }
        pause.release.notified().await;
    }
}

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

/// Atomically write `chunks` to `target` via a temp file + rename, returning
/// the metadata of the saved file. Steps: create an exclusive unpredictable
/// temp file in `target`'s directory, write each chunk, `fsync` (durability),
/// restore the original file's permissions (fail closed on Unix when that
/// fails), revalidate the target's stable identity against `expected`
/// immediately before the replace, then `rename` over the target. Every
/// failure path removes the temp file and leaves the target untouched.
async fn atomic_write_file(
    target: &Path,
    bytes: &[u8],
    expected: Option<&TargetIdentity>,
) -> Result<FileMetadata, AtomicSaveError> {
    atomic_write_chunks(target, std::iter::once(bytes), expected).await
}

/// Atomically create a brand-new file (temp + write + fsync + rename) so a
/// crash mid-write cannot leave a partial file where a caller already reported
/// success. A file that did not exist has no stable identity to revalidate, so
/// this is `atomic_write_file` with `expected: None`: a file created by someone
/// else between the caller's existence check and this rename is replaced
/// without a change check (pre-atomic `fs::write` clobbered it too, and this at
/// least never leaves it torn).
pub(crate) async fn atomic_create_file(target: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write_file(target, bytes, None)
        .await
        .map(|_| ())
        .map_err(|error| match error {
            AtomicSaveError::Io(error) => error,
            // Unreachable with `expected: None`; keep the result typed anyway.
            AtomicSaveError::TargetChanged => {
                io::Error::new(io::ErrorKind::NotFound, "target vanished during save")
            }
        })
}

async fn atomic_write_chunks(
    target: &Path,
    chunks: impl IntoIterator<Item = impl AsRef<[u8]>>,
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
        for chunk in chunks {
            file.write_all(chunk.as_ref()).await?;
        }
        #[cfg(test)]
        pause_atomic_write(target).await;
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
/// and an Arc-root Crop rope clone (via the per-document mutex, not the
/// workspace mutex) so the commit phase can detect a concurrent edit through
/// `mark_clean_if_version` without holding the document lock across IO.
async fn save_io(plan: &SavePlan) -> Result<SaveIoOutcome, WorkspaceError> {
    let (prepared_version, rope) = {
        let document = plan.document.lock().await;
        (document.version(), document.clone_rope())
    };
    // Atomic save: write an exclusive unpredictable temp file in the target's
    // directory, fsync it, restore the original file's permissions, revalidate
    // the target's stable identity, then `rename` over the target. A crash or
    // power loss during the write leaves the original file intact (only the
    // temp is partial); the rename is atomic so the target is either the old
    // or the new content, never a torn write. Chunks are streamed from the
    // captured rope so a large document never becomes a transient `String`.
    let saved_metadata = atomic_write_chunks(
        &plan.canonical_path,
        rope.chunks(),
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
    let (text, reloaded_metadata) = open_io(
        &plan.canonical_path,
        plan.relative_path.clone(),
        plan.reservation.bytes(),
        plan.reservation.current_bytes(),
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
                plan.reservation.bytes(),
                plan.reservation.current_bytes(),
            )
            .await?;
            let mut file_state = plan.file_state;
            // Record the metadata of the handle actually read, not the
            // prepare-time stat, so the first save's staleness baseline
            // matches the bytes in the document.
            file_state.last_known_metadata = observed_metadata;
            let mut workspace = workspace.lock().await;
            workspace
                .register_canonical_file(file_state, text, plan.client_id, plan.reservation)
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
            let (text, observed_metadata) = open_io(
                &plan.canonical_path,
                plan.display_path.clone(),
                plan.reservation.bytes(),
                plan.reservation.current_bytes(),
            )
            .await?;
            let mut workspace = workspace.lock().await;
            workspace
                .register_selected_file(plan, text, observed_metadata)
                .await
        }
    }
}

/// Resolve the opening seed for the built-in path session, in order:
/// 1. canonical parent of the authorized active document,
/// 2. the bound tab's workspace directory root,
/// 3. the server's canonical current directory.
///
/// The seed is only a starting directory for navigation; it creates no root
/// or grant. The caller snapshots the active document id and tab root before
/// awaiting (both come from the connection's own command context).
pub(crate) async fn resolve_user_browse_seed(
    workspace: &Arc<Mutex<WorkspaceState>>,
    active_document_id: Option<DocumentId>,
    tab_workspace_root: Option<&str>,
) -> PathBuf {
    if let Some(document_id) = active_document_id {
        let workspace = workspace.lock().await;
        if let Some(parent) = workspace
            .document_canonical_path(document_id)
            .and_then(|path| path.parent().map(Path::to_path_buf))
        {
            return parent;
        }
    }
    if let Some(root) = tab_workspace_root.filter(|root| !root.is_empty()) {
        return PathBuf::from(root);
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
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
    DocumentBudgetExceeded {
        path: PathBuf,
        budget_bytes: u64,
        current_bytes: u64,
        requested_bytes: u64,
    },
    BinaryFileNotSupported {
        path: PathBuf,
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
            Self::DocumentBudgetExceeded {
                path,
                budget_bytes,
                current_bytes,
                requested_bytes,
            } => WorkspaceDiagnostic::new(
                FileErrorCode::DocumentBudgetExceeded,
                format!(
                    "opening workspace file {} would exceed the {} byte resident document budget ({} bytes resident, {} requested)",
                    display_workspace_path(path),
                    budget_bytes,
                    current_bytes,
                    requested_bytes,
                ),
                Some("Close other documents before opening this file; the resident-memory budget is server-owned and not configurable from init.js.".to_string()),
            ),
            Self::BinaryFileNotSupported { path } => WorkspaceDiagnostic::new(
                FileErrorCode::BinaryFileNotSupported,
                format!(
                    "workspace file {} appears to be binary and is not supported as a text document",
                    display_workspace_path(path)
                ),
                Some(format!(
                    "Clay checks for NUL bytes in the first {BINARY_SNIFF_BYTES} bytes; open a UTF-8 text file instead."
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
    let mut map = LISTING_CANCELLATIONS
        .lock()
        .expect("listing-cancellation registry mutex poisoned");
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
    let map = LISTING_CANCELLATIONS
        .lock()
        .expect("listing-cancellation registry mutex poisoned");
    if let Some(token) = map.get(token_id) {
        token.store(true, Ordering::Relaxed);
        true
    } else {
        false
    }
}

/// Remove a registered cancellation token. Called when the listing ends.
pub(crate) fn remove_listing_cancel_token(token_id: &str) {
    let mut map = LISTING_CANCELLATIONS
        .lock()
        .expect("listing-cancellation registry mutex poisoned");
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
            | Self::DocumentBudgetExceeded { .. }
            | Self::BinaryFileNotSupported { .. }
            | Self::RootLimitExceeded => None,
        }
    }
}

#[cfg(test)]
mod tests;
