//! Plan 119 SC-6 verification: one agent session per workspace, two workspaces
//! live at once.
//!
//! These tests drive a **real `clay server` process** over its real IPC socket,
//! with two real clients bound to two different workspace roots, and assert the
//! invariant the SC-6 remediation is about: an agent's tool calls and session
//! state resolve to the *session's* workspace root, never to another root and
//! never to the server's bootstrap root.
//!
//! Two layers, deliberately split by what each can prove:
//!
//! 1. [`two_workspaces_keep_their_agent_writes_in_their_own_root`] — a scripted
//!    daemon (`CLAY_AGENT_MAIN`) blindly attempts the same `document.write`
//!    against *both* roots for every prompt. The server's containment check is
//!    the only thing standing between the two workspaces, so the accepted write
//!    names which root the session resolved to. Also covers the fail-closed
//!    case (a session whose tab closed) and session identity across a daemon
//!    restart.
//! 2. [`real_daemon_serves_one_session_per_workspace`] — the **shipped
//!    clay-agent daemon** in `--mock` mode (no model call needed) reports each
//!    session's own workspace listing (`workspace.files` walks
//!    `live.workspaceRoot`), which proves the daemon-side half: the resumed
//!    session reads the root recorded in its metadata, not the daemon's launch
//!    directory.
//!
//! Evidence for the plan task is:
//! `cargo test --test security -- agent_session_isolation` (the `security`
//! suite aggregates this file; there is no standalone test target).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use clay::client::{ClientConnectionEvent, ClientSession, connect_with_workspace_root};
use clay::ipc::IpcEndpoint;
use clay::protocol::{
    AgentClientCommand, AgentPickerKind, AgentSecret, AgentServerMessage, ClientMessage,
    SduiActionArgument, SduiActionIntent, SduiActionSource, SduiActionValue, SduiNodeId,
    TabCommand,
};

type TabId = u64;

const EVENT_TIMEOUT: Duration = Duration::from_secs(30);

/// The generated daemon script. `{root_a}`/`{root_b}`/`{log}` are baked in so
/// the script needs no knowledge of which session owns which root: it tries
/// both and lets the server's containment check decide.
const SCRIPTED_DAEMON: &str = r#"#!/usr/bin/env python3
import json, os, sys

ROOT_A = {root_a}
ROOT_B = {root_b}
LOG = {log}
PROBE = {probe}
OUT = "out.txt"

def note(line):
    with open(LOG, "a") as handle:
        handle.write(line + "\n")

def send(payload):
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()

next_id = 9000

def reverse(method, params):
    global next_id
    next_id += 1
    send({"jsonrpc": "2.0", "id": next_id, "method": method, "params": params})
    return json.loads(sys.stdin.readline())

def attempt(kind, session_id, root):
    content = session_id if kind == "write" else "probe:" + session_id
    reply = reverse("document.write", {"path": os.path.join(root, OUT), "content": content, "sessionId": session_id})
    error = (reply.get("error") or {}).get("message", "")
    note("{} {} {} {}".format(kind, session_id, "ok" if "result" in reply else "err", error))

sessions = []
counter = 0

