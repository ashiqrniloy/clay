use super::*;

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
async fn streamed_read_carries_utf8_scalars_across_read_boundaries() {
    let root = temp_workspace("utf8-carry");
    // A 4-byte scalar split 1..=3 bytes before a 64 KiB read boundary must
    // reassemble byte-identically: plan 119 P1-1 rewrote the carry buffer, so
    // this pins the byte-split contract for every split position.
    for split in 1..=3usize {
        let scalar = "\u{1F980}";
        let mut text = "a".repeat(FILE_READ_BUFFER_BYTES - (scalar.len() - split));
        text.push_str(scalar);
        text.push_str("\u{00E9}\u{4E2D}tail");
        let file = root.join(format!("carry-{split}.txt"));
        fs::write(&file, text.as_bytes()).unwrap();

        let (rope, _) = match read_file_streamed(&file, &file, u64::MAX, 0).await {
            Ok(read) => read,
            Err(error) => panic!("split scalar {split} must reassemble, got {error:?}"),
        };
        assert_eq!(
            rope.byte_slice(0..rope.byte_len()).to_string(),
            text,
            "scalar split {split} bytes before the read boundary reassembled wrong"
        );
    }

    // A scalar truncated at EOF stays a typed refusal after the rewrite.
    let truncated = root.join("truncated.txt");
    let mut bytes = "b".repeat(FILE_READ_BUFFER_BYTES + 1).into_bytes();
    bytes.extend_from_slice(&[0xF0, 0x9F]);
    fs::write(&truncated, &bytes).unwrap();
    let error = match read_file_streamed(&truncated, &truncated, u64::MAX, 0).await {
        Ok(_) => panic!("truncated scalar must be refused"),
        Err(error) => error,
    };
    assert!(matches!(error, WorkspaceError::InvalidUtf8 { .. }));

    let _ = fs::remove_dir_all(root);
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
