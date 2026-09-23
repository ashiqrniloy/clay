use super::*;

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
async fn release_single_document_access_releases_bytes_with_last_holder() {
    let root = temp_workspace("release-single-access");
    fs::write(root.join("note.txt"), "hello").unwrap();
    let mut workspace = WorkspaceState::new();
    let root_id = workspace.add_root(&root).unwrap();
    let opened = workspace
        .open_existing_file(root_id, "note.txt", 0)
        .await
        .unwrap();
    let document_id = opened.document_id;
    // Second client, same file: two access holders on one registry entry and
    // one resident-memory charge.
    workspace
        .open_existing_file(root_id, "note.txt", 7)
        .await
        .unwrap();
    assert_eq!(workspace.resident_document_bytes, 5);

    assert!(
        !workspace
            .release_single_document_access(document_id, 7)
            .await,
        "releasing one of two holders keeps the document registered"
    );
    assert!(workspace.document_handle(document_id).is_some());
    assert_eq!(workspace.resident_document_bytes, 5);

    assert!(
        workspace
            .release_single_document_access(document_id, 0)
            .await,
        "releasing the last holder removes the document"
    );
    assert!(workspace.document_handle(document_id).is_none());
    assert_eq!(workspace.resident_document_bytes, 0);

    let _ = fs::remove_file(root.join("note.txt"));
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
