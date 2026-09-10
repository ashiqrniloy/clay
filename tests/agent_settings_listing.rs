//! Plan 117 follow-up regression: the agent-settings listing must survive the
//! whole trip from the server to the webview-facing event lane.
//!
//! The server answered `ListAgentSettingsFiles` from the day it shipped, but
//! the client read loop had no arm for `ServerMessage::AgentSettingsFiles`, so
//! it fell through the catch-all `Ok(_) => {}` and the reply never became a
//! `ClientConnectionEvent`. The Settings tab therefore sat on "Loading…"
//! forever while every unit test passed (they all injected the envelope
//! directly). This test drives the real `clay::client` connection — the layer
//! that had the hole — instead of a raw socket.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use clay::client::{ClientConnectionEvent, connect_with_workspace_root};
use clay::ipc::smoke_endpoint;
use clay::protocol::ClientMessage;
use clay::server::{IpcServer, ServerConfig};

fn unique_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "clay-agent-settings-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

/// Seed the per-agent config root the way the daemon does, plus a matching
/// `.seed-manifest.json` stamp so one file reads built-in and the other (no
/// stamp) reads edited — the two provenance states the tab renders.
fn seed_agent_config(configuration_root: &Path) {
    let config_root = configuration_root.join("agents").join("coding-agent");
    fs::create_dir_all(config_root.join("skills").join("graft")).unwrap();
    fs::write(config_root.join("SYSTEM.md"), "").unwrap();
    fs::write(
        config_root.join("skills").join("graft").join("SKILL.md"),
        "---\nname: graft\ndescription: d\n---\n\nbody\n",
    )
    .unwrap();
    let metadata = fs::metadata(config_root.join("SYSTEM.md")).unwrap();
    let mtime_ms = u64::try_from(
        metadata
            .modified()
            .unwrap()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    fs::write(
        config_root.join(".seed-manifest.json"),
        format!(
            r#"{{"SYSTEM.md":{{"sizeBytes":{},"mtimeMs":{}}}}}"#,
            metadata.len(),
            mtime_ms
        ),
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_settings_listing_reaches_the_client_event_lane() {
    let configuration_root = unique_root("listing");
    seed_agent_config(&configuration_root);
    let workspace_root = unique_root("listing-ws");

    let endpoint = smoke_endpoint("agent-settings-listing");
    let mut config = ServerConfig::new(endpoint.clone());
    config.configuration_root = Some(configuration_root.clone());
    let server = IpcServer::try_new(config).expect("server config is valid");
    let runner = tokio::spawn(server.run());

    // The real client connection: Hello + workspace tab bind happen inside,
    // so this is exactly the path the desktop bridge uses.
    let session = loop {
        match connect_with_workspace_root(&endpoint, workspace_root.display().to_string()).await {
            Ok(session) => break session,
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    };
    let client_id = session.initial_state.client_id;
    let mut events = session.events;

    session
        .edit_queue
        .enqueue_raw(ClientMessage::ListAgentSettingsFiles { client_id })
        .expect("listing request enqueues");

    let mut listing = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, events.recv()).await {
        if let ClientConnectionEvent::AgentSettingsFiles { client_id, files } = event {
            listing = Some((client_id, files));
            break;
        }
    }

    let (event_client_id, files) = listing.expect(
        "ServerMessage::AgentSettingsFiles must become a client event (the read loop dropped it)",
    );
    assert_eq!(event_client_id, client_id);
    let names: Vec<&str> = files.iter().map(|file| file.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["SYSTEM.md", "skills/graft/SKILL.md"],
        "the listing covers the delivered layout, sorted"
    );
    let system = files.iter().find(|f| f.name == "SYSTEM.md").unwrap();
    let graft = files
        .iter()
        .find(|f| f.name.ends_with("graft/SKILL.md"))
        .unwrap();
    assert!(!system.edited, "a stamped seed reads built-in");
    assert!(graft.edited, "an unstamped file reads edited");

    runner.abort();
    let _ = fs::remove_dir_all(&configuration_root);
    let _ = fs::remove_dir_all(&workspace_root);
}
