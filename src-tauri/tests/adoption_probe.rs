//! Plan 097 follow-up regression: the desktop supervisor must never adopt an
//! endpoint whose listener cannot serve the current protocol version, and
//! readiness probing must classify incompatible answers instead of timing
//! out. See `clay::client::probe_protocol` and `Supervisor::start`.

use std::time::Duration;

use clay::client::{ProtocolProbe, probe_protocol};
use clay::ipc::IpcEndpoint;
use clay::protocol::{
    ClientMessage, PROTOCOL_VERSION, ProtocolErrorCode, ServerMessage, codec::Codec,
};
use clay_desktop_lib::server::{ServerStatus, Supervisor};
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;

/// Serves exactly one scripted reply per incoming connection.
fn scripted_listener(reply: ServerMessage) -> (std::path::PathBuf, tokio::task::JoinHandle<()>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("probe.sock");
    let listener = UnixListener::bind(&path).expect("bind");
    let handle = tokio::spawn(async move {
        let _dir_guard = dir;
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let codec = Codec::default();
            // Read one framed client message, then answer with the script.
            if let Ok(message) = codec.read_client_message(&mut stream).await {
                assert!(matches!(message, ClientMessage::Hello { .. }));
            }
            let _ = codec.write_server_message(&mut stream, &reply).await;
            let _ = stream.shutdown().await;
        }
    });
    (path, handle)
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(future)
}

#[test]
fn probe_classifies_welcome_error_and_silence() {
    block_on(async {
        // Nothing listens on a fresh socket path.
        let missing = tempfile::tempdir().unwrap();
        assert_eq!(
            probe_protocol(&IpcEndpoint::UnixSocket(missing.path().join("none.sock"))).await,
            ProtocolProbe::NotListening
        );

        // A current-version server answers Welcome and is adoptable.
        let (path, server) = scripted_listener(ServerMessage::Welcome {
            client_id: 1,
            protocol_version: PROTOCOL_VERSION,
        });
        assert_eq!(
            probe_protocol(&IpcEndpoint::UnixSocket(path.clone())).await,
            ProtocolProbe::Compatible
        );
        server.abort();

        // A stale/foreign listener refuses the handshake.
        let (path, server) = scripted_listener(ServerMessage::Error {
            code: ProtocolErrorCode::UnsupportedProtocolVersion,
            message: "unsupported protocol version".into(),
        });
        assert_eq!(
            probe_protocol(&IpcEndpoint::UnixSocket(path)).await,
            ProtocolProbe::Incompatible
        );
        server.abort();
    });
}

#[test]
fn supervisor_refuses_to_adopt_incompatible_listener() {
    // Listener runs on the shared async runtime; Supervisor::start() must run
    // outside any ambient runtime because it drives its probe via
    // `tauri::async_runtime::block_on`.
    let (path, server) = tauri::async_runtime::block_on(async {
        scripted_listener(ServerMessage::Error {
            code: ProtocolErrorCode::UnsupportedProtocolVersion,
            message: "unsupported protocol version".into(),
        })
    });
    let endpoint = IpcEndpoint::UnixSocket(path);
    // A deliberately broken spawn target proves start() did NOT attempt to
    // spawn past the occupied endpoint: the typed refusal wins.
    let supervisor = Supervisor::with_test_config(
        endpoint,
        Some(std::path::PathBuf::from("/no-such-clay-server")),
        Duration::from_millis(10),
        Duration::from_millis(100),
    );
    let supervisor = std::sync::Arc::new(supervisor);
    supervisor.start();
    match supervisor.status() {
        ServerStatus::Disconnected { reason } => {
            assert!(reason.contains("incompatible"), "{reason}");
        }
        other => panic!("expected Disconnected, got {other:?}"),
    }
    // No spawn happened: a tracked child would surface Connecting/Connected.
    server.abort();
}
