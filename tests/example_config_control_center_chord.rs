//! Example-configuration boot parity. Launching the real server against a copy
//! of the shipped example tree (`cp -r examples/config/. ~/.clay/`) is what
//! `scripts/build.sh run` does, so these tests are the regression net for the
//! canonical example itself:
//!
//! - Plan 117 follow-up, re-pointed by Plan 124: the canonical example keeps
//!   the Global `Ctrl+X Ctrl+O` → `controlCenter.open` palette chord and the
//!   separate Global `Ctrl+X Ctrl+P` → `shell.toggleAgentLane` lane chord, and
//!   the `controlCenter.open` command intent still round-trips to a
//!   TransientMenuSnapshot.
//! - Plan 118: the example's design-system selection boots the shipped Quiet
//!   Instrument system (bundled resolution, no `loadPackage`), the Settings
//!   choice set is the shipped one, and nothing removed is requested or fails
//!   to load.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use clay::ipc::smoke_endpoint;
use clay::protocol::{
    ClientMessage, KeyCode, PROTOCOL_VERSION, PackageUiTrustDomain, ServerMessage, TabCommand,
    codec::Codec,
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

fn has_ctrl_x_chord(
    manifest: &clay::protocol::BehaviorManifest,
    command_id: &str,
    second_stroke: &str,
) -> bool {
    manifest.keymaps.iter().any(|rule| {
        rule.command_id == command_id
            && rule.sequence.len() == 2
            && rule.sequence[0].key == KeyCode::Character("x".to_string())
            && rule.sequence[0].modifiers.control
            && rule.sequence[1].key == KeyCode::Character(second_stroke.to_string())
            && rule.sequence[1].modifiers.control
    })
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
    // default palette chord (Plan 124 re-anchor) and the separate lane-toggle
    // chord both ship even with the example config's bindKey tables active.
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
                if has_ctrl_x_chord(&manifest, "controlCenter.open", "o")
                    && has_ctrl_x_chord(&manifest, "shell.toggleAgentLane", "p")
                {
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
        "the example config must keep the Global Ctrl+X Ctrl+O controlCenter.open \
         palette chord and the separate Global Ctrl+X Ctrl+P shell.toggleAgentLane chord"
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
async fn example_config_boot_activates_the_shipped_design_system() {
    // Plan 118: a copied example config must reach the shipped Quiet Instrument
    // system by itself. Booting the real server (instead of grepping init.js)
    // proves the active selection resolves from the compiled bundled inventory
    // with no loadPackage call, that the choice set the Settings panel
    // enumerates is the shipped one, and that every requested package resolves —
    // no removed specifier is asked for and nothing fails closed.
    let root = unique_root("design-system");
    copy_example_config(&root);

    let endpoint = smoke_endpoint("example-config-design-system");
    let mut config = ServerConfig::new(endpoint.clone());
    config.configuration_root = Some(root.clone());
    let server = IpcServer::try_new(config).expect("example config server config is valid");
    let runner = tokio::spawn(server.run());

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
                client_name: "example-config-design-system-test".to_string(),
            },
        )
        .await
        .unwrap();

    let mut state = None;
    for _ in 0..400 {
        let frame = tokio::time::timeout(
            Duration::from_secs(10),
            codec.read_server_message(&mut stream),
        )
        .await
        .expect("bootstrap frame timeout")
        .unwrap();
        match frame {
            ServerMessage::RuntimeStateSnapshot(snapshot) => {
                state = Some(*snapshot);
                break;
            }
            ServerMessage::Error { code, message } => {
                panic!("server errored during bootstrap: {code:?} {message}");
            }
            _ => continue,
        }
    }
    let state = state.expect("bootstrap must install a runtime state snapshot");

    // 1. The active system is the shipped package, trusted, with real recipes —
    //    and it carries the approved Quiet Instrument geometry, not a stale or
    //    defaulted blob (8px control radius, 1px hairline, 150ms state motion).
    let active = &state.active_design_system;
    assert_eq!(
        active.specifier, "@clay/design-instrument",
        "the canonical example must boot the shipped design system"
    );
    assert_eq!(
        active.provenance.trust_domain,
        PackageUiTrustDomain::Trusted,
        "bundled first-party provenance must be Trusted"
    );
    assert!(
        !active.recipes.is_empty(),
        "the shipped selection must resolve recipes, not just a specifier"
    );
    let button = active
        .recipes
        .get(
            &clay::shell::design_system::RecipeKey::parse("button.default.root.rest")
                .expect("parse recipe key"),
        )
        .expect("the shipped system must resolve button.default.root.rest");
    assert!(
        (button.border_radius - 8.0).abs() < f64::EPSILON,
        "control radius ladder"
    );
    assert!(
        (button.border_width - 1.0).abs() < f64::EPSILON,
        "hairline border width"
    );
    assert!(
        (button.transition_duration - 150.0).abs() < f64::EPSILON,
        "motion.fast tier"
    );

    // 2. The Settings panel enumerates the shipped choice set: @clay/core first,
    //    then the bundled contributor (no third-party system is enabled here).
    let design_systems: Vec<&str> = state
        .ui_choices
        .design_systems
        .iter()
        .map(|choice| choice.specifier.as_str())
        .collect();
    assert_eq!(
        design_systems,
        vec!["@clay/core", "@clay/design-instrument"],
        "the copied example must offer exactly the shipped design systems"
    );

    // 3. Nothing removed is requested, and nothing fails closed: a removed
    //    package line, a removed specifier, or a broken bundled record would
    //    surface as a load/module diagnostic here.
    for diagnostic in &state.diagnostics {
        let text = format!("{} {}", diagnostic.code, diagnostic.message).to_lowercase();
        for needle in [
            "@clay/chat",
            "design-neobrutal",
            "design-glass",
            "neobrutal",
            "chat.entry",
        ] {
            assert!(
                !text.contains(needle),
                "the example must not request a removed surface: {diagnostic:?}"
            );
        }
        for code in [
            "theme.load_failed",
            "package.load_failed",
            "configuration.module_failed",
        ] {
            assert!(
                !text.contains(code),
                "the canonical example must boot clean: {diagnostic:?}"
            );
        }
    }

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
