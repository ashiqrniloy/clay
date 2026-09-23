//! Desktop-side Clay server supervision.
//!
//! The Tauri shell owns the *process* only: it launches the existing
//! `clay-server` binary against a Clay IPC endpoint and reports typed
//! connection status. All protocol authority stays inside the Clay server
//! (`clay` crate); this module never speaks the protocol itself.

use std::fmt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use clay::ipc::IpcEndpoint;
use serde::Serialize;

use crate::release::{classify_spawn_error, resolve_server_binary};

/// Typed connection status surfaced to the frontend over the bridge.
///
/// Serde shape (contract pinned by tests): `{ "state": "connecting" |
/// "connected" | "disconnected", ... }` with camelCase fields.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ServerStatus {
    Connecting {
        endpoint: String,
    },
    /// `pid` is `None` when the shell adopted an already-running server
    /// instead of spawning one.
    Connected {
        endpoint: String,
        pid: Option<u32>,
    },
    Disconnected {
        reason: String,
    },
}

impl fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connecting { endpoint } => write!(f, "connecting to {endpoint}"),
            Self::Connected { endpoint, pid } => match pid {
                Some(pid) => write!(f, "connected to {endpoint} (pid {pid})"),
                None => write!(f, "connected to {endpoint} (adopted)"),
            },
            Self::Disconnected { reason } => write!(f, "disconnected: {reason}"),
        }
    }
}

struct Inner {
    child: Option<Child>,
    status: ServerStatus,
}

/// Owns the spawned `clay-server` child process and its readiness probing.
///
/// Dropping the supervisor kills and reaps the child, so an exiting desktop
/// app cannot leak a server process. `shutdown()` provides the explicit
/// clean-exit path used from the Tauri `RunEvent::Exit` hook.
pub struct Supervisor {
    endpoint: IpcEndpoint,
    /// Readiness probe budget. Defaults suit interactive launch; tests shrink
    /// them so failure paths stay fast and deterministic.
    probe_interval: Duration,
    probe_deadline: Duration,
    /// Incremented per supervised attempt so a stale probe thread from a
    /// previous attempt cannot act on (or kill) a newer child.
    generation: AtomicU64,
    /// Test hook: explicit server binary path. `None` resolves through
    /// [`resolve_server_binary`].
    server_binary: Option<PathBuf>,
    inner: Mutex<Option<Inner>>,
}

impl Supervisor {
    /// Integration-test constructor: fixed binary and probe budgets.
    #[doc(hidden)]
    pub fn with_test_config(
        endpoint: IpcEndpoint,
        server_binary: Option<PathBuf>,
        probe_interval: Duration,
        probe_deadline: Duration,
    ) -> Self {
        Self {
            endpoint,
            probe_interval,
            probe_deadline,
            generation: AtomicU64::new(0),
            server_binary,
            inner: Mutex::new(None),
        }
    }

    pub fn new(endpoint: IpcEndpoint) -> Self {
        Self {
            endpoint,
            probe_interval: Duration::from_millis(150),
            probe_deadline: Duration::from_secs(15),
            generation: AtomicU64::new(0),
            server_binary: None,
            inner: Mutex::new(None),
        }
    }

    /// Launches `clay-server` for the supervised endpoint (idempotent while a
    /// previous attempt is still tracked) and starts readiness probing on a
    /// background thread.
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned, which can only follow an
    /// active panic elsewhere in this module.
    pub fn start(self: &Arc<Self>) {
        let mut guard = self.lock();
        if guard.is_some() {
            return;
        }

        // Adopt an already-running server only after a real handshake: a
        // stale server from an older build otherwise gets adopted and every
        // later session fails with UnsupportedProtocolVersion.
        match tauri::async_runtime::block_on(clay::client::probe_protocol(&self.endpoint)) {
            clay::client::ProtocolProbe::Compatible => {
                *guard = Some(Inner {
                    status: ServerStatus::Connected {
                        endpoint: self.endpoint.to_string(),
                        pid: None,
                    },
                    child: None,
                });
            }
            clay::client::ProtocolProbe::Incompatible => {
                // Occupied by something that cannot serve this client;
                // spawning would just fail with EndpointInUse.
                *guard = Some(Inner {
                    status: ServerStatus::Disconnected {
                        reason: "endpoint is served by an incompatible Clay server \
                                 (protocol version mismatch); stop that server or \
                                 choose another endpoint via CLAY_ENDPOINT"
                            .to_string(),
                    },
                    child: None,
                });
            }
            clay::client::ProtocolProbe::NotListening => self.spawn_locked(guard),
        }
    }

