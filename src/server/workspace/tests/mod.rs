//! Stay-in-crate unit tests for workspace state: document open/dedup, path
//! authorization, dirty state and saves, atomic-save hardening, per-client
//! document budgets, workspace roots, user browse, and directory listing.
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
    ATOMIC_WRITE_PAUSES, AtomicSaveError, AtomicWritePause, BEFORE_REVALIDATE_HOOKS,
    BETWEEN_METADATA_AND_READ_HOOKS, FILE_READ_BUFFER_BYTES, FileListEntryKind, FileListRequest,
    FileMetadata, SaveIoOutcome, TEST_TEMP_NAMES, TargetIdentity, UserBrowseEntryKind,
    UserBrowseError, UserBrowseListingPlan, WorkspaceError, WorkspaceState, atomic_write_file,
    build_ignore_set, execute_user_browse_listing, open_existing_file_unlocked,
    open_selected_file_unlocked, read_file_streamed, resolve_user_browse_seed,
    save_document_unlocked, traverse_user_browse_directory,
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

// Test suites (see each file for its scope).
mod atomic_save_hardening;
mod directory_listing;
mod dirty_state_and_save;
mod document_lifecycle;
mod document_open_dedup;
mod path_authorization;
mod save_concurrency;
mod user_browse;
mod workspace_roots;
