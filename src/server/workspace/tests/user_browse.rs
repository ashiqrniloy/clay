use super::*;

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
