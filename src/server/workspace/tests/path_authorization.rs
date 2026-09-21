use super::*;

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