for line in sys.stdin:
    message = json.loads(line)
    ident = message.get("id")
    method = message.get("method")
    params = message.get("params") or {}
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": ident, "result": {"ok": True}})
    elif method == "provider.list":
        send({"jsonrpc": "2.0", "id": ident, "result": {"providers": [
            {"id": "mock", "configured": True, "auth": [{"kind": "api_key", "name": "API key"}]}]}})
    elif method == "model.list":
        send({"jsonrpc": "2.0", "id": ident, "result": {"models": [
            {"provider": "mock", "model": "demo", "displayName": "Demo"}]}})
    elif method == "agentProfile.list":
        send({"jsonrpc": "2.0", "id": ident, "result": {"profiles": []}})
    elif method == "session.list":
        send({"jsonrpc": "2.0", "id": ident, "result": {"sessions": []}})
    elif method == "session.new":
        counter += 1
        session_id = "mock-session-%d" % counter
        send({"jsonrpc": "2.0", "id": ident, "result": {
            "sessionId": session_id,
            "profile": params.get("profile"),
            "provider": params.get("provider"),
            "model": params.get("model")}})
    elif method == "session.prompt":
        session_id = params.get("sessionId", "")
        if session_id not in sessions:
            sessions.append(session_id)
        # The same write against both roots: only the session's own root can
        # accept it. Earlier sessions whose tab has since closed are probed too,
        # which must fail closed.
        for root in (ROOT_A, ROOT_B):
            attempt("write", session_id, root)
        # The test drops the sentinel once a tab has been closed; the earlier
        # sessions are then stale and their writes must fail closed.
        if os.path.exists(PROBE):
            for stale in sessions[:-1]:
                attempt("probe", stale, ROOT_A)
        send({"jsonrpc": "2.0", "method": "event", "params": {"sessionId": session_id,
            "event": {"type": "agent_finished", "runId": "run-1",
                      "usage": {"inputTokens": 1, "outputTokens": 1}}}})
        send({"jsonrpc": "2.0", "id": ident, "result": {"lastEvent": "agent_finished"}})
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": ident, "result": {"ok": True}})
        break
    else:
        send({"jsonrpc": "2.0", "id": ident, "result": {}})