    /// Spawns the configured binary for the supervised endpoint. Takes the
    /// state lock so it can be released before readiness probing starts.
    fn spawn_locked(self: &Arc<Self>, mut guard: MutexGuard<'_, Option<Inner>>) {
        let binary = self
            .server_binary
            .clone()
            .unwrap_or_else(resolve_server_binary);
        let mut command = Command::new(&binary);
        command
            .arg(self.endpoint.as_child_arg())
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        // Surface server logs on stderr in debug builds; discard in release to
        // avoid interleaving with the desktop process's own logging.
        if cfg!(debug_assertions) {
            command.stderr(Stdio::inherit());
        } else {
            command.stderr(Stdio::null());
        }

        match command.spawn() {
            Ok(child) => {
                let status = ServerStatus::Connecting {
                    endpoint: self.endpoint.to_string(),
                };
                *guard = Some(Inner {
                    status: status.clone(),
                    child: Some(child),
                });
                drop(guard);
                // This attempt's generation id: fetch_add returns the previous
                // counter, so +1 yields an id no shutdown/restart can have
                // pre-invalidate.
                let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
                self.spawn_probe_thread(generation);
            }
            Err(error) => {
                *guard = Some(Inner {
                    status: ServerStatus::Disconnected {
                        reason: classify_spawn_error(&binary, &error),
                    },
                    child: None,
                });
            }
        }
    }

    /// Records a typed disconnected status without spawning. Used when the
    /// configured endpoint is rejected before start.
    pub fn mark_disconnected(&self, reason: impl Into<String>) {
        let mut guard = self.lock();
        *guard = Some(Inner {
            status: ServerStatus::Disconnected {
                reason: reason.into(),
            },
            child: None,
        });
    }

    /// Kills and reaps the supervised child, if any. Safe to call repeatedly.
    pub fn shutdown(&self) {
        // Invalidate any in-flight probe before touching state.
        self.generation.fetch_add(1, Ordering::Relaxed);
        let mut guard = self.lock();
        let Some(mut slot) = guard.take() else {
            return;
        };
        if let Some(child) = slot.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait(); // reap: no zombie, no orphan
        }
        slot.child = None;
        if matches!(slot.status, ServerStatus::Connecting { .. }) {
            slot.status = ServerStatus::Disconnected {
                reason: "shutdown".to_string(),
            };
        }
        *guard = Some(slot);
    }

    pub fn restart(self: &Arc<Self>) {
        self.shutdown();
        // Clear any terminal disconnected marker so the UI shows the new
        // attempt instead of stale failure text.
        *self.lock() = None;
        self.start();
    }

    pub fn status(&self) -> ServerStatus {
        match self.lock().as_ref() {
            Some(inner) => inner.status.clone(),
            None => ServerStatus::Connecting {
                endpoint: self.endpoint.to_string(),
            },
        }
    }

    /// Child pid while a process is tracked (test/lifecycle introspection).
    #[cfg(test)]
    pub fn child_pid(&self) -> Option<u32> {
        self.lock().as_ref()?.child.as_ref().map(|child| child.id())
    }

    fn lock(&self) -> MutexGuard<'_, Option<Inner>> {
        self.inner.lock().expect("supervisor lock")
    }

    fn spawn_probe_thread(self: &Arc<Self>, generation: u64) {
        let supervisor = Arc::clone(self);
        std::thread::Builder::new()
            .name("clay-server-probe".to_string())
            .spawn(move || supervisor.probe_until_ready(generation))
            .expect("spawn probe thread");
    }

    fn probe_until_ready(self: Arc<Self>, generation: u64) {
        let deadline = Instant::now() + self.probe_deadline;
        loop {
            if self.generation.load(Ordering::Relaxed) != generation {
                return; // superseded by restart/shutdown
            }
            {
                let mut guard = self.lock();
                let Some(inner) = guard.as_mut() else {
                    return; // shut down before readiness resolved
                };
                if let Some(child) = inner.child.as_mut() {
                    match child.try_wait() {
                        Ok(Some(exit)) => {
                            inner.status = ServerStatus::Disconnected {
                                reason: format!("clay-server exited early: {exit}"),
                            };
                            inner.child = None;
                            return;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            inner.status = ServerStatus::Disconnected {
                                reason: format!("clay-server poll failed: {error}"),
                            };
                            inner.child = None;
                            return;
                        }
                    }
                }
                match tauri::async_runtime::block_on(clay::client::probe_protocol(&self.endpoint)) {
                    clay::client::ProtocolProbe::Compatible => {
                        if let Some(inner) = guard.as_mut()
                            && let Some(child) = inner.child.as_ref()
                        {
                            inner.status = ServerStatus::Connected {
                                endpoint: self.endpoint.to_string(),
                                pid: Some(child.id()),
                            };
                        }
                        return;
                    }
                    clay::client::ProtocolProbe::Incompatible => {
                        // The spawned (or adopted-by-mistake) server cannot
                        // serve this client; waiting out the deadline would
                        // only hide the real cause.
                        if let Some(inner) = guard.as_mut() {
                            if let Some(child) = inner.child.as_mut() {
                                let _ = child.kill();
                                let _ = child.wait();
                            }
                            inner.child = None;
                            inner.status = ServerStatus::Disconnected {
                                reason: "clay-server answered with an incompatible \
                                         protocol version; install the matching sidecar \
                                         or set CLAY_SERVER_BIN"
                                    .to_string(),
                            };
                        }
                        return;
                    }
                    clay::client::ProtocolProbe::NotListening => {}
                }
            }
            if Instant::now() >= deadline {
                let mut guard = self.lock();
                if let Some(inner) = guard.as_mut()
                    && matches!(inner.status, ServerStatus::Connecting { .. })
                {
                    if let Some(child) = inner.child.as_mut() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    inner.child = None;
                    inner.status = ServerStatus::Disconnected {
                        reason: format!(
                            "endpoint did not accept within {}ms",
                            self.probe_deadline.as_millis()
                        ),
                    };
                }
                return;
            }
            std::thread::sleep(self.probe_interval);
        }
    }
}

