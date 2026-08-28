//! Phase 25 agent IPC + clay-agent process manager.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use clay::perf::budgets::{
    AGENT_DAEMON_SPAWN_P95_BUDGET_MS, AGENT_PROMPT_TO_FIRST_DELTA_P95_BUDGET_MS,
    KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS,
};
use clay::protocol::{
    AgentClientCommand, AgentPickerKind, AgentSecret, AgentServerMessage, AgentToolPhase,
    AgentWireEvent, ClientMessage, PROTOCOL_VERSION, ServerMessage,
    codec::{Codec, CodecError},
};
use clay::server::agent::{AgentHost, AgentHostConfig};

mod common;
use common::{assert_absent, non_test, read_src};

const FRAME_PREFIX_BYTES: usize = 4;

fn payload_len(frame: &[u8]) -> usize {
    frame.len() - FRAME_PREFIX_BYTES
}

fn temp_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "clay-agent-host-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn mock_daemon() -> PathBuf {
    let dir = temp_dir("mock");
    let path = dir.join("mock-agent");
    fs::write(
        &path,
        r#"#!/usr/bin/env python3
import json, sys, time
for line in sys.stdin:
    msg = json.loads(line)
    ident = msg.get("id")
    method = msg.get("method")
    params = msg.get("params") or {}
    if method == "initialize":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"ok":True}}), flush=True)
    elif method == "session.prompt":
        sid = params.get("sessionId","")
        print(json.dumps({"jsonrpc":"2.0","method":"event","params":{"sessionId":sid,"event":{"type":"message_delta","runId":"r1","content":{"type":"text","text":"hi"}}}}), flush=True)
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"lastEvent":"agent_finished"}}), flush=True)
    elif method == "session.new":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessionId":"s1","profile":params.get("profile"),"provider":params.get("provider"),"model":params.get("model")}}), flush=True)
    elif method == "provider.list":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"providers":[{"id":"mock"}]}}), flush=True)
    elif method == "model.list":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"models":[{"provider":"mock","model":"demo","displayName":"Demo"}]}}), flush=True)
    elif method == "agentProfile.list":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"profiles":[{"name":"chat"}]}}), flush=True)
    elif method == "session.list":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessions":[]}}), flush=True)
    elif method == "session.load":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessionId":params.get("sessionId"),"profile":"chat","metadata":{"provider":"mock","model":"demo"},"entries":[{"role":"user","content":{"text":"Hi"}},{"role":"assistant","content":{"text":"hello"}}]}}), flush=True)
    elif method == "credential.put":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"stored":True,"provider":params.get("provider")}}), flush=True)
    elif method == "shutdown":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"ok":True}}), flush=True)
        break
    else:
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{}}), flush=True)
"#,
    )
    .unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path
}

fn slow_daemon() -> PathBuf {
    let dir = temp_dir("slow");
    let path = dir.join("slow-agent");
    fs::write(
        &path,
        r#"#!/usr/bin/env python3
import json, sys, time
for line in sys.stdin:
    msg = json.loads(line)
    ident = msg.get("id")
    method = msg.get("method")
    if method == "initialize":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"ok":True}}), flush=True)
    elif method == "session.prompt":
        time.sleep(2)
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"lastEvent":"agent_finished"}}), flush=True)
    elif method == "shutdown":
        break
    else:
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{}}), flush=True)
"#,
    )
    .unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path
}

fn host_for(program: PathBuf) -> AgentHost {
    AgentHost::new(AgentHostConfig {
        program,
        args: Vec::new(),
        data_dir: temp_dir("data"),
        inherit_environment: Vec::new(),
        inert: false,
    })
}

fn every_client_command() -> Vec<AgentClientCommand> {
    vec![
        AgentClientCommand::Prompt {
            session_id: "s1".into(),
            text: "hi".into(),
        },
        AgentClientCommand::Cancel {
            session_id: "s1".into(),
        },
        AgentClientCommand::Steer {
            session_id: "s1".into(),
            text: "nope".into(),
            soft_interrupt: false,
        },
        AgentClientCommand::NewSession {
            profile: "chat".into(),
            provider: "mock".into(),
            model: "demo".into(),
        },
        AgentClientCommand::LoadSession {
            session_id: "s1".into(),
        },
        AgentClientCommand::ResumeSession {
            session_id: "s1".into(),
        },
        AgentClientCommand::DeleteSession {
            session_id: "s1".into(),
        },
        AgentClientCommand::ListSessions,
        AgentClientCommand::OpenPicker {
            kind: AgentPickerKind::Provider,
        },
        AgentClientCommand::Select {
            kind: AgentPickerKind::Model,
            id: "mock/demo".into(),
        },
        AgentClientCommand::CredentialPut {
            provider: "mock".into(),
            name: "apiKey".into(),
            secret: AgentSecret("sk-testsecretvalue999".into()),
        },
        AgentClientCommand::CredentialDelete {
            provider: "mock".into(),
            name: "apiKey".into(),
        },
        AgentClientCommand::RegisterProfile {
            name: "chat".into(),
            description: "Chat".into(),
            instructions: "Be brief.".into(),
        },
    ]
}

