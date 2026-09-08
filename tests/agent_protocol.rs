//! Phase 25 agent IPC + clay-agent process manager.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
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

use serde_json::{Value, json};

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
REG = {}
for line in sys.stdin:
    msg = json.loads(line)
    ident = msg.get("id")
    method = msg.get("method")
    params = msg.get("params") or {}
    if method == "initialize":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"ok":True}}), flush=True)
    elif method == "session.prompt":
        sid = params.get("sessionId","")
        print(json.dumps({"jsonrpc":"2.0","method":"event","params":{"sessionId":sid,"event":{"type":"agent_finished","runId":"r1","usage":{"inputTokens":9,"outputTokens":3}}}}), flush=True)
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"lastEvent":"agent_finished"}}), flush=True)
    elif method == "session.new":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessionId":"s1","profile":params.get("profile"),"provider":params.get("provider"),"model":params.get("model")}}), flush=True)
    elif method == "provider.list":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"providers":[{"id":"mock"}]}}), flush=True)
    elif method == "model.list":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"models":[{"provider":"mock","model":"demo","displayName":"Demo"}]}}), flush=True)
    elif method == "agentProfile.list":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"profiles":REG.get("profiles",[])}}, ), flush=True)
    elif method == "agentProfile.register":
        profiles = REG.setdefault("profiles", [])
        profiles.append({"name": params.get("name"), "description": params.get("description"), "tools": params.get("tools", []), "skills": params.get("skills", [])})
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"name":params.get("name"),"registered":True}}), flush=True)
    elif method == "skill.register":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"name":params.get("name"),"registered":True}}), flush=True)
    elif method == "command.register":
        cmds = REG.setdefault("commands", [])
        cmds.append({"name": params.get("name"), "handler": params.get("handler")})
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"name":params.get("name"),"registered":True}}), flush=True)
    elif method == "command.dispatch":
        cmds = REG.get("commands", [])
        if any(c.get("name") == params.get("name") for c in cmds):
            print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"name":params.get("name"),"value":{"dispatched":True}}}), flush=True)
        else:
            print(json.dumps({"jsonrpc":"2.0","id":ident,"error":{"code":-32602,"message":"unknown command"}}), flush=True)
    elif method == "session.list":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessions":[]}}), flush=True)
    elif method == "session.load":
        first_text = "Hi"
        if params.get("entryId"):
            first_text = "opened:" + str(params.get("entryId"))
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessionId":params.get("sessionId"),"profile":"chat","metadata":{"provider":"mock","model":"demo"},"entries":[{"role":"user","content":{"text":first_text}},{"role":"assistant","content":{"text":"hello"}}]}}), flush=True)
    elif method == "knowledge.setOptions":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"workspaceRoot":params.get("workspaceRoot"),"wiki":params.get("wiki")}}), flush=True)
    elif method == "session.search":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"hits":[{"sessionId":"s1","leafId":"entry_9","updatedAt":"2026-09-03","label":"chat","snippet":"mock snippet"}],"nextCursor":""}}), flush=True)
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
        mcp_allow_list: Vec::new(),
    })
}