"#;

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("clay-sc6-{label}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn dir(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Keeps the spawned server (and any daemon it spawned) from outliving a test.
struct ServerProcess {
    child: Child,
    endpoint: IpcEndpoint,
    log: PathBuf,
}

impl ServerProcess {
    fn spawn(scratch: &Scratch, env: &[(&str, String)]) -> Self {
        let endpoint = clay::ipc::smoke_endpoint("sc6");
        let socket = endpoint.as_child_arg();
        let log = scratch.path("server.log");
        let mut command = Command::new(env!("CARGO_BIN_EXE_clay"));
        command
            .arg("server")
            .arg(&socket)
            .current_dir(scratch.dir("launch"))
            // Isolated profile: agent config, sessions, and the daemon's data
            // all land under the scratch HOME, never the developer's.
            .env("HOME", scratch.dir("home"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                fs::File::create(&log).expect("server log file"),
            ));
        for (name, value) in env {
            command.env(name, value);
        }
        let child = command.spawn().expect("spawn clay server");
        Self {
            child,
            endpoint,
            log,
        }
    }

    /// The server's own diagnostics (`[agent] ...` lines), for failure messages.
    fn log_tail(&self) -> String {
        let text = fs::read_to_string(&self.log).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        lines[lines.len().saturating_sub(12)..].join("\n")
    }

    /// PIDs of the processes the server spawned (its agent daemon). Children
    /// are tracked per *task*, so every thread's list is unioned.
    fn daemon_pids(&self) -> Vec<u32> {
        let pid = self.child.id();
        let mut pids: Vec<u32> = Vec::new();
        let tasks = fs::read_dir(format!("/proc/{pid}/task"))
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for task in tasks {
            let children =
                fs::read_to_string(format!("/proc/{pid}/task/{task}/children")).unwrap_or_default();
            for child in children
                .split_whitespace()
                .filter_map(|pid| pid.parse().ok())
            {
                if !pids.contains(&child) {
                    pids.push(child);
                }
            }
        }
        pids
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn wait_for_server(endpoint: &IpcEndpoint) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if clay::client::probe_protocol(endpoint).await == clay::client::ProtocolProbe::Compatible {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "clay server did not answer a handshake in time"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// The `session.bound` payload this connection was told about.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Bound {
    client_id: u64,
    tab_id: u64,
    session_id: String,
}

fn bound_of(message: &AgentServerMessage) -> Option<Bound> {
    let AgentServerMessage::AgentRpc { code, result_json } = message else {
        return None;
    };
    if code != "session.bound" {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(result_json).ok()?;
    Some(Bound {
        client_id: value.get("clientId")?.as_u64()?,
        tab_id: value.get("tabId")?.as_u64()?,
        session_id: value.get("sessionId")?.as_str()?.to_string(),
    })
}

/// Drain events until `predicate` holds, or panic with what did arrive.
async fn await_event<T>(
    client: &mut ClientSession,
    label: &str,
    mut predicate: impl FnMut(&ClientConnectionEvent, &mut Option<T>) -> bool,
) -> T {
    let mut found = None;
    let deadline = Instant::now() + EVENT_TIMEOUT;
    while found.is_none() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = tokio::time::timeout(remaining, client.events.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {label}"))
            .unwrap_or_else(|| panic!("connection closed before {label}"));
        predicate(&event, &mut found);
    }
    found.unwrap()
}

fn agent_message(event: &ClientConnectionEvent) -> Option<&AgentServerMessage> {
    match event {
        ClientConnectionEvent::Agent(message) => Some(message),
        _ => None,
    }
}

/// Registers a tab for `root` on this connection (the folder-open path).
async fn open_tab(endpoint: &IpcEndpoint, root: &Path, label: &str) -> ClientSession {
    let client = connect_with_workspace_root(endpoint, root.to_string_lossy().into_owned())
        .await
        .unwrap_or_else(|error| panic!("{label} connect: {error}"));
    assert_eq!(
        client.initial_state.workspace_root,
        root.to_string_lossy(),
        "{label} bound to its own root"
    );
    client
}

/// The tab id the server registered for this connection's root.
async fn tab_id(client: &mut ClientSession, root: &Path, client_id: u64) -> TabId {
    let root = root.to_string_lossy().into_owned();
    await_event(client, "tab registry snapshot", |event, found| {
        if let ClientConnectionEvent::TabRegistry(snapshot) = event
            && let Some(tab) = snapshot
                .tabs
                .iter()
                .find(|tab| tab.workspace_root == root && tab.client_id == client_id)
        {
            *found = Some(tab.tab_id);
        }
        found.is_some()
    })
    .await
}

fn select_provider_and_model(client: &ClientSession, provider: &str, model: &str) {
    for command in [
        AgentClientCommand::Select {
            kind: AgentPickerKind::Provider,
            id: provider.to_string(),
        },
        AgentClientCommand::Select {
            kind: AgentPickerKind::Model,
            id: model.to_string(),
        },
    ] {
        client
            .edit_queue
            .enqueue_raw(ClientMessage::Agent {
                client_id: client.initial_state.client_id,
                command: Box::new(command),
            })
            .expect("enqueue agent command");
    }
}

/// The composer's submit path (`agent.submit` with the prompt text argument).
fn submit_prompt(client: &ClientSession, text: &str) {
    let mut intent = SduiActionIntent::command(
        "agent.submit",
        SduiActionSource::Button {
            node_id: SduiNodeId(1),
        },
    );
    intent.arguments.push(SduiActionArgument {
        name: "value".to_string(),
        value: SduiActionValue::String(text.to_string()),
    });
    client
        .edit_queue
        .enqueue_sdui_action(0, intent)
        .expect("enqueue agent.submit");
}

/// The panel's mount: the tab's own answer names the session it owns.
fn ask_tab_state(client: &ClientSession) {
    client
        .edit_queue
        .enqueue_raw(ClientMessage::Agent {
            client_id: client.initial_state.client_id,
            command: Box::new(AgentClientCommand::TabState),
        })
        .expect("enqueue tab state");
}

fn workspace_files(client: &ClientSession, session_id: &str) {
    client
        .edit_queue
        .enqueue_raw(ClientMessage::Agent {
            client_id: client.initial_state.client_id,
            command: Box::new(AgentClientCommand::WorkspaceFiles {
                session_id: session_id.to_string(),
            }),
        })
        .expect("enqueue workspace files");
}

fn close_tab(client: &ClientSession, tab: TabId) {
    client
        .edit_queue
        .enqueue_tab_command(TabCommand::Close { tab_id: tab })
        .expect("enqueue tab close");
}

async fn read_file(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

/// Polls the filesystem: the writes are the server's own, arriving
/// asynchronously behind the daemon's reverse RPC.
async fn await_file(path: &Path) -> String {
    let deadline = Instant::now() + EVENT_TIMEOUT;
    loop {
        if let Some(text) = read_file(path).await {
            return text;
        }
        assert!(
            Instant::now() < deadline,
            "{} was never written",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(250)).await;
}

fn python3() -> String {
    for candidate in ["/usr/bin/python3", "/usr/local/bin/python3"] {
        if Path::new(candidate).is_file() {
            return candidate.to_string();
        }
    }
    "python3".to_string()
}

/// An absolute `node`, resolved the way the server resolves it: `PATH`
/// first, then the usual install locations (the test host keeps node under
/// nvm, and the spawned server does not necessarily inherit PATH).
fn node() -> String {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join("node");
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let nvm = Path::new(&home).join(".nvm/versions/node");
    if let Ok(entries) = fs::read_dir(&nvm) {
        for entry in entries.filter_map(|entry| entry.ok()) {
            let candidate = entry.path().join("bin/node");
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    "node".to_string()
}

fn clay_agent_main() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("clay-agent/dist/main.js")
}

#[tokio::test]
async fn two_workspaces_keep_their_agent_writes_in_their_own_root() {
    let scratch = Scratch::new("scripted");
    let root_a = scratch.dir("workspace-a");
    let root_b = scratch.dir("workspace-b");
    let log = scratch.path("writes.log");
    let probe = scratch.path("probe-stale");
    let script = scratch.write(
        "scripted-daemon.py",
        &SCRIPTED_DAEMON
            .replace("{root_a}", &format!("{:?}", root_a.to_string_lossy()))
            .replace("{root_b}", &format!("{:?}", root_b.to_string_lossy()))
            .replace("{log}", &format!("{:?}", log.to_string_lossy()))
            .replace("{probe}", &format!("{:?}", probe.to_string_lossy())),
    );
    let server = ServerProcess::spawn(
        &scratch,
        &[
            ("CLAY_NODE", python3()),
            ("CLAY_AGENT_MAIN", script.to_string_lossy().into_owned()),
        ],
    );
    wait_for_server(&server.endpoint).await;

    let mut client_a = open_tab(&server.endpoint, &root_a, "client A").await;
    let mut client_b = open_tab(&server.endpoint, &root_b, "client B").await;
    let client_a_id = client_a.initial_state.client_id;
    let client_b_id = client_b.initial_state.client_id;
    let tab_a = tab_id(&mut client_a, &root_a, client_a_id).await;
    let tab_b = tab_id(&mut client_b, &root_b, client_b_id).await;
    assert_ne!(tab_a, tab_b, "each workspace has its own tab");

    // Both tabs are configured for the same provider/model; the session is what
    // has to differ (one per workspace root).
    select_provider_and_model(&client_a, "provider:mock", "model:mock/demo");
    select_provider_and_model(&client_b, "provider:mock", "model:mock/demo");
    ask_tab_state(&client_a);
    ask_tab_state(&client_b);

    let bound_a = await_event(&mut client_a, "session bound for tab A", |event, found| {
        if let Some(bound) = agent_message(event).and_then(bound_of)
            && bound.tab_id == tab_a
            && !bound.session_id.is_empty()
        {
            *found = Some(bound);
        }
        found.is_some()
    })
    .await;
    let bound_b = await_event(&mut client_b, "session bound for tab B", |event, found| {
        if let Some(bound) = agent_message(event).and_then(bound_of)
            && bound.tab_id == tab_b
            && !bound.session_id.is_empty()
        {
            *found = Some(bound);
        }
        found.is_some()
    })
    .await;
    assert_eq!(bound_a.client_id, client_a.initial_state.client_id);
    assert_eq!(bound_b.client_id, client_b.initial_state.client_id);
    assert_ne!(
        bound_a.session_id, bound_b.session_id,
        "one workspace root, one session"
    );

    // 1. The folder-A prompt writes into root A only.
    submit_prompt(&client_a, "write into my workspace");
    let written_a = await_file(&root_a.join("out.txt")).await;
    assert_eq!(
        written_a, bound_a.session_id,
        "the session's own root accepted the write"
    );
    settle().await;
    assert!(
        !root_b.join("out.txt").exists(),
        "the sibling workspace must not receive root A's write"
    );

    // 2. The folder-B prompt writes into root B only, with its own session.
    submit_prompt(&client_b, "write into my workspace");
    let written_b = await_file(&root_b.join("out.txt")).await;
    assert_eq!(written_b, bound_b.session_id);
    assert_eq!(
        read_file(&root_a.join("out.txt")).await.as_deref(),
        Some(bound_a.session_id.as_str()),
        "root A still holds root A's session content"
    );

    // 3. Closing tab A removes its authority: the daemon's next prompt probes
    // the stale session and the server must fail closed with a diagnostic
    // instead of falling back to a root.
    close_tab(&client_a, tab_a);
    settle().await;
    fs::write(&probe, b"probe").unwrap();
    submit_prompt(&client_b, "probe the closed workspace");
    let diagnostic = await_event(
        &mut client_b,
        "workspace-unresolved diagnostic",
        |event, found| {
            if let Some(AgentServerMessage::Diagnostic { code, .. }) = agent_message(event)
                && code == "agent.workspace_unresolved"
            {
                *found = Some(code.clone());
            }
            found.is_some()
        },
    )
    .await;
    assert_eq!(diagnostic, "agent.workspace_unresolved");
    assert_eq!(
        read_file(&root_a.join("out.txt")).await.as_deref(),
        Some(bound_a.session_id.as_str()),
        "the closed session's write never landed (daemon log: {})",
        fs::read_to_string(&log).unwrap_or_default()
    );
    eprintln!("server log:\n{}", server.log_tail());
    let logged = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        logged
            .lines()
            .any(|line| line.starts_with("probe ") && line.contains(" err ")),
        "the probe against the closed session was rejected: {logged}"
    );

    // 4. A daemon restart must not move the session to another root: the next
    // prompt reaches the same session (bound id unchanged) and writes again
    // into root B.
    let pids = server.daemon_pids();
    assert_eq!(pids.len(), 1, "the server spawned exactly one daemon");
    let killed = Command::new("kill")
        .arg(pids[0].to_string())
        .status()
        .unwrap();
    assert!(killed.success(), "kill the daemon");
    settle().await;
    let before = read_file(&root_b.join("out.txt")).await;
    fs::remove_file(root_b.join("out.txt")).unwrap();
    drop(before);
    submit_prompt(&client_b, "after the daemon restart");
    let after_restart = await_file(&root_b.join("out.txt")).await;
    assert_eq!(
        after_restart, bound_b.session_id,
        "the resumed session kept its own root"
    );
    assert_eq!(
        read_file(&root_a.join("out.txt")).await.as_deref(),
        Some(bound_a.session_id.as_str()),
        "the restart never redirects a session into root A"
    );
}

#[tokio::test]
async fn real_daemon_serves_one_session_per_workspace() {
    if !clay_agent_main().is_file() {
        eprintln!("skipping: clay-agent dist is not built");
        return;
    }
    let scratch = Scratch::new("live");
    let root_a = scratch.dir("workspace-a");
    let root_b = scratch.dir("workspace-b");
    scratch.write("workspace-a/only-a.md", "# only A\n");
    scratch.write("workspace-b/only-b.md", "# only B\n");

    let server = ServerProcess::spawn(
        &scratch,
        &[
            ("CLAY_NODE", node()),
            (
                "CLAY_AGENT_MAIN",
                clay_agent_main().to_string_lossy().into_owned(),
            ),
            // The shipped daemon's mock provider: real daemon, no model call.
            ("CLAY_AGENT_MOCK", "1".to_string()),
        ],
    );
    wait_for_server(&server.endpoint).await;

    let mut client_a = open_tab(&server.endpoint, &root_a, "client A").await;
    let mut client_b = open_tab(&server.endpoint, &root_b, "client B").await;
    let client_a_id = client_a.initial_state.client_id;
    let client_b_id = client_b.initial_state.client_id;
    let tab_a = tab_id(&mut client_a, &root_a, client_a_id).await;
    let tab_b = tab_id(&mut client_b, &root_b, client_b_id).await;

    // The mock provider is credential-gated like any other; the panel's own
    // flow stores a key before the first run.
    for client in [&client_a, &client_b] {
        client
            .edit_queue
            .enqueue_raw(ClientMessage::Agent {
                client_id: client.initial_state.client_id,
                command: Box::new(AgentClientCommand::CredentialPut {
                    provider: "mock".to_string(),
                    name: "apiKey".to_string(),
                    secret: AgentSecret("mock-secret".to_string()),
                }),
            })
            .unwrap();
        select_provider_and_model(client, "provider:mock", "model:mock/demo");
        ask_tab_state(client);
    }

    let bound_a = await_event(&mut client_a, "session bound for tab A", |event, found| {
        if let Some(bound) = agent_message(event).and_then(bound_of)
            && bound.tab_id == tab_a
            && !bound.session_id.is_empty()
        {
            *found = Some(bound);
        }
        found.is_some()
    })
    .await;
    let bound_b = await_event(&mut client_b, "session bound for tab B", |event, found| {
        if let Some(bound) = agent_message(event).and_then(bound_of)
            && bound.tab_id == tab_b
            && !bound.session_id.is_empty()
        {
            *found = Some(bound);
        }
        found.is_some()
    })
    .await;
    assert_ne!(bound_a.session_id, bound_b.session_id);

    // Each session sees its own root: `workspace.files` walks the daemon-side
    // `live.workspaceRoot`.
    let files_a = workspace_files_for(&mut client_a, &bound_a.session_id, "only-a.md").await;
    assert!(
        files_a.iter().any(|file| file == "only-a.md"),
        "session A lists root A: {files_a:?}"
    );
    assert!(
        !files_a.iter().any(|file| file == "only-b.md"),
        "session A never lists root B: {files_a:?}"
    );
    let files_b = workspace_files_for(&mut client_b, &bound_b.session_id, "only-b.md").await;
    assert!(
        files_b.iter().any(|file| file == "only-b.md")
            && !files_b.iter().any(|file| file == "only-a.md"),
        "session B lists root B only: {files_b:?}"
    );

    // A prompt streams from the real daemon for this tab's session.
    submit_prompt(&client_a, "hello from workspace A");
    let streamed = await_event(&mut client_a, "streamed run event", |event, found| {
        if let Some(AgentServerMessage::Event { session_id, .. }) = agent_message(event)
            && session_id == &bound_a.session_id
        {
            *found = Some(true);
        }
        found.is_some()
    })
    .await;
    assert!(streamed, "run events carry this tab's session id");

    // Restart the real daemon: the session must come back on the root recorded
    // in its metadata, not the daemon's launch directory (the SC-6 daemon fix).
    let pids = server.daemon_pids();
    assert_eq!(pids.len(), 1, "the server spawned exactly one daemon");
    let killed = Command::new("kill")
        .arg(pids[0].to_string())
        .status()
        .unwrap();
    assert!(killed.success(), "kill the daemon");
    settle().await;

    let files_a_after = workspace_files_for(&mut client_a, &bound_a.session_id, "only-a.md").await;
    assert!(
        files_a_after.iter().any(|file| file == "only-a.md")
            && !files_a_after.iter().any(|file| file == "only-b.md"),
        "the resumed session A kept root A: {files_a_after:?}"
    );
    let files_b_after = workspace_files_for(&mut client_b, &bound_b.session_id, "only-b.md").await;
    assert!(
        files_b_after.iter().any(|file| file == "only-b.md")
            && !files_b_after.iter().any(|file| file == "only-a.md"),
        "the resumed session B kept root B: {files_b_after:?}"
    );
}

/// Sends `workspace.files` for `session_id` and returns the listing that names
/// this root's own `marker`. The agent relay broadcasts every session's reply
/// to every connection and `workspace.files` carries no session id (a recorded
/// ceiling of the contract), so the reply is attributed by its content: if the
/// daemon resolved the wrong root, this never sees the marker and fails.
async fn workspace_files_for(
    client: &mut ClientSession,
    session_id: &str,
    marker: &str,
) -> Vec<String> {
    workspace_files(client, session_id);
    let label = format!("workspace.files reply naming {marker}");
    await_event(client, &label, |event, found| {
        if let Some(AgentServerMessage::AgentRpc { code, result_json }) = agent_message(event)
            && code == "workspace.files"
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(result_json)
            && let Some(files) = value.get("files").and_then(|files| files.as_array())
        {
            let files: Vec<String> = files
                .iter()
                .filter_map(|file| file.as_str().map(ToString::to_string))
                .collect();
            if files.iter().any(|file| file == marker) {
                *found = Some(files);
            }
        }
        found.is_some()
    })
    .await
}