fn every_server_message() -> Vec<AgentServerMessage> {
    vec![
        AgentServerMessage::Snapshot(clay::protocol::AgentSessionSnapshot {
            session_id: "s1".into(),
            profile: "chat".into(),
            provider: "mock".into(),
            model: "demo".into(),
            leaf_id: None,
            entries: vec![clay::protocol::AgentTranscriptEntry {
                kind: clay::protocol::AgentTranscriptKind::User,
                text: "hi".into(),
            }],
        }),
        AgentServerMessage::Event {
            session_id: "s1".into(),
            event: AgentWireEvent::MessageDelta {
                session_id: "s1".into(),
                run_id: "r1".into(),
                text: "hello".into(),
            },
        },
        AgentServerMessage::Event {
            session_id: "s1".into(),
            event: AgentWireEvent::Tool {
                session_id: "s1".into(),
                run_id: "r1".into(),
                phase: AgentToolPhase::Started,
                name: "read".into(),
                tool_call_id: "c1".into(),
            },
        },
        AgentServerMessage::Event {
            session_id: "s1".into(),
            event: AgentWireEvent::Permission {
                session_id: "s1".into(),
                run_id: "r1".into(),
                request_id: "p1".into(),
                tool_name: "write".into(),
                allowed: None,
            },
        },
        AgentServerMessage::Inventory(clay::protocol::AgentInventory {
            providers: vec![clay::protocol::AgentProviderInfo {
                id: "mock".into(),
                configured: false,
            }],
            models: vec![],
            profiles: vec![],
            sessions: vec![],
        }),
        AgentServerMessage::Picker {
            kind: AgentPickerKind::Agent,
            items: vec![clay::protocol::AgentPickerItem {
                id: "chat".into(),
                label: "Chat".into(),
            }],
        },
        AgentServerMessage::CredentialAck {
            provider: "mock".into(),
            name: "apiKey".into(),
            stored: true,
        },
        AgentServerMessage::Diagnostic {
            code: "agent.node_missing".into(),
            message: "Node >= 20 is required".into(),
        },
    ]
}

#[test]
fn phase25_protocol_version_is_pinned() {
    assert_eq!(PROTOCOL_VERSION, 29);
}

#[test]
fn every_agent_client_command_round_trips_the_codec() {
    let codec = Codec::default();
    for command in every_client_command() {
        let message = ClientMessage::Agent {
            client_id: 7,
            command: Box::new(command),
        };
        let frame = codec.encode_client_message(&message).expect("encode");
        assert!(payload_len(&frame) <= codec.max_frame_size());
        assert_eq!(codec.decode_client_message(&frame).unwrap(), message);
    }
}

#[test]
fn every_agent_server_message_round_trips_the_codec() {
    let codec = Codec::default();
    for payload in every_server_message() {
        let message = ServerMessage::Agent(Box::new(payload));
        let frame = codec.encode_server_message(&message).expect("encode");
        assert!(payload_len(&frame) <= codec.max_frame_size());
        assert_eq!(codec.decode_server_message(&frame).unwrap(), message);
    }
}

#[test]
fn credential_put_debug_and_ack_omit_the_secret() {
    let secret = "sk-testsecretvalue999";
    let command = AgentClientCommand::CredentialPut {
        provider: "mock".into(),
        name: "apiKey".into(),
        secret: AgentSecret(secret.into()),
    };
    assert!(!format!("{command:?}").contains(secret));

    let ack = AgentServerMessage::CredentialAck {
        provider: "mock".into(),
        name: "apiKey".into(),
        stored: true,
    };
    let codec = Codec::default();
    let frame = codec
        .encode_server_message(&ServerMessage::Agent(Box::new(ack)))
        .unwrap();
    assert!(!String::from_utf8_lossy(&frame).contains(secret));
}

