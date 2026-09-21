use super::*;

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
