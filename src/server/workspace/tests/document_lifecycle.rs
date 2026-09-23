use super::*;

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