fn every_client_command() -> Vec<AgentClientCommand> {
    vec![
        AgentClientCommand::Prompt {
            session_id: "s1".into(),
            text: "hi".into(),
            provider: None,
            model: None,
            thinking_level: None,
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
            workspace_root: None,
            full_autonomy: None,
            om_observation: None,
            om_reflection: None,
        },
        AgentClientCommand::NewSession {
            profile: "coding".into(),
            provider: "mock".into(),
            model: "demo".into(),
            workspace_root: Some("/tmp/workspace".into()),
            full_autonomy: Some(true),
            om_observation: None,
            om_reflection: None,
        },
        AgentClientCommand::LoadSession {
            session_id: "s1".into(),
            entry_id: None,
        },
        // Plan 109 I7: context inspector list + item-detail fetches.
        AgentClientCommand::Context {
            session_id: "s1".into(),
            item_id: None,
        },
        AgentClientCommand::Context {
            session_id: "s1".into(),
            item_id: Some("clay-entry-0#1".into()),
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
        AgentClientCommand::Compact {
            session_id: "s1".into(),
            strategy: Some("default".into()),
        },
        AgentClientCommand::SetAutonomy {
            session_id: "s1".into(),
            enabled: false,
        },
        AgentClientCommand::SearchSessions {
            session_id: "s1".into(),
            query: Some("flake".into()),
            limit: Some(10),
        },
        AgentClientCommand::RunResume {
            session_id: "s1".into(),
            run_id: "r1".into(),
            decision_json: "{\"kind\":\"approve\"}".into(),
        },
        AgentClientCommand::SessionTree {
            session_id: "s1".into(),
            method: "checkout".into(),
            entry_id: "entry-7".into(),
        },
        AgentClientCommand::SkillRegister {
            name: "rust-review".into(),
            description: Some("Review Rust diffs".into()),
            instructions: Some("Check borrow errors".into()),
            tool_names: vec!["read".into(), "edit".into()],
        },
        AgentClientCommand::CommandRegister {
            name: "steer-like".into(),
            handler: Some("steer".into()),
            description: Some("Dispatch a steer".into()),
        },
        AgentClientCommand::CommandDispatch {
            name: "steer-like".into(),
            session_id: Some("s1".into()),
            args_json: Some("{\"text\":\"go left\"}".into()),
        },
        AgentClientCommand::ApprovalResolve {
            request_id: "approval-1".into(),
            allowed: true,
        },
        AgentClientCommand::AskDecisionResolve {
            request_id: "approval-2".into(),
            answer_json: "{\"selectedId\":\"opt-a\"}".into(),
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
            entries: vec![clay::protocol::AgentTranscriptEntry::new(
                clay::protocol::AgentTranscriptKind::User,
                "hi",
            )],
            mcp_servers: Vec::new(),
            context_tokens: None,
            effort_levels: Vec::new(),
            effort: None,
            commands: Vec::new(),
            branch: String::new(),
            extensions: Vec::new(),
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
                args_digest: None,
                output_digest: None,
                skill_name: None,
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
            provider: String::new(),
            model: String::new(),
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
        AgentServerMessage::AgentRpc {
            code: "agent.search_result".into(),
            result_json: "{\"hits\":[]}".into(),
        },
        AgentServerMessage::ApprovalRequest {
            request_id: "approval-1".into(),
            kind: clay::protocol::ApprovalRequestKind::Mutation,
            payload_json: "{\"kind\":\"write\",\"paths\":[\"/tmp/x\"]}".into(),
        },
        AgentServerMessage::ApprovalRequest {
            request_id: "approval-2".into(),
            kind: clay::protocol::ApprovalRequestKind::AskDecision,
            payload_json: "{\"question\":\"Which?\",\"options\":[]}".into(),
        },
        AgentServerMessage::Diagnostic {
            code: "agent.node_missing".into(),
            message: "Node >= 20 is required".into(),
        },
    ]
}

#[test]
fn phase25_protocol_version_is_pinned() {
    assert_eq!(PROTOCOL_VERSION, 30);
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
    // The boundary is the PACKAGE (third-party) op set and facade allowlist,
    // not file-level name absence: trusted configuration/first-party packages
    // legitimately reach the daemon through the Phase 1 `clay:agent` facade.
    let ops_src = read_src("src/server/ops/mod.rs");
    let package_extension = ops_src
        .split("clay_runtime_package_extension")
        .nth(1)
        .expect("package extension must exist");
    assert_absent(
        package_extension,
        &["op_clay_agent"],
        "package ops must not talk to the clay-agent pipe",
    );
    let facades_src = read_src("src/server/facades.rs");
    assert!(
        facades_src.contains("clay:agent"),
        "clay:agent facade must be registered for the trusted runtime"
    );
    assert!(
        !facades_src.contains("Facade::public(\"clay:agent\")"),
        "clay:agent facade must stay trusted-only (no third-party daemon access)"
    );
    let trusted_src = read_src("src/server/ops/agent.rs");
    assert!(
        non_test(&trusted_src).contains("AgentHostHandle::global()"),
        "agent ops must fail closed when no agent host is wired"
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
        mcp_allow_list: Vec::new(),
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
            workspace_root: None,
            full_autonomy: None,
            om_observation: None,
            om_reflection: None,
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
            provider: None,
            model: None,
            thinking_level: None,
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
    // Structured usage (plan 108 task 9): the daemon's agent_finished usage
    // rides Finished as bounded counters, not just display text.
    let AgentServerMessage::Event {
        event: AgentWireEvent::Finished { usage, .. },
        ..
    } = event.as_ref()
    else {
        panic!("expected Finished event, got {:?}", event.as_ref());
    };
    assert_eq!(usage, "9 in / 3 out");

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
    let loaded = host.resume_tab(3, "s1", None).await;
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
        provider: None,
        model: None,
        thinking_level: None,
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
    // ACP/AG-UI and retired 0.3 names stay denied everywhere. MCP is a
    // package-declared bridge allowed only in the clay-agent JS graph, so the
    // MCP needles are Cargo.toml-only denies (Phase 1).
    for needle in [
        "prism-acp",
        "prism-ag-ui",
        "agentclientprotocol",
        "prism-coding-agent",
        "@arnilo/prism-coding-agent",
        // 0.5: office + antigravity stay out of the daemon graph; exact
        // needles catch a future accidental adoption.
        "@arnilo/prism-office",
        "prism-antigravity-agent",
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
    for needle in ["@modelcontextprotocol", "@arnilo/prism-mcp"] {
        assert!(
            !cargo.contains(needle),
            "Cargo.toml must not depend on {needle}"
        );
    }
    // MCP SDK v2 (2026-07-28) lives transitively inside @arnilo/prism-mcp;
    // clay-agent must never depend on the monolithic SDK directly.
    for needle in [
        "\"@modelcontextprotocol/sdk\"",
        "\"@modelcontextprotocol/client\"",
        "\"@modelcontextprotocol/server\"",
    ] {
        assert!(
            !agent_pkg.contains(needle),
            "clay-agent/package.json must not depend directly on {needle}"
        );
    }
    assert!(agent_readme.contains("0.5.3"));
    assert!(agent_readme.contains("Upgrade Prism"));
    assert!(agent_readme.contains("no tools and no sandbox"));
    assert!(chat_docs.contains("no tools, no sandbox"));
    // Phase 0 + Phase 1 (Prism 0.5.x, live pins 0.5.3): exact family pins,
    // no retired 0.3 package names.
    for pin in [
        "\"@arnilo/prism\": \"0.5.3\"",
        "\"@arnilo/prism-core\": \"0.5.3\"",
        "\"@arnilo/prism-providers\": \"0.5.3\"",
        "\"@arnilo/prism-coding-tools\": \"0.5.3\"",
        "\"@arnilo/prism-web-tools\": \"0.5.3\"",
        "\"@arnilo/prism-memory\": \"0.5.3\"",
        "\"@arnilo/prism-mcp\": \"0.5.3\"",
        "\"better-sqlite3\": \"13.0.3\"",
    ] {
        assert!(
            agent_pkg.contains(pin),
            "clay-agent/package.json must pin exactly {pin}"
        );
    }
    // Prism 0.5.1 kernel construction (decision 2026-09-07-2149): the host
    // session-cache stopgap from decision 1325 must not return — the kernel
    // fills options.sessionId/cacheKey and default cache breakpoints.
    assert!(
        !read_src("clay-agent/src/host.ts").contains("createSessionCachePolicy"),
        "clay-agent/src/host.ts must not register createSessionCachePolicy"
    );
    let agent_src = [
        "clay-agent/src/host.ts",
        "clay-agent/src/providers.ts",
        "clay-agent/src/main.ts",
        "clay-agent/src/rpc.ts",
        "clay-agent/src/redact.ts",
    ]
    .map(read_src)
    .join("\n");
    for needle in [
        "@arnilo/prism-credentials-node",
        "@arnilo/prism-session-store-sqlite",
        "@arnilo/prism-session-store-codecs",
        "@arnilo/prism-tool-validator-json-schema",
        "@arnilo/prism-model-router",
        "\"@arnilo/prism-provider-",
    ] {
        assert!(
            !agent_pkg.contains(needle),
            "clay-agent/package.json must not use retired Prism 0.3 name {needle}"
        );
        assert!(
            !agent_src.contains(needle),
            "clay-agent sources must not import retired Prism 0.3 name {needle}"
        );
    }
    // Family subpaths must be the only @arnilo/prism* import shape.
    assert!(agent_src.contains("@arnilo/prism-core/credentials/node"));
    assert!(agent_src.contains("@arnilo/prism-core/sessions/sqlite"));
    assert!(agent_src.contains("@arnilo/prism-core/validation/json-schema"));
    assert!(agent_src.contains("@arnilo/prism-providers/openai"));
    // 0.5 adds hyper/commandcode to the explicit first-party loads; office
    // and antigravity stay out of clay-agent sources too.
    assert!(agent_src.contains("@arnilo/prism-providers/hyper"));
    assert!(agent_src.contains("@arnilo/prism-providers/commandcode"));
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
        mcp_allow_list: Vec::new(),
    });
    let started = Instant::now();
    let _ = host.run(AgentClientCommand::ListSessions).await;
    assert!(started.elapsed() < Duration::from_millis(AGENT_DAEMON_SPAWN_P95_BUDGET_MS));
    let meta = fs::metadata(data_dir.join("vault.passphrase")).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    host.shutdown().await;
}

#[tokio::test]
async fn reverse_rpc_document_request_round_trips() {
    let dir = temp_dir("reverse");
    let script = dir.join("reverse-agent");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    ident = msg.get("id")
    method = msg.get("method")
    if method == "initialize":
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"ok":True}}), flush=True)
    elif method == "session.new":
        # Daemon-initiated reverse request: document.read.
        print(json.dumps({"jsonrpc":"2.0","id":9001,"method":"document.read","params":{"path":"/tmp/ws/x.txt"}}), flush=True)
        reply = json.loads(sys.stdin.readline())
        text = (reply.get("result") or {}).get("text", "ERR")
        print(json.dumps({"jsonrpc":"2.0","id":ident,"result":{"sessionId":text,"profile":"p","provider":"pv","model":"m"}}), flush=True)
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let host = host_for(script);
    host.set_reverse_handler(Arc::new(|method: String, _params: Value| {
        Box::pin(async move {
            assert_eq!(method, "document.read");
            Ok(json!({ "text": "agent-doc-text", "version": 1, "dirty": false, "open": true }))
        })
    }));
    let snapshot = host
        .run(AgentClientCommand::NewSession {
            profile: "coding".into(),
            provider: "mock".into(),
            model: "demo".into(),
            workspace_root: None,
            full_autonomy: None,
            om_observation: None,
            om_reflection: None,
        })
        .await;
    match snapshot {
        AgentServerMessage::Snapshot(snapshot) => {
            assert_eq!(snapshot.session_id, "agent-doc-text");
        }
        other => panic!("expected snapshot, got {other:?}"),
    }
    host.shutdown().await;
}

