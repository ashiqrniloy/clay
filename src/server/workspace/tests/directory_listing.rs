use super::*;

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
