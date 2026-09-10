//! Plan 117 follow-up regression: launching the real server against a copy of
//! the shipped example configuration must still publish the Global
//! `Ctrl+X Ctrl+P` → `controlCenter.open` default keymap, and the
//! `controlCenter.open` command intent must still round-trip to a
//! TransientMenuSnapshot. This mirrors `scripts/build.sh run` with the
//! example config copied over a fresh root.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use clay::ipc::smoke_endpoint;
use clay::protocol::{
    ClientMessage, KeyCode, PROTOCOL_VERSION, ServerMessage, TabCommand, codec::Codec,
};
use clay::server::{IpcServer, ServerConfig};

fn unique_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "clay-example-config-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn copy_example_config(destination: &Path) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/config");
    copy_dir(&source, destination);
}

fn copy_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn example_config_boot_publishes_the_control_center_chord() {
    let root = unique_root("chord");
    copy_example_config(&root);

    let endpoint = smoke_endpoint("example-config-chord");
    let mut config = ServerConfig::new(endpoint.clone());
    config.configuration_root = Some(root.clone());
    let server = IpcServer::try_new(config).expect("example config server config is valid");
    let runner = tokio::spawn(server.run());

    // Wait for the socket, then connect like the desktop bridge does.
    let socket = endpoint.as_unix_socket_path().to_path_buf();
    let mut connected = None;
    for _ in 0..200 {
        if let Ok(stream) = tokio::net::UnixStream::connect(&socket).await {
            connected = Some(stream);
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let mut stream = connected.expect("server socket must accept connections");
    let codec = Codec::default();
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "example-config-chord-test".to_string(),
            },
        )
        .await
        .unwrap();

    // Collect bootstrap frames until the initial state settles; assert the
    // default Control Center chord ships even with the example config's
    // bindKey tables active.
    let mut saw_chord = false;
    let mut boot_version: Option<u64> = None;
    let mut client_id: Option<u64> = None;
    for _ in 0..400 {
        let frame = tokio::time::timeout(
            Duration::from_secs(10),
            codec.read_server_message(&mut stream),
        )
        .await
        .expect("bootstrap frame timeout")
        .unwrap();
        match frame {
            ServerMessage::Welcome { client_id: id, .. } => {
                client_id = Some(id);
            }
            ServerMessage::BehaviorManifest(manifest) => {
                boot_version = Some(manifest.behavior_version);
                if manifest.keymaps.iter().any(|rule| {
                    rule.command_id == "controlCenter.open"
                        && rule.sequence.len() == 2
                        && rule.sequence[0].key == KeyCode::Character("x".to_string())
                        && rule.sequence[0].modifiers.control
                        && rule.sequence[1].key == KeyCode::Character("p".to_string())
                        && rule.sequence[1].modifiers.control
                }) {
                    saw_chord = true;
                    break;
                }
            }
            ServerMessage::Error { code, message } => {
                panic!("server errored during bootstrap: {code:?} {message}");
            }
            _ => continue,
        }
    }
    assert!(
        saw_chord,
        "the example config must keep the Global Ctrl+X Ctrl+P controlCenter.open default"
    );
    let boot_version = boot_version.expect("manifest carried a behavior version");
    let client_id = client_id.expect("welcome must assign the client id");

    // Bind the connection to a workspace tab like the desktop bridge does.
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::TabCommand {
                client_id,
                command: TabCommand::New {
                    workspace_root: root.to_string_lossy().to_string(),
                },
            },
        )
        .await
        .unwrap();

    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::CommandIntent {
                client_id,
                document_id: 1,
                behavior_version: boot_version,
                command_id: "controlCenter.open".to_string(),
            },
        )
        .await
        .unwrap();
    let mut menu = None;
    for _ in 0..50 {
        let frame = tokio::time::timeout(
            Duration::from_secs(10),
            codec.read_server_message(&mut stream),
        )
        .await
        .expect("intent reply timeout")
        .unwrap();
        match frame {
            ServerMessage::TransientMenuSnapshot(snapshot) => {
                menu = Some(*snapshot);
                break;
            }
            ServerMessage::Error { code, message } => {
                panic!("controlCenter.open intent rejected: {code:?} {message}");
            }
            _ => continue,
        }
    }
    let menu = menu.expect("controlCenter.open must answer with a transient menu snapshot");
    assert!(
        !menu.items.is_empty(),
        "the control centre must list at least the built-in commands"
    );

    runner.abort();
    let _ = fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread")]
async fn example_config_bootstrap_manifest_json_shape() {
    // Prints the wire JSON of the bootstrap manifest so the frontend chord
    // matcher's expectations (camelCase commandId/sequence/context/
    // routingPolicy, key.character) can be checked against reality.
    let root = unique_root("json");
    copy_example_config(&root);
    let endpoint = smoke_endpoint("example-config-json");
    let mut config = ServerConfig::new(endpoint.clone());
    config.configuration_root = Some(root.clone());
    let server = IpcServer::try_new(config).unwrap();
    let runner = tokio::spawn(server.run());
    let socket = endpoint.as_unix_socket_path().to_path_buf();
    let mut stream = None;
    for _ in 0..200 {
        if let Ok(s) = tokio::net::UnixStream::connect(&socket).await {
            stream = Some(s);
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let mut stream = stream.expect("socket");
    let codec = Codec::default();
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "shape-test".to_string(),
            },
        )
        .await
        .unwrap();
    for _ in 0..100 {
        match tokio::time::timeout(
            Duration::from_secs(10),
            codec.read_server_message(&mut stream),
        )
        .await
        .expect("frame timeout")
        .unwrap()
        {
            ServerMessage::BehaviorManifest(manifest) => {
                let json = serde_json::to_string_pretty(&*manifest).unwrap();
                let value: serde_json::Value = serde_json::from_str(&json).unwrap();
                let keymaps = value.get("keymaps").expect("keymaps key");
                println!(
                    "MANIFEST-JSON keymaps[0..2]: {}",
                    serde_json::to_string_pretty(
                        &keymaps
                            .as_array()
                            .unwrap()
                            .iter()
                            .take(2)
                            .collect::<Vec<_>>()
                    )
                    .unwrap()
                );
                let control_center = keymaps
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|rule| {
                        rule.get("commandId").and_then(|v| v.as_str()) == Some("controlCenter.open")
                    })
                    .expect("controlCenter.open rule present")
                    .clone();
                println!("CONTROL-CENTER-RULE: {control_center}");
                break;
            }
            _ => continue,
        }
    }
    runner.abort();
    let _ = fs::remove_dir_all(root);
}