#[test]
fn truncated_invalid_and_oversized_agent_frames_fail_closed() {
    let codec = Codec::default();
    let valid = codec
        .encode_server_message(&ServerMessage::Agent(Box::new(
            AgentServerMessage::Diagnostic {
                code: "agent.error".into(),
                message: "x".into(),
            },
        )))
        .unwrap();
    let declared = payload_len(&valid);

    let truncated = valid[..valid.len() - 1].to_vec();
    assert!(matches!(
        codec.decode_server_message(&truncated),
        Err(CodecError::LengthMismatch { declared: got, actual })
            if got == declared && actual == truncated.len() - FRAME_PREFIX_BYTES
    ));

    let invalid = [4_u32.to_be_bytes().as_slice(), &[0xde, 0xad, 0xbe, 0xef]].concat();
    let invalid_result = std::panic::catch_unwind(|| codec.decode_server_message(&invalid))
        .expect("invalid archive must not panic");
    assert!(matches!(invalid_result, Err(CodecError::Deserialize(_))));

    let small = Codec::new(64);
    let oversized = (65_u32).to_be_bytes().to_vec();
    assert!(matches!(
        small.decode_server_message(&oversized),
        Err(CodecError::FrameTooLarge { len: 65, max: 64 })
    ));
}

#[test]
fn package_runtime_cannot_import_a_daemon_handle() {
    let ops_src = read_src("src/server/ops/mod.rs");
    assert_absent(
        non_test(&ops_src),
        &["op_clay_agent", "AgentHost", "clay-agent"],
        "package ops must not talk to the clay-agent pipe",
    );
    let facades_src = read_src("src/server/facades.rs");
    assert!(
        !non_test(&facades_src).contains("clay:agent"),
        "clay:agent facade is a later task; this task must not expose a JS daemon handle"
    );
    let trusted_src = read_src("src/server/js_runtime/mod.rs");
    assert_absent(
        non_test(&trusted_src),
        &["AgentHost", "op_clay_agent"],
        "package runtimes must not hold AgentHost",
    );
    assert!(
        clay::packages::manifest::RESERVED_CORE_API_DOMAINS.contains(&"agent"),
        "agent domain must be reserved"
    );
}

#[test]
fn agent_domain_is_reserved() {
    assert!(clay::packages::manifest::RESERVED_CORE_API_DOMAINS.contains(&"agent"));
}

