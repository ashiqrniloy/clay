use super::*;

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
    let plan = workspace.prepare_save(document_id).await.unwrap();

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