#[cfg(unix)]
#[tokio::test]
async fn registration_rpc_queues_without_spawning_then_drains_after_initialize() {
    let host = host_for(mock_daemon());
    // Package load entries queue while the daemon is down: no spawn, no boot
    // wait, and the load returns immediately.
    let queued_skill = host
        .rpc_or_queue(
            "skill.register",
            json!({ "name": "coding-agent.createPlan" }),
        )
        .await
        .expect("queue accepts while daemon is down");
    assert_eq!(queued_skill, json!({ "queued": true }));
    let queued_profile = host
        .rpc_or_queue(
            "agentProfile.register",
            json!({
                "name": "coding",
                "description": "Workspace coding agent",
                "tools": ["read", "edit"],
                "skills": ["coding-agent.createPlan"],
            }),
        )
        .await
        .expect("queue accepts while daemon is down");
    assert_eq!(queued_profile, json!({ "queued": true }));
    assert_eq!(host.pending_registration_len().await, 2);

    // The next command spawns the daemon; queued registrations apply right
    // after the initialize handshake and before the command itself.
    let created = host
        .run(AgentClientCommand::NewSession {
            profile: "coding".into(),
            provider: "mock".into(),
            model: "demo".into(),
            workspace_root: None,
            full_autonomy: None,
            om_observation: None,
            om_reflection: None,
        })
        .await;
    assert!(matches!(created, AgentServerMessage::Snapshot(_)));
    assert_eq!(
        host.pending_registration_len().await,
        0,
        "queue must drain after the initialize handshake"
    );

    // The registrations reached the daemon in order: the profile list now
    // contains the queued coding profile.
    let listing = host
        .rpc("agentProfile.list", json!({}))
        .await
        .expect("list");
    let profiles = listing
        .get("profiles")
        .and_then(Value::as_array)
        .expect("profiles array")
        .iter()
        .filter_map(|profile| profile.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(
        profiles,
        vec!["coding"],
        "queued registration must reach the daemon exactly once"
    );

    // With the daemon running, registration executes immediately over the
    // live rpc path (no queue detour: the answer is the daemon's own).
    let live = host
        .rpc_or_queue(
            "agentProfile.register",
            json!({ "name": "coding-again", "description": "again" }),
        )
        .await
        .expect("live registration reaches the daemon");
    assert_eq!(
        live,
        json!({ "name": "coding-again", "registered": true }),
        "live path must not report queued"
    );
    assert_eq!(host.pending_registration_len().await, 0);
}

/// Phase 2 command surface (plan 108 task 7): command registrations queue
/// like other inert declarations while the daemon is down, then apply after
/// initialize; a live dispatch reaches the daemon's command registry and an
/// unknown command fails closed with the daemon's typed error.
#[tokio::test]
async fn command_registration_queues_then_live_dispatch_reaches_daemon() {
    let host = host_for(mock_daemon());
    let queued = host
        .rpc_or_queue(
            "command.register",
            json!({ "name": "/compact", "handler": "compact", "description": "Compact this session." }),
        )
        .await
        .expect("queue accepts while daemon is down");
    assert_eq!(queued, json!({ "queued": true }));
    assert_eq!(host.pending_registration_len().await, 1);

    // The next command spawns the daemon and drains the queued registration.
    let created = host
        .run(AgentClientCommand::NewSession {
            profile: "chat".into(),
            provider: "mock".into(),
            model: "demo".into(),
            workspace_root: None,
            full_autonomy: None,
            om_observation: None,
            om_reflection: None,
        })
        .await;
    assert!(matches!(created, AgentServerMessage::Snapshot(_)));
    assert_eq!(host.pending_registration_len().await, 0);

    // The registered command is dispatchable on the daemon.
    let listed = host
        .rpc(
            "command.dispatch",
            json!({ "name": "/compact", "args": {} }),
        )
        .await
        .expect("dispatch reaches the daemon");
    assert!(
        listed.is_object() || listed.is_null(),
        "dispatch returns the handler result envelope"
    );

    // Unknown command: bounded typed failure, not a hang.
    let unknown = host
        .rpc("command.dispatch", json!({ "name": "/nope", "args": {} }))
        .await;
    assert!(
        unknown.is_err(),
        "unknown commands fail closed at the daemon"
    );
    assert_eq!(host.pending_registration_len().await, 0);
}

#[cfg(unix)]
#[tokio::test]
async fn session_search_picker_hits_parse_and_selection_opens_at_entry() {
    // Plan 108 task 11 (decision 2201): the picker's search method parses
    // the Phase 1 session.search hits verbatim, and activating a result
    // resumes the tab with the transcript opened at the matching entry
    // (read-only view — no tree mutation, no implicit context attach).
    let host = host_for(mock_daemon());
    // Bind the tab to a session first: the search scope is the tab's
    // current session workspace (decision 2201).
    host.resume_tab(3, "s1", None).await;
    let hits = host
        .search_sessions(3, "needle", 50)
        .await
        .expect("search parses");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].session_id, "s1");
    assert_eq!(hits[0].leaf_id.as_deref(), Some("entry_9"));
    assert_eq!(hits[0].label, "chat");
    assert_eq!(hits[0].snippet, "mock snippet");

    // Selection: resume_tab carries the entry id through session.load.
    let loaded = host.resume_tab(3, "s1", Some("entry_9")).await;
    match loaded {
        AgentServerMessage::Snapshot(snapshot) => {
            assert!(
                snapshot.entries[0].text.contains("opened:entry_9"),
                "the entry id reaches the daemon load"
            );
            assert_eq!(snapshot.entries.len(), 2);
        }
        other => panic!("expected snapshot, got {other:?}"),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn knowledge_set_options_forwards_to_daemon() {
    // Plan 108 task 12: the opt-in wiki configuration reaches the daemon's
    // knowledge.setOptions; the host passes the workspace root through
    // verbatim (decision 2156 keeps activation daemon-side and opt-in).
    let host = host_for(mock_daemon());
    let result = host
        .rpc(
            "knowledge.setOptions",
            json!({ "workspaceRoot": "/tmp/workspace", "wiki": true }),
        )
        .await
        .expect("knowledge options forward");
    assert_eq!(
        result.get("workspaceRoot").and_then(Value::as_str),
        Some("/tmp/workspace")
    );
    assert_eq!(result.get("wiki").and_then(Value::as_bool), Some(true));
}
