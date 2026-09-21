use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    ipc::IpcEndpoint,
    server::{IpcServer, ServerConfig, workspace::open_existing_file_unlocked},
};

fn temp_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "clay-tab-state-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("test root directory can be created");
    root
}

#[tokio::test]
async fn new_tab_states_keep_roots_and_documents_disjoint() {
    let root_a = temp_root("alpha");
    let root_b = temp_root("beta");
    fs::write(root_a.join("one.txt"), "one").expect("alpha file can be written");
    fs::write(root_a.join("two.txt"), "two").expect("alpha file can be written");
    fs::write(root_b.join("one.txt"), "other").expect("beta file can be written");

    let mut config = ServerConfig::new(IpcEndpoint::from_argument("tab-state-test"));
    config.workspace_roots.push(root_a.clone());
    let server = IpcServer::new(config);
    let (alpha_snapshot, _) = server
        .create_tab_state(1, root_a.to_string_lossy().into_owned())
        .await
        .expect("bootstrap tab state can be created");
    let alpha_tab = alpha_snapshot.tabs[0].tab_id;
    let (beta_snapshot, _) = server
        .create_tab_state(2, root_b.to_string_lossy().into_owned())
        .await
        .expect("second tab state can be created");
    let beta_tab = beta_snapshot.tabs[1].tab_id;

    let alpha = server
        .tab_state(alpha_tab)
        .await
        .expect("alpha state is installed");
    let beta = server
        .tab_state(beta_tab)
        .await
        .expect("beta state is installed");
    let alpha_root = alpha
        .workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .next()
        .expect("alpha has one root");
    let beta_root = beta
        .workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .next()
        .expect("beta has one root");
    assert_eq!(
        alpha_root.canonical_path,
        fs::canonicalize(&root_a).unwrap()
    );
    assert_eq!(beta_root.canonical_path, fs::canonicalize(&root_b).unwrap());
    // Visible by default; the toggle flips each tab independently.
    assert!(alpha.workspace_pane_visible());
    assert!(beta.workspace_pane_visible());
    assert!(!alpha.toggle_workspace_pane());
    assert!(!alpha.workspace_pane_visible());
    assert!(beta.workspace_pane_visible());
    assert!(!beta.toggle_workspace_pane());
    assert!(!beta.workspace_pane_visible());
    assert!(!alpha.workspace_pane_visible());
    assert_ne!(
        alpha.welcome.lock().await.document_id(),
        beta.welcome.lock().await.document_id()
    );

    let alpha_one =
        open_existing_file_unlocked(&alpha.workspace, alpha_root.workspace_root_id, "one.txt", 1)
            .await
            .expect("alpha file opens in alpha state");
    let alpha_two =
        open_existing_file_unlocked(&alpha.workspace, alpha_root.workspace_root_id, "two.txt", 1)
            .await
            .expect("second alpha file opens in alpha state");
    let beta_one =
        open_existing_file_unlocked(&beta.workspace, beta_root.workspace_root_id, "one.txt", 2)
            .await
            .expect("beta file opens in beta state");
    assert_ne!(alpha_one.document_id, alpha_two.document_id);
    assert_ne!(alpha_one.document_id, beta_one.document_id);
    assert_eq!(
        alpha
            .workspace
            .lock()
            .await
            .list_documents(1)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        beta.workspace
            .lock()
            .await
            .list_documents(2)
            .await
            .unwrap()
            .len(),
        1
    );

    fs::remove_dir_all(root_a).expect("alpha test root can be removed");
    fs::remove_dir_all(root_b).expect("beta test root can be removed");
}