#[tokio::test]
async fn missing_node_is_a_diagnostic_not_a_hang() {
    let host = AgentHost::new(AgentHostConfig {
        program: PathBuf::from("/nonexistent-clay-node-zzz"),
        args: Vec::new(),
        data_dir: temp_dir("missing-node"),
        inherit_environment: Vec::new(),
        inert: false,
    });
    let started = Instant::now();
    let message = host.run(AgentClientCommand::ListSessions).await;
    assert!(started.elapsed() < Duration::from_secs(1));
    match message {
        AgentServerMessage::Diagnostic { code, .. } => {
            assert_eq!(code, "agent.node_missing");
        }
        other => panic!("expected node-missing diagnostic, got {other:?}"),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn mock_daemon_prompt_persists_no_secret_on_ack() {
    let host = host_for(mock_daemon());
    let created = host
        .run(AgentClientCommand::NewSession {
            profile: "chat".into(),
            provider: "mock".into(),
            model: "demo".into(),
        })
        .await;
    let AgentServerMessage::Snapshot(snapshot) = created else {
        panic!("expected snapshot, got {created:?}");
    };
    assert_eq!(snapshot.session_id, "s1");

    let mut events = host.subscribe();
    let prompted = host
        .run(AgentClientCommand::Prompt {
            session_id: snapshot.session_id.clone(),
            text: "Hi".into(),
        })
        .await;
    assert!(matches!(prompted, AgentServerMessage::Snapshot(_)));
    let event = tokio::time::timeout(
        Duration::from_millis(AGENT_PROMPT_TO_FIRST_DELTA_P95_BUDGET_MS),
        events.recv(),
    )
    .await
    .expect("event")
    .expect("broadcast");
    assert!(matches!(
        event.as_ref(),
        AgentServerMessage::Event {
            event: AgentWireEvent::MessageDelta { text, .. },
            ..
        } if text == "hi"
    ));

    let secret = "sk-testsecretvalue999";
    let ack = host
        .run(AgentClientCommand::CredentialPut {
            provider: "mock".into(),
            name: "apiKey".into(),
            secret: AgentSecret(secret.into()),
        })
        .await;
    match ack {
        AgentServerMessage::CredentialAck { stored, .. } => assert!(stored),
        other => panic!("expected ack, got {other:?}"),
    }
    assert!(!format!("{ack:?}").contains(secret));
    host.shutdown().await;
}

#[tokio::test]
async fn unconfigured_prompt_is_instructional_snapshot() {
    let host = AgentHost::inert();
    let message = host.begin_prompt(1, "Hello").await;
    match message {
        AgentServerMessage::Snapshot(snapshot) => {
            assert!(snapshot.session_id.is_empty());
            assert!(snapshot.entries.is_empty());
        }
        other => panic!("expected empty snapshot, got {other:?}"),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn resume_after_daemon_load_restores_bounded_history() {
    let host = host_for(mock_daemon());
    let loaded = host.resume_tab(3, "s1").await;
    match loaded {
        AgentServerMessage::Snapshot(snapshot) => {
            assert_eq!(snapshot.session_id, "s1");
            assert_eq!(snapshot.entries.len(), 2);
            assert_eq!(
                snapshot.entries[0].kind,
                clay::protocol::AgentTranscriptKind::User
            );
            assert_eq!(snapshot.entries[1].text, "hello");
        }
        other => panic!("expected snapshot, got {other:?}"),
    }
    host.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn slow_daemon_submit_does_not_block_caller() {
    let host = host_for(slow_daemon());
    let started = Instant::now();
    host.dispatch(AgentClientCommand::Prompt {
        session_id: "s1".into(),
        text: "Hi".into(),
    });
    assert!(started.elapsed() < Duration::from_millis(KEYPRESS_TO_LOCAL_PAINT_P95_BUDGET_MS));
    host.shutdown().await;
}

#[test]
fn phase25_dependencies_deny_acp_agui_mcp() {
    let cargo = read_src("Cargo.toml");
    let agent_pkg = read_src("clay-agent/package.json");
    let agent_readme = read_src("clay-agent/README.md");
    let chat_docs = read_src("packages/chat/docs/index.md");
    for needle in [
        "prism-acp",
        "prism-ag-ui",
        "agentclientprotocol",
        "@modelcontextprotocol",
        "prism-coding-agent",
        "@arnilo/prism-coding-agent",
    ] {
        assert!(
            !cargo.contains(needle),
            "Cargo.toml must not depend on {needle}"
        );
        assert!(
            !agent_pkg.contains(needle),
            "clay-agent/package.json must not depend on {needle}"
        );
    }
    assert!(agent_readme.contains("0.3.0"));
    assert!(agent_readme.contains("Upgrade Prism"));
    assert!(agent_readme.contains("no tools and no sandbox"));
    assert!(chat_docs.contains("no tools, no sandbox"));
}

#[test]
fn agent_io_stays_off_paint_and_keypress() {
    let paint = common::hot_path_concat(&[
        "frontend/src/editor/ClayEditor.tsx",
        "frontend/src/editor/extensions/controller.ts",
        "frontend/src/editor/sync/session.ts",
    ]);
    assert_absent(
        &paint,
        &["AgentHost", "session.prompt", "clay-agent"],
        "paint/keypress must not talk to the daemon",
    );
    let spawn_src = read_src("src/server/agent.rs");
    let spawn = non_test(&spawn_src);
    assert!(spawn.contains("env_clear"));
    assert!(spawn.contains("inherit_environment: Vec::new()"));
    assert!(spawn.contains("fileMode: 0o600") || spawn.contains("mode(0o600)"));
}

#[cfg(unix)]
#[tokio::test]
async fn mock_spawn_creates_owner_only_passphrase_within_budget() {
    let data_dir = temp_dir("perms");
    let host = AgentHost::new(AgentHostConfig {
        program: mock_daemon(),
        args: Vec::new(),
        data_dir: data_dir.clone(),
        inherit_environment: Vec::new(),
        inert: false,
    });
    let started = Instant::now();
    let _ = host.run(AgentClientCommand::ListSessions).await;
    assert!(started.elapsed() < Duration::from_millis(AGENT_DAEMON_SPAWN_P95_BUDGET_MS));
    let meta = fs::metadata(data_dir.join("vault.passphrase")).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    host.shutdown().await;
}
