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
    BETWEEN_METADATA_AND_READ_HOOKS, FileListEntryKind, FileListRequest, FileMetadata,
    SaveIoOutcome, TEST_TEMP_NAMES, TargetIdentity, UserBrowseEntryKind, UserBrowseError,
    UserBrowseListingPlan, WorkspaceError, WorkspaceState, atomic_write_file, build_ignore_set,
    execute_user_browse_listing, open_existing_file_unlocked, open_selected_file_unlocked,
    resolve_user_browse_seed, save_document_unlocked, traverse_user_browse_directory,
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
            head: crate::protocol::DocumentTextHead::complete("hello 🌎\n".to_string()),
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
            head: crate::protocol::DocumentTextHead::complete("fn main() {}\n".to_string()),
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

    assert_eq!(opened.document.lock().await.text(), "changed on disk");
    assert!(!reloaded.dirty);

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
    assert_eq!(opened.document.lock().await.text(), "new text");
    assert!(!reloaded.dirty);
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

#[tokio::test]
async fn open_existing_file_streams_large_utf8_text_and_bounds_head() {
    let root = temp_workspace("large-open");
    let file = root.join("big.txt");
    let text = format!(
        "{}é{}",
        "a".repeat(crate::perf::budgets::MAX_CHUNK_BYTES * 4),
        "b".repeat(32)
    );
    fs::write(&file, &text).unwrap();
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_root(&root).unwrap();

    let opened = workspace
        .open_existing_file(root_id, "big.txt", 1)
        .await
        .unwrap();
    let document = opened.document.lock().await;
    let head = document.document_text_head();

    assert_eq!(document.text(), text);
    assert_eq!(head.total_bytes, text.len() as u64);
    assert!(head.first_chunk.len() <= crate::perf::budgets::MAX_CHUNK_BYTES);
    assert!(head.first_chunk.is_char_boundary(head.first_chunk.len()));
    assert_eq!(workspace.resident_document_bytes, text.len() as u64);

    drop(document);
    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn opening_three_documents_including_ten_megabytes_keeps_bounded_heads() {
    use std::io::Write;

    let root = temp_workspace("restore-three");
    fs::write(root.join("a.txt"), "alpha").unwrap();
    fs::write(root.join("b.txt"), "beta").unwrap();
    let large_path = root.join("large.txt");
    {
        let mut file = fs::File::create(&large_path).unwrap();
        let chunk = vec![b'x'; 1024 * 1024];
        for _ in 0..10 {
            file.write_all(&chunk).unwrap();
        }
    }
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_root(&root).unwrap();

    let first = workspace
        .open_existing_file(root_id, "a.txt", 1)
        .await
        .unwrap();
    let second = workspace
        .open_existing_file(root_id, "b.txt", 1)
        .await
        .unwrap();
    let large = workspace
        .open_existing_file(root_id, "large.txt", 1)
        .await
        .unwrap();

    for opened in [&first, &second, &large] {
        let head = opened.document.lock().await.document_text_head();
        assert!(head.first_chunk.len() <= crate::perf::budgets::MAX_CHUNK_BYTES);
    }
    assert_eq!(large.document.lock().await.byte_len(), 10 * 1024 * 1024);

    let refreshes = workspace.open_document_refreshes(0).await.unwrap();
    assert_eq!(refreshes.len(), 3);
    assert!(
        refreshes
            .windows(2)
            .all(|pair| { pair[0].metadata.document_id < pair[1].metadata.document_id })
    );

    let _ = fs::remove_file(root.join("a.txt"));
    let _ = fs::remove_file(root.join("b.txt"));
    let _ = fs::remove_file(large_path);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn selected_open_streams_large_text_without_file_size_ceiling() {
    let root = temp_workspace("large-selected");
    let file = root.join("picked.md");
    let text = "🦀".repeat(crate::perf::budgets::MAX_CHUNK_BYTES + 32);
    fs::write(&file, &text).unwrap();
    let mut workspace = WorkspaceState::new();

    let opened = workspace.open_selected_file(&file, 1).await.unwrap();
    assert_eq!(opened.document.lock().await.text(), text);

    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn reload_streams_new_text_and_replaces_resident_bytes() {
    let root = temp_workspace("large-reload");
    let file = root.join("note.txt");
    fs::write(&file, "small").unwrap();
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_root(&root).unwrap();
    let opened = workspace
        .open_existing_file(root_id, "note.txt", 1)
        .await
        .unwrap();

    let text = "reload 🪐 ".repeat(crate::perf::budgets::MAX_CHUNK_BYTES / 2);
    fs::write(&file, &text).unwrap();
    let reloaded = workspace
        .reload_document(opened.document_id, 1, true)
        .await
        .unwrap();

    assert!(!reloaded.dirty);
    assert_eq!(opened.document.lock().await.text(), text);
    assert_eq!(workspace.resident_document_bytes, text.len() as u64);

    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn document_budget_rejects_open_and_close_releases_resident_bytes() {
    let root = temp_workspace("document-budget");
    let file = root.join("note.txt");
    fs::write(&file, "1234").unwrap();
    let mut workspace = WorkspaceState::new();
    workspace.resident_document_bytes =
        crate::perf::budgets::DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES - 3;
    let root_id = workspace.add_root(&root).unwrap();

    let error = workspace
        .open_existing_file(root_id, "note.txt", 1)
        .await
        .unwrap_err();
    assert_eq!(
        error.diagnostic().code,
        FileErrorCode::DocumentBudgetExceeded
    );
    assert_eq!(
        workspace.resident_document_bytes,
        crate::perf::budgets::DOCUMENT_RESIDENT_MEMORY_BUDGET_BYTES - 3
    );

    workspace.resident_document_bytes = 0;
    let opened = workspace
        .open_existing_file(root_id, "note.txt", 1)
        .await
        .unwrap();
    assert!(workspace.resident_document_bytes > 0);
    workspace
        .close_document(opened.document_id, 1, true)
        .await
        .unwrap();
    assert_eq!(workspace.resident_document_bytes, 0);

    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn binary_sniff_rejects_nul_in_leading_bytes_but_not_after_boundary() {
    let root = temp_workspace("binary-sniff");
    let binary = root.join("binary.dat");
    let mut bytes = b"text".to_vec();
    bytes.push(0);
    bytes.extend_from_slice(b"tail");
    fs::write(&binary, bytes).unwrap();
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_root(&root).unwrap();
    let error = workspace
        .open_existing_file(root_id, "binary.dat", 1)
        .await
        .unwrap_err();
    assert_eq!(
        error.diagnostic().code,
        FileErrorCode::BinaryFileNotSupported
    );

    let text_file = root.join("late-nul.txt");
    let mut text = b"a".repeat(crate::perf::budgets::BINARY_SNIFF_BYTES);
    text.push(0);
    text.extend_from_slice(b"tail");
    fs::write(&text_file, &text).unwrap();
    let opened = workspace
        .open_existing_file(root_id, "late-nul.txt", 1)
        .await
        .unwrap();
    assert_eq!(opened.document.lock().await.byte_len(), text.len());

    let _ = fs::remove_file(binary);
    let _ = fs::remove_file(text_file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn utf8_scalar_split_across_file_read_buffers_remains_valid() {
    let root = temp_workspace("utf8-boundary");
    let file = root.join("boundary.txt");
    let text = format!("{}éafter", "a".repeat(64 * 1024 - 1));
    fs::write(&file, &text).unwrap();
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_root(&root).unwrap();

    let opened = workspace
        .open_existing_file(root_id, "boundary.txt", 1)
        .await
        .unwrap();
    assert_eq!(opened.document.lock().await.text(), text);

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

#[tokio::test]
async fn streamed_save_equals_rope_across_crop_chunks() {
    let root = temp_workspace("streamed-save-chunks");
    let file = root.join("note.txt");
    let mut text = String::new();
    while text.len() < 5000 {
        text.push_str("ascii-pad-");
        text.push('é');
        text.push('\u{1F30E}');
    }
    fs::write(&file, &text).unwrap();
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_root(&root).unwrap();
    let opened = workspace
        .open_existing_file(root_id, "note.txt", 1)
        .await
        .unwrap();
    {
        let document = opened.document.lock().await;
        let rope = document.clone_rope();
        assert!(
            rope.chunks().count() > 1,
            "fixture must span more than one Crop chunk"
        );
        assert_eq!(rope.to_string(), text);
    }

    let saved = workspace
        .save_document(opened.document_id, 1, 1)
        .await
        .unwrap();
    assert!(!saved.dirty);
    assert_eq!(fs::read(&file).unwrap(), text.as_bytes());

    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn edit_during_blocked_save_does_not_wait_on_document_mutex() {
    let root = temp_workspace("save-blocked-edit");
    let file = root.join("note.txt");
    fs::write(&file, "hello").unwrap();
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
    let root_id = workspace.lock().await.add_root(&root).unwrap();
    let opened = open_existing_file_unlocked(&workspace, root_id, "note.txt", 1)
        .await
        .unwrap();
    {
        let mut document = opened.document.lock().await;
        assert_eq!(
            document.apply_edit(
                opened.document_id,
                1,
                Some(1),
                1,
                90,
                EditOperation::Insert {
                    byte_offset: 5,
                    text: " world".to_string(),
                },
            ),
            ServerMessage::EditAck {
                document_id: opened.document_id,
                confirmed_version: 2,
                transaction_id: 90,
            }
        );
    }

    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let release = Arc::new(tokio::sync::Notify::new());
    ATOMIC_WRITE_PAUSES
        .lock()
        .expect("pause lock")
        .push(AtomicWritePause {
            path: file.clone(),
            entered: Some(entered_tx),
            release: Arc::clone(&release),
        });

    let ws_save = Arc::clone(&workspace);
    let document_id = opened.document_id;
    let save =
        tokio::spawn(async move { save_document_unlocked(&ws_save, document_id, 1, 2).await });
    tokio::time::timeout(std::time::Duration::from_secs(2), entered_rx)
        .await
        .expect("save must pause after releasing the document mutex")
        .expect("pause sender dropped");

    {
        let mut document = opened.document.lock().await;
        assert_eq!(
            document.apply_edit(
                opened.document_id,
                1,
                Some(1),
                2,
                91,
                EditOperation::Insert {
                    byte_offset: 11,
                    text: "!".to_string(),
                },
            ),
            ServerMessage::EditAck {
                document_id: opened.document_id,
                confirmed_version: 3,
                transaction_id: 91,
            }
        );
    }

    release.notify_one();
    let outcome = save.await.unwrap().unwrap();
    assert!(
        outcome.dirty,
        "older snapshot commit must leave a newer edit dirty"
    );
    assert!(opened.document.lock().await.is_dirty());
    assert_eq!(fs::read_to_string(&file).unwrap(), "hello world");
    assert_eq!(opened.document.lock().await.text(), "hello world!");

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
    let open_a =
        tokio::spawn(
            async move { open_existing_file_unlocked(&ws_a, root_id, "shared.txt", 1).await },
        );
    let open_b =
        tokio::spawn(
            async move { open_existing_file_unlocked(&ws_b, root_id, "shared.txt", 2).await },
        );
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

/// A file that passes prepare-time metadata validation but grows before
/// EOF stays within its reserved session-budget slice and is rejected
/// rather than allowing an unbounded read (Plan 060 T5).
#[tokio::test]
async fn streamed_read_rejects_file_that_grows_past_reservation() {
    let root = temp_workspace("grow-during-read");
    let file = root.join("grow.txt");
    let initial = "a".repeat(64 * 1024);
    fs::write(&file, &initial).unwrap();
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_root(&root).unwrap();

    let grow_path = file.clone();
    BETWEEN_METADATA_AND_READ_HOOKS
        .lock()
        .expect("hook lock")
        .push((
            grow_path.clone(),
            Box::new(move || {
                fs::write(&grow_path, format!("{initial}b")).unwrap();
            }),
        ));

    let error = workspace
        .open_existing_file(root_id, "grow.txt", 1)
        .await
        .unwrap_err();
    assert_eq!(
        error.diagnostic().code,
        FileErrorCode::DocumentBudgetExceeded
    );
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
    assert!(path_rules.is_ignored(std::path::Path::new("packages/markdown/node_modules"), true));
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
fn user_browse_depth1_listing_returns_immediate_children_only() {
    let root = temp_workspace("user-browse-depth1");
    fs::create_dir_all(root.join("sub").join("nested")).unwrap();
    fs::write(root.join("sub").join("deep.txt"), "x").unwrap();
    fs::write(root.join("a.txt"), "a").unwrap();
    fs::write(root.join("b.md"), "bb").unwrap();

    let page = traverse_user_browse_directory(UserBrowseListingPlan {
        target: root.clone(),
        max_entries: 64,
    })
    .unwrap();

    assert_eq!(page.canonical_dir, fs::canonicalize(&root).unwrap());
    assert!(!page.truncated);
    let names: Vec<&str> = page.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["a.txt", "b.md", "sub"]);
    // Depth-1: nested children never appear.
    assert!(
        page.entries
            .iter()
            .all(|e| e.canonical_path.parent() == Some(page.canonical_dir.as_path()))
    );
    // Every activation path is canonical and absolute.
    assert!(page.entries.iter().all(|e| e.canonical_path.is_absolute()));
    let file = page.entries.iter().find(|e| e.name == "a.txt").unwrap();
    assert_eq!(file.kind, UserBrowseEntryKind::File);
    assert_eq!(file.size, Some(1));
    let dir = page.entries.iter().find(|e| e.name == "sub").unwrap();
    assert_eq!(dir.kind, UserBrowseEntryKind::Directory);
    assert_eq!(dir.size, None);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn user_browse_listing_truncates_deterministically_at_cap() {
    let root = temp_workspace("user-browse-cap");
    fs::write(root.join("d.txt"), "d").unwrap();
    fs::write(root.join("a.txt"), "a").unwrap();
    fs::write(root.join("c.txt"), "c").unwrap();
    fs::write(root.join("b.txt"), "b").unwrap();
    fs::write(root.join("e.txt"), "e").unwrap();

    // The lexicographically-first names survive regardless of read_dir
    // order, and the cap never exceeds the transient-menu ceiling.
    let page = traverse_user_browse_directory(UserBrowseListingPlan {
        target: root.clone(),
        max_entries: 3,
    })
    .unwrap();
    let names: Vec<&str> = page.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["a.txt", "b.txt", "c.txt"]);
    assert!(page.truncated);
    assert!(page.entries.len() <= 3);

    // A directory with exactly the cap entries is not truncated.
    let page = traverse_user_browse_directory(UserBrowseListingPlan {
        target: root.clone(),
        max_entries: 5,
    })
    .unwrap();
    assert!(!page.truncated);
    assert_eq!(page.entries.len(), 5);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn user_browse_listing_fails_cleanly_without_grant_mutation() {
    let root = temp_workspace("user-browse-fail");
    fs::write(root.join("plain.txt"), "x").unwrap();
    let workspace = WorkspaceState::new();

    // Missing target.
    let error = traverse_user_browse_directory(UserBrowseListingPlan {
        target: root.join("missing"),
        max_entries: 64,
    })
    .unwrap_err();
    assert!(matches!(error, UserBrowseError::Unavailable { .. }));

    // Non-directory target.
    let error = traverse_user_browse_directory(UserBrowseListingPlan {
        target: root.join("plain.txt"),
        max_entries: 64,
    })
    .unwrap_err();
    assert!(matches!(error, UserBrowseError::NotADirectory { .. }));

    // No browse ever mutates workspace grants.
    assert_eq!(workspace.list_root_metadata().len(), 0);

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn user_browse_listing_resolves_symlink_targets_and_skips_broken_links() {
    let root = temp_workspace("user-browse-symlink");
    fs::create_dir(root.join("real")).unwrap();
    fs::write(root.join("real").join("f.txt"), "x").unwrap();
    fs::write(root.join("plain.txt"), "x").unwrap();
    std::os::unix::fs::symlink(root.join("real"), root.join("linked")).unwrap();
    std::os::unix::fs::symlink(root.join("nowhere"), root.join("broken")).unwrap();

    let page = traverse_user_browse_directory(UserBrowseListingPlan {
        target: root.clone(),
        max_entries: 64,
    })
    .unwrap();

    // Symlink-to-directory lists as a Directory with the canonical
    // target path, ready for activation.
    let linked = page.entries.iter().find(|e| e.name == "linked").unwrap();
    assert_eq!(linked.kind, UserBrowseEntryKind::Directory);
    assert_eq!(
        linked.canonical_path,
        fs::canonicalize(root.join("real")).unwrap()
    );
    // Broken symlink child is skipped, never a panic.
    assert!(page.entries.iter().all(|e| e.name != "broken"));
    let names: Vec<&str> = page.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["linked", "plain.txt", "real"]);

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn user_browse_seed_prefers_active_document_canonical_parent() {
    let root = temp_workspace("user-browse-seed-doc");
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("main.rs"), "fn main() {}\n").unwrap();
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
    let root_id = workspace.lock().await.add_root(&root).unwrap();
    let lease = workspace
        .lock()
        .await
        .register_loaded_file(
            root_id,
            src.join("main.rs"),
            "fn main() {}\n".to_string(),
            1,
        )
        .await
        .unwrap();

    let seed = resolve_user_browse_seed(&workspace, Some(lease.document_id), None).await;
    assert_eq!(seed, fs::canonicalize(&src).unwrap());

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn user_browse_seed_uses_real_canonical_parent_of_selected_file() {
    // Selected-file (SingleFile grant) opens seed from their actual
    // canonical parent, not any workspace-relative path.
    let outside = temp_workspace("user-browse-seed-selected");
    let file = outside.join("notes.md");
    fs::write(&file, "# notes").unwrap();
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
    let lease = open_selected_file_unlocked(&workspace, &file, 1)
        .await
        .unwrap();

    let seed = resolve_user_browse_seed(&workspace, Some(lease.document_id), None).await;
    assert_eq!(seed, fs::canonicalize(&outside).unwrap());

    let _ = fs::remove_dir_all(outside);
}

#[tokio::test]
async fn user_browse_seed_falls_back_to_tab_root_then_cwd() {
    let root = temp_workspace("user-browse-seed-tab");
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

    // Tab directory root wins over cwd.
    let seed = resolve_user_browse_seed(&workspace, None, Some(root.to_str().unwrap())).await;
    assert_eq!(seed, root);

    // Empty tab root falls through to the canonical cwd (no chdir needed:
    // the seed is compared against the caller's real current directory).
    let expected_cwd = fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
    let seed = resolve_user_browse_seed(&workspace, None, Some("")).await;
    assert_eq!(seed, expected_cwd);

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn user_browse_listing_runs_without_holding_workspace_lock() {
    let root = temp_workspace("user-browse-concurrent");
    fs::write(root.join("a.txt"), "a").unwrap();
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));
    let root_id = workspace.lock().await.add_root(&root).unwrap();
    let plan = UserBrowseListingPlan {
        target: root.clone(),
        max_entries: 64,
    };
    let listing = tokio::spawn(execute_user_browse_listing(plan));

    // Hold the workspace lock across the whole listing: traversal must
    // complete anyway because it reads no workspace state.
    let page = {
        let _guard = workspace.lock().await;
        listing.await.unwrap().unwrap()
    };
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].name, "a.txt");

    // Unrelated workspace operations progressed concurrently.
    assert_eq!(
        workspace.lock().await.list_root_metadata()[0].workspace_root_id,
        root_id
    );

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
