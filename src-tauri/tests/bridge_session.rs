//! Plan 097 Phase 3 integration: the typed bridge session against a real
//! `clay-server` process. Covers bootstrap completeness, event delivery,
//! disconnect notices, reconnect-with-fresh-generation (stale stream data is
//! structurally rejected by pump teardown), and the typed request path.
//!
//! Skips when `target/<profile>/clay-server` has not been built.

use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clay::client::ClientConnectionEvent;
use clay::ipc::IpcEndpoint;
use clay_desktop_lib::bridge::forwarder::EventSink;
use clay_desktop_lib::bridge::{BridgeEnvelope, BridgeState};

#[derive(Default)]
struct Collector(Arc<Mutex<Vec<BridgeEnvelope>>>);

impl EventSink for Collector {
    fn deliver(&self, envelope: BridgeEnvelope) -> Result<(), String> {
        self.0.lock().expect("collector").push(envelope);
        Ok(())
    }
}

struct ServerProcess(Child);

impl ServerProcess {
    fn spawn(socket: &std::path::Path) -> Self {
        let exe = server_binary();
        let child = Command::new(exe)
            .arg(socket)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn clay-server");
        Self(child)
    }

    fn kill(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.kill();
    }
}

fn server_binary() -> std::path::PathBuf {
    let exe = std::env::current_exe().expect("current_exe");
    let candidate = exe
        .parent()
        .and_then(|dir| dir.parent())
        .map(|profile| profile.join("clay-server"))
        .filter(|candidate| candidate.is_file());
    candidate.expect("clay-server must be built before this test")
}

fn wait_for_listener(socket: &std::path::Path) {
    use std::os::unix::net::UnixStream;
    for _ in 0..200 {
        if UnixStream::connect(socket).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("clay-server never started listening on {socket:?}");
}

fn envelopes(collector: &Collector) -> Vec<BridgeEnvelope> {
    collector.0.lock().expect("collector").clone()
}

fn wait_for<F: Fn(&BridgeEnvelope) -> bool>(collector: &Collector, predicate: F) -> bool {
    for _ in 0..100 {
        if envelopes(collector).iter().any(&predicate) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bridge_session_bootstraps_notifies_disconnect_and_reconnects_cleanly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = dir.path().join("bridge.sock");
    let endpoint = IpcEndpoint::UnixSocket(socket.clone());

    let mut server = ServerProcess::spawn(&socket);
    wait_for_listener(&socket);

    let bridge = Arc::new(BridgeState::new(endpoint));
    let collector = Collector::default();
    bridge.subscribe(Collector(Arc::clone(&collector.0)));

    // --- bootstrap installs one complete state
    let first = bridge.bootstrap().await.expect("bootstrap");
    assert!(first.client_id > 0);
    assert_eq!(first.protocol_version, clay::protocol::PROTOCOL_VERSION);
    assert!(
        !first.behavior_manifest.commands.is_empty(),
        "bootstrap carries the behavior manifest"
    );
    assert!(first.initial_document.document_id > 0);
    // Theme snapshot arrives fully resolved: every core token present,
    // density scale derived, raw overrides absent from the wire shape.
    assert_eq!(first.active_theme.tokens.len(), 91);
    assert_eq!(first.active_theme.editor_styles.len(), 37);
    assert!(
        first.active_theme.editor_styles["keyword"]
            .color
            .starts_with('#')
    );
    assert!(matches!(
        first.active_theme.tokens.get("surface.main"),
        Some(clay::shell::theme::ThemeTokenValueDto::Color(hex)) if hex.starts_with('#')
    ));
    assert!((first.active_theme.density_scale - 1.0).abs() < f64::EPSILON);
    assert!(first.active_typography.hierarchy.body > 0.0);
    assert!(bridge.is_connected());
    assert_eq!(bridge.stats().generation, 1);
    assert!(
        first.endpoint.contains("bridge.sock"),
        "endpoint echoed: {}",
        first.endpoint
    );

    // --- handshake events flow through the subscription channel
    assert!(
        wait_for(&collector, |envelope| match envelope {
            BridgeEnvelope::Event(event) | BridgeEnvelope::Routed { event, .. } => {
                matches!(event.as_ref(), ClientConnectionEvent::TabRegistry(_))
            }
            _ => false,
        }),
        "tab registry event should arrive during/after bootstrap"
    );

    // --- typed request path: one optimistic edit through the bridge queue,
    // acknowledged by the server with our transaction id. (TabCommand::New is
    // handshake-owned: the connection binds its single tab during connect, so
    // an explicit New is rejected — by design.)
    let document_id = first.initial_document.document_id;
    bridge
        .request(&format!(
            r#"{{"family":"edit","payload":{{"clientId":0,"leaseId":null,"documentId":{document_id},"baseVersion":1,"behaviorVersion":2,"transactionId":77,"operation":{{"insert":{{"byteOffset":0,"text":"x"}}}}}}}}"#
        ))
        .expect("request accepted");
    assert!(
        wait_for(&collector, |envelope| match envelope {
            BridgeEnvelope::Event(event) | BridgeEnvelope::Routed { event, .. } => {
                matches!(
                    event.as_ref(),
                    ClientConnectionEvent::EditAck {
                        transaction_id: 77,
                        ..
                    }
                )
            }
            _ => false,
        }),
        "server should acknowledge the bridged edit"
    );

    // --- killing the server surfaces a Disconnected notice
    server.kill();
    assert!(
        wait_for(&collector, |envelope| matches!(
            envelope,
            BridgeEnvelope::Disconnected { .. }
        )),
        "disconnect notice should arrive after the server dies"
    );

    // --- reconnect on the same socket path: the new server removes the stale
    // socket file; the bridge reclaims-or-creates and installs generation 2.
    let restarted = ServerProcess::spawn(&socket);
    wait_for_listener(&socket);
    let second = bridge.reconnect().await.expect("reconnect");
    assert_eq!(second.generation, 2, "generation advances across sessions");
    assert_ne!(
        second.client_id, first.client_id,
        "a fresh server assigns a fresh identity"
    );
    assert!(!second.behavior_manifest.commands.is_empty());

    drop(restarted);
}