/// Safety net only: probe threads hold `Arc` clones, so teardown triggered
/// purely by dropping the last owner may be deferred until they observe the
/// generation bump and finish. Production exit paths (`RunEvent::Exit`) call
/// [`Supervisor::shutdown`] explicitly for synchronous kill+reap.
impl Drop for Supervisor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes an executable stub that ignores its arguments and stays alive
    /// (never listens) until killed — the fake `clay-server` for lifecycle
    /// tests. Unix-only: process supervision tests assert via /proc anyway.
    #[cfg(unix)]
    fn write_fake_server(dir: &std::path::Path, label: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let script = dir.join(format!("fake-server-{label}.sh"));
        std::fs::write(&script, "#!/bin/sh\nexec sleep 30\n").expect("write stub");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("chmod stub");
        script
    }

    #[cfg(unix)]
    fn fake_server_supervisor(label: &str) -> (Arc<Supervisor>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        // Unique per-test socket inside a user-owned, auto-cleaned temp dir:
        // the server refuses sockets in directories it does not own.
        let endpoint = IpcEndpoint::UnixSocket(dir.path().join(format!("fake-{label}.sock")));
        let mut supervisor = Supervisor::new(endpoint);
        supervisor.server_binary = Some(write_fake_server(dir.path(), label));
        supervisor.probe_interval = Duration::from_millis(20);
        supervisor.probe_deadline = Duration::from_millis(400);
        (Arc::new(supervisor), dir)
    }

    #[test]
    fn missing_server_binary_reports_typed_disconnected() {
        let dir = tempfile::tempdir().expect("tempdir");
        #[cfg(unix)]
        let endpoint = IpcEndpoint::UnixSocket(dir.path().join("missing.sock"));
        #[cfg(windows)]
        let endpoint = IpcEndpoint::WindowsNamedPipe(r"\\.\pipe\clay-missing-test".into());
        #[cfg(not(any(unix, windows)))]
        let endpoint = IpcEndpoint::from_argument("missing");
        let mut supervisor = Supervisor::new(endpoint);
        supervisor.server_binary = Some(dir.path().join("no-such-clay-server"));
        let supervisor = Arc::new(supervisor);
        supervisor.start();
        match supervisor.status() {
            ServerStatus::Disconnected { reason } => {
                assert!(reason.contains("not found"), "{reason}");
                assert!(
                    !reason.contains(dir.path().to_string_lossy().as_ref()),
                    "{reason}"
                );
            }
            other => panic!("expected Disconnected, got {other:?}"),
        }
    }

    #[test]
    fn mark_disconnected_skips_spawn() {
        let supervisor = Arc::new(Supervisor::new(clay::ipc::default_endpoint()));
        supervisor.mark_disconnected("network endpoints are not supported");
        match supervisor.status() {
            ServerStatus::Disconnected { reason } => {
                assert!(reason.contains("network endpoints"));
            }
            other => panic!("expected Disconnected, got {other:?}"),
        }
        assert_eq!(supervisor.child_pid(), None);
    }

    #[test]
    fn status_contract_serializes_typed_states() {
        let connecting = serde_json::to_value(ServerStatus::Connecting {
            endpoint: "/tmp/clay.sock".into(),
        })
        .unwrap();
        assert_eq!(connecting["state"], "connecting");

        let connected = serde_json::to_value(ServerStatus::Connected {
            endpoint: "/tmp/clay.sock".into(),
            pid: Some(4321),
        })
        .unwrap();
        assert_eq!(connected["state"], "connected");
        assert_eq!(connected["pid"], 4321);

        let adopted = serde_json::to_value(ServerStatus::Connected {
            endpoint: "/tmp/clay.sock".into(),
            pid: None,
        })
        .unwrap();
        assert_eq!(adopted["state"], "connected");
        assert!(adopted["pid"].is_null());

        let disconnected = serde_json::to_value(ServerStatus::Disconnected {
            reason: "boom".into(),
        })
        .unwrap();
        assert_eq!(disconnected["state"], "disconnected");
        assert_eq!(disconnected["reason"], "boom");
    }

    #[test]
    #[cfg(unix)]
    fn spawn_connect_shutdown_leaves_no_orphan() {
        let (supervisor, _dir) = fake_server_supervisor("lifecycle");
        assert_eq!(supervisor.child_pid(), None);

        supervisor.start();
        let pid = supervisor.child_pid().expect("child spawned");
        // The fake server never listens, so the probe must time out, kill the
        // child, and report a typed disconnected state.
        for _ in 0..200 {
            if let ServerStatus::Disconnected { .. } = supervisor.status() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            matches!(supervisor.status(), ServerStatus::Disconnected { .. }),
            "expected timed-out disconnect, got {:?}",
            supervisor.status()
        );
        assert_eq!(supervisor.child_pid(), None, "failed child must be cleared");
        assert!(!proc_entry_alive(pid), "reaped child left a /proc entry");

        supervisor.shutdown();
        assert!(matches!(
            supervisor.status(),
            ServerStatus::Disconnected { .. }
        ));
    }

    #[test]
    #[cfg(unix)]
    fn restart_replaces_child_and_reports_connecting() {
        let (supervisor, _dir) = fake_server_supervisor("restart");
        supervisor.start();
        let first = supervisor.child_pid();

        supervisor.restart();
        let second = supervisor.child_pid();
        assert!(second.is_some(), "restart must spawn a fresh child");
        if let (Some(first), Some(second)) = (first, second) {
            assert_ne!(first, second, "restart must not reuse the old pid");
            assert!(!proc_entry_alive(first), "old child leaked across restart");
        }

        supervisor.shutdown();
    }

    #[test]
    #[cfg(unix)]
    fn dropping_supervisor_reaps_child() {
        let (supervisor, _dir) = fake_server_supervisor("drop");
        supervisor.start();
        let pid = supervisor.child_pid().expect("child spawned");
        // Explicit shutdown must reap synchronously even while the readiness
        // probe thread still holds an Arc: it observes the generation bump
        // and exits without acting.
        supervisor.shutdown();
        assert!(!proc_entry_alive(pid), "shutdown must kill+reap the child");
        drop(supervisor); // releases the idle probe-thread reference
    }

    /// Real end-to-end check: launches the actual `clay-server` binary built
    /// alongside the workspace, waits for a typed Connected transition, then
    /// verifies clean teardown. Skips (rather than fails) when the binary has
    /// not been built, so targeted suite runs stay usable.
    /// A listener that accepts but cannot speak the current protocol must be
    /// refused with a typed reason, never adopted and never spawned past.
    #[test]
    #[cfg(unix)]
    fn incompatible_listener_is_refused_with_typed_reason() {
        use std::io::{Read, Write as _};
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("stale.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        let server_thread = std::thread::spawn(move || {
            // Answer one probe, then keep accepting-and-stalling so any
            // mistaken spawn attempt would still not "become ready".
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 512];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(b"not-a-clay-handshake");
                let _ = stream.flush();
            }
            std::thread::sleep(Duration::from_secs(5));
        });

        let supervisor = Supervisor::with_test_config(
            IpcEndpoint::UnixSocket(socket),
            Some(PathBuf::from("/bin/false")),
            Duration::from_millis(10),
            Duration::from_millis(300),
        );
        let supervisor = Arc::new(supervisor);
        supervisor.start();
        match supervisor.status() {
            ServerStatus::Disconnected { reason } => {
                assert!(reason.contains("incompatible"), "{reason}");
            }
            other => panic!("expected typed refusal, got {other:?}"),
        }
        assert_eq!(supervisor.child_pid(), None);
        let _ = server_thread.join();
    }

    /// Adoption now requires a real protocol-v28 handshake: a scripted
    /// current-version Welcome is adoptable without spawning.
    #[test]
    #[cfg(unix)]
    fn adopted_server_reports_connected_without_spawn() {
        use clay::protocol::{ServerMessage, codec::Codec};
        use tokio::io::AsyncWriteExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("adopt.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).expect("bind");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let server_thread = std::thread::spawn(move || {
            // Accept in nonblocking mode and hand tokio a nonblocking fd.
            let stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return,
                }
            };
            // Answer through the real protocol codec so the probe sees a
            // genuine current-version Welcome.
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            // std accept() re-enables blocking mode on the child socket.
            if stream.set_nonblocking(true).is_err() {
                return;
            }
            runtime.block_on(async move {
                let Ok(mut stream) = tokio::net::UnixStream::from_std(stream) else {
                    return;
                };
                let codec = Codec::default();
                if codec.read_client_message(&mut stream).await.is_err() {
                    return;
                }
                let _ = codec
                    .write_server_message(
                        &mut stream,
                        &ServerMessage::Welcome {
                            client_id: 1,
                            protocol_version: clay::protocol::PROTOCOL_VERSION,
                        },
                    )
                    .await;
                let _ = stream.shutdown().await;
            });
        });

        let mut supervisor = Supervisor::new(IpcEndpoint::UnixSocket(socket));
        // Would poison the test if the spawn path ran instead of adoption.
        supervisor.server_binary = Some(PathBuf::from("/bin/false"));
        let supervisor = Arc::new(supervisor);

        supervisor.start();
        match supervisor.status() {
            ServerStatus::Connected { pid, .. } => {
                assert_eq!(pid, None, "adoption must not claim a spawned pid");
            }
            other => panic!("expected adopted Connected status, got {other:?}"),
        }
        assert_eq!(supervisor.child_pid(), None);
        supervisor.shutdown();
        let _ = server_thread.join();
    }

    #[test]
    #[cfg(unix)]
    fn real_clay_server_reaches_connected_then_shuts_down() {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        // Test binaries live under target/{debug,release}/deps; the server
        // binary sits two directories up in the profile root.
        let bin = exe
            .parent()
            .and_then(|dir| dir.parent())
            .map(|profile| profile.join("clay-server"))
            .filter(|candidate| candidate.is_file());
        let Some(bin) = bin else {
            eprintln!("skipping: clay-server binary not built");
            return;
        };

        // User-owned dir + auto cleanup: the server refuses to create a
        // socket in a directory it does not own (EndpointOwnership guard).
        let dir = tempfile::tempdir().expect("tempdir");
        let endpoint = IpcEndpoint::UnixSocket(dir.path().join("real.sock"));
        let mut supervisor = Supervisor::new(endpoint);
        supervisor.server_binary = Some(bin);
        supervisor.probe_interval = Duration::from_millis(50);
        supervisor.probe_deadline = Duration::from_secs(20);
        let supervisor = Arc::new(supervisor);

        supervisor.start();
        let mut connected = false;
        for _ in 0..400 {
            if matches!(supervisor.status(), ServerStatus::Connected { .. }) {
                connected = true;
                break;
            }
            if matches!(supervisor.status(), ServerStatus::Disconnected { .. }) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let final_status = supervisor.status();
        assert!(
            connected,
            "real server never reported Connected: {final_status}"
        );
        let pid = supervisor.child_pid();
        supervisor.shutdown();
        assert_eq!(supervisor.child_pid(), None);
        if let Some(pid) = pid {
            assert!(!proc_entry_alive(pid), "shutdown leaked the real server");
        }
    }

    #[cfg(unix)]
    fn proc_entry_alive(pid: u32) -> bool {
        std::path::Path::new("/proc").join(pid.to_string()).exists()
    }

    #[cfg(not(unix))]
    fn proc_entry_alive(_pid: u32) -> bool {
        false
    }
}
