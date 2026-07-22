//! Host-owned, deny-by-default language-server process/session service.
//!
//! A [`LanguageServerProcessService`] owns a bounded table of opaque child
//! sessions behind a dedicated background runtime. Package JavaScript never
//! receives a raw process or stdio handle: it exchanges validated, opaque,
//! bounded exact byte chunks over a typed session ID, and every operation
//! rechecks the current exact grant recorded by `PackageService`.
//!
//! Spawn parameters come only from a fixed, validated
//! [`LanguageServerContributionDescriptor`] plus an approved workspace-root
//! canonical path. The child is launched with `tokio::process::Command`
//! (never a shell string), `env_clear()` plus only the explicitly declared
//! inherited environment names, piped stdio, and `kill_on_drop`. See the
//! approved decision log `2026-07-14-2023-language-server-package-authority`
//! and `references/authority-boundaries.md` for the containment statement
//! (launch metadata does not prevent a same-user child from touching other
//! paths/network/subprocesses).

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::perf::budgets::{
    LANGUAGE_SERVER_MAX_SESSIONS, LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES,
    LANGUAGE_SERVER_READ_TIMEOUT_MS, LANGUAGE_SERVER_SESSION_COMMAND_CAPACITY,
    LANGUAGE_SERVER_STDERR_BUDGET_BYTES,
};

/// Opaque, non-guessable identifier for one live language-server session.
///
/// Assigned monotonically by the service; package code only ever receives the
/// `u64` and cannot forge access to another package's session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LanguageServerSessionId(u64);

impl LanguageServerSessionId {
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Construct a session id from an opaque JS-provided value. The service
    /// re-verifies package/contribution/fingerprint on every operation, so a
    /// forged id cannot reach another package's session.
    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }
}

/// Fixed, grant-validated spawn parameters for one session. Resolved by the
/// caller from the installed descriptor and exact grant immediately before
/// launch; the background task does not re-read package input.
#[derive(Debug, Clone)]
pub struct LanguageServerSpawn {
    pub package_name: String,
    pub contribution_id: String,
    pub descriptor_fingerprint: u64,
    pub canonical_executable: PathBuf,
    pub args: Vec<String>,
    pub inherit_environment: Vec<String>,
    pub cwd: PathBuf,
}

#[derive(Debug)]
struct SessionHandle {
    package_name: String,
    contribution_id: String,
    descriptor_fingerprint: u64,
    cwd: PathBuf,
    command_tx: mpsc::Sender<SessionActorCommand>,
    stop_tx: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

#[derive(Debug)]
struct SessionProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: Arc<tokio::sync::Mutex<StderrCapture>>,
    stderr_task: Option<JoinHandle<()>>,
}

#[derive(Debug)]
enum SessionActorCommand {
    Write {
        bytes: Vec<u8>,
        reply: oneshot::Sender<Result<(), LanguageServerError>>,
    },
    Read {
        max_bytes: usize,
        timeout_ms: u64,
        reply: oneshot::Sender<Result<Vec<u8>, LanguageServerError>>,
    },
}

impl SessionActorCommand {
    fn reply_error(self, error: LanguageServerError) {
        match self {
            Self::Write { reply, .. } => {
                let _ = reply.send(Err(error));
            }
            Self::Read { reply, .. } => {
                let _ = reply.send(Err(error));
            }
        }
    }
}

#[derive(Debug, Default)]
struct StderrCapture {
    bytes: Vec<u8>,
    truncated: bool,
}

#[derive(Debug)]
enum SessionCommand {
    Start {
        session_id: LanguageServerSessionId,
        spawn: LanguageServerSpawn,
        reply: oneshot::Sender<Result<(), LanguageServerError>>,
    },
    Write {
        session: LanguageServerSessionId,
        package: String,
        contribution: String,
        descriptor_fingerprint: u64,
        bytes: Vec<u8>,
        reply: oneshot::Sender<Result<(), LanguageServerError>>,
    },
    Read {
        session: LanguageServerSessionId,
        package: String,
        contribution: String,
        descriptor_fingerprint: u64,
        max_bytes: usize,
        timeout_ms: u64,
        reply: oneshot::Sender<Result<Vec<u8>, LanguageServerError>>,
    },
    Stop {
        session: LanguageServerSessionId,
        package: String,
        contribution: String,
        descriptor_fingerprint: u64,
        reply: oneshot::Sender<Result<(), LanguageServerError>>,
    },
    RevokeForPackage {
        package: String,
        reply: oneshot::Sender<usize>,
    },
    ShutdownAll {
        reply: oneshot::Sender<usize>,
    },
    SessionCount {
        reply: oneshot::Sender<usize>,
    },
}

/// Host-owned language-server process/session service.
///
/// The service owns one dedicated current-thread Tokio runtime. Its central
/// router handles only bounded identity/table routing; each child process and
/// its stdio live in an independent session actor on that runtime, isolated
/// from persistent JavaScript workers and the Masonry render path.
#[derive(Clone)]
pub struct LanguageServerProcessService {
    inner: Arc<ServiceInner>,
}

struct ServiceInner {
    command_tx: mpsc::Sender<SessionCommand>,
    next_session_id: AtomicU64,
}

impl Default for LanguageServerProcessService {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageServerProcessService {
    pub fn new() -> Self {
        // Central ingress is bounded above the session cap. The router performs
        // only table/identity checks and non-blocking actor dispatch; child I/O
        // never runs in this loop.
        let (command_tx, command_rx) = mpsc::channel(64);
        let inner = Arc::new(ServiceInner {
            command_tx,
            next_session_id: AtomicU64::new(1),
        });
        spawn_router_thread(command_rx);
        Self { inner }
    }

    fn next_session_id(&self) -> LanguageServerSessionId {
        LanguageServerSessionId(self.inner.next_session_id.fetch_add(1, Ordering::Relaxed))
    }

    pub async fn start(
        &self,
        spawn: LanguageServerSpawn,
    ) -> Result<LanguageServerSessionId, LanguageServerError> {
        let session_id = self.next_session_id();
        let (reply_tx, reply_rx) = oneshot::channel();
        self.inner
            .command_tx
            .send(SessionCommand::Start {
                session_id,
                spawn,
                reply: reply_tx,
            })
            .await
            .map_err(|_| LanguageServerError::ServiceStopped)?;
        reply_rx
            .await
            .map_err(|_| LanguageServerError::ServiceStopped)??;
        Ok(session_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn write(
        &self,
        session: LanguageServerSessionId,
        package: String,
        contribution: String,
        descriptor_fingerprint: u64,
        bytes: Vec<u8>,
    ) -> Result<(), LanguageServerError> {
        if bytes.len() > LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES {
            return Err(LanguageServerError::PayloadTooLarge {
                len: bytes.len(),
                max: LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES,
            });
        }
        let (reply_tx, reply_rx) = oneshot::channel();
        self.inner
            .command_tx
            .send(SessionCommand::Write {
                session,
                package,
                contribution,
                descriptor_fingerprint,
                bytes,
                reply: reply_tx,
            })
            .await
            .map_err(|_| LanguageServerError::ServiceStopped)?;
        reply_rx
            .await
            .map_err(|_| LanguageServerError::ServiceStopped)?
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn read(
        &self,
        session: LanguageServerSessionId,
        package: String,
        contribution: String,
        descriptor_fingerprint: u64,
        max_bytes: usize,
        timeout_ms: u64,
    ) -> Result<Vec<u8>, LanguageServerError> {
        if max_bytes > LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES {
            return Err(LanguageServerError::PayloadTooLarge {
                len: max_bytes,
                max: LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES,
            });
        }
        let (reply_tx, reply_rx) = oneshot::channel();
        self.inner
            .command_tx
            .send(SessionCommand::Read {
                session,
                package,
                contribution,
                descriptor_fingerprint,
                max_bytes,
                timeout_ms,
                reply: reply_tx,
            })
            .await
            .map_err(|_| LanguageServerError::ServiceStopped)?;
        reply_rx
            .await
            .map_err(|_| LanguageServerError::ServiceStopped)?
    }

    pub async fn stop(
        &self,
        session: LanguageServerSessionId,
        package: String,
        contribution: String,
        descriptor_fingerprint: u64,
    ) -> Result<(), LanguageServerError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.inner
            .command_tx
            .send(SessionCommand::Stop {
                session,
                package,
                contribution,
                descriptor_fingerprint,
                reply: reply_tx,
            })
            .await
            .map_err(|_| LanguageServerError::ServiceStopped)?;
        reply_rx
            .await
            .map_err(|_| LanguageServerError::ServiceStopped)?
    }

    /// Kill and reap every session owned by `package`. Returns the number of
    /// sessions terminated. Called from package disable/revoke paths so a
    /// revoked grant's live sessions are torn down promptly rather than only
    /// on the next access.
    ///
    // ponytail: no runtime package-disable op exists in Phase 18.20 (disable
    // is CLI/Generation-swap only, which drops the whole service). This hook is
    // exercised by tests now and wired into the disable op in Phase 18.21.
    #[allow(dead_code)]
    pub async fn revoke_for_package(&self, package: &str) -> usize {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .inner
            .command_tx
            .send(SessionCommand::RevokeForPackage {
                package: package.to_string(),
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            return 0;
        }
        reply_rx.await.unwrap_or(0)
    }

    /// Kill and reap every live session. Used when a runtime generation is
    /// replaced so previous-generation language-server authority ends immediately
    /// rather than waiting for `Drop` of the owning service.
    pub async fn shutdown_all(&self) -> usize {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .inner
            .command_tx
            .send(SessionCommand::ShutdownAll { reply: reply_tx })
            .await
            .is_err()
        {
            return 0;
        }
        reply_rx.await.unwrap_or(0)
    }

    /// Current live session count. Test/diagnostic helper for generation cleanup.
    pub async fn session_count(&self) -> usize {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .inner
            .command_tx
            .send(SessionCommand::SessionCount { reply: reply_tx })
            .await
            .is_err()
        {
            return 0;
        }
        reply_rx.await.unwrap_or(0)
    }
}

fn spawn_router_thread(mut command_rx: mpsc::Receiver<SessionCommand>) {
    if let Err(_error) = std::thread::Builder::new()
        .name("clay-language-server".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            runtime.block_on(router_loop(&mut command_rx));
        })
    {
        // Thread spawn failure drops the receiver, which closes the channel;
        // every subsequent op observes `ServiceStopped` instead of hanging.
    }
}

async fn router_loop(command_rx: &mut mpsc::Receiver<SessionCommand>) {
    let mut sessions: HashMap<LanguageServerSessionId, SessionHandle> = HashMap::new();
    while let Some(command) = command_rx.recv().await {
        match command {
            SessionCommand::Start {
                session_id,
                spawn,
                reply,
            } => {
                let result = handle_start(&mut sessions, session_id, spawn);
                let _ = reply.send(result);
            }
            SessionCommand::Write {
                session,
                package,
                contribution,
                descriptor_fingerprint,
                bytes,
                reply,
            } => route_actor_command(
                &mut sessions,
                session,
                &package,
                &contribution,
                descriptor_fingerprint,
                SessionActorCommand::Write { bytes, reply },
            ),
            SessionCommand::Read {
                session,
                package,
                contribution,
                descriptor_fingerprint,
                max_bytes,
                timeout_ms,
                reply,
            } => route_actor_command(
                &mut sessions,
                session,
                &package,
                &contribution,
                descriptor_fingerprint,
                SessionActorCommand::Read {
                    max_bytes,
                    timeout_ms,
                    reply,
                },
            ),
            SessionCommand::Stop {
                session,
                package,
                contribution,
                descriptor_fingerprint,
                reply,
            } => {
                let handle = match sessions.get(&session) {
                    Some(handle) => {
                        match verify_identity(
                            handle,
                            &package,
                            &contribution,
                            descriptor_fingerprint,
                        ) {
                            Ok(()) => sessions.remove(&session),
                            Err(error) => {
                                let _ = reply.send(Err(error));
                                continue;
                            }
                        }
                    }
                    None => None,
                };
                tokio::spawn(async move {
                    if let Some(handle) = handle {
                        stop_actor(handle).await;
                    }
                    let _ = reply.send(Ok(()));
                });
            }
            SessionCommand::RevokeForPackage { package, reply } => {
                let matching: Vec<_> = sessions
                    .iter()
                    .filter(|(_, session)| session.package_name == package)
                    .map(|(id, _)| *id)
                    .collect();
                let handles: Vec<_> = matching
                    .iter()
                    .filter_map(|id| sessions.remove(id))
                    .collect();
                let count = handles.len();
                tokio::spawn(async move {
                    stop_actors(handles).await;
                    let _ = reply.send(count);
                });
            }
            SessionCommand::ShutdownAll { reply } => {
                let handles: Vec<_> = sessions.drain().map(|(_, handle)| handle).collect();
                let count = handles.len();
                tokio::spawn(async move {
                    stop_actors(handles).await;
                    let _ = reply.send(count);
                });
            }
            SessionCommand::SessionCount { reply } => {
                let _ = reply.send(sessions.len());
            }
        }
    }
    // Channel closed (service dropped / runtime replacement): stop and reap
    // every actor before the dedicated runtime exits.
    stop_actors(sessions.drain().map(|(_, handle)| handle).collect()).await;
}

fn handle_start(
    sessions: &mut HashMap<LanguageServerSessionId, SessionHandle>,
    session_id: LanguageServerSessionId,
    spawn: LanguageServerSpawn,
) -> Result<(), LanguageServerError> {
    if sessions.values().any(|session| {
        session.package_name == spawn.package_name
            && session.contribution_id == spawn.contribution_id
            && session.descriptor_fingerprint == spawn.descriptor_fingerprint
            && session.cwd == spawn.cwd
    }) {
        return Err(LanguageServerError::SessionAlreadyRunning);
    }
    if sessions.len() >= LANGUAGE_SERVER_MAX_SESSIONS {
        return Err(LanguageServerError::TooManySessions {
            max: LANGUAGE_SERVER_MAX_SESSIONS,
        });
    }
    let mut command = Command::new(&spawn.canonical_executable);
    command
        .args(&spawn.args)
        .current_dir(&spawn.cwd)
        .env_clear();
    for name in &spawn.inherit_environment {
        if let Ok(value) = std::env::var(name) {
            command.env(name, value);
        }
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(LanguageServerError::Spawn)?;
    let stdin = child.stdin.take().ok_or(LanguageServerError::MissingPipe)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(LanguageServerError::MissingPipe)?;
    let stderr = child
        .stderr
        .take()
        .ok_or(LanguageServerError::MissingPipe)?;
    let stderr_capture = Arc::new(tokio::sync::Mutex::new(StderrCapture::default()));
    let stderr_task = tokio::spawn(read_capped_stderr(stderr, Arc::clone(&stderr_capture)));
    let process = SessionProcess {
        child,
        stdin,
        stdout,
        stderr: stderr_capture,
        stderr_task: Some(stderr_task),
    };
    let (command_tx, command_rx) = mpsc::channel(LANGUAGE_SERVER_SESSION_COMMAND_CAPACITY);
    let (stop_tx, stop_rx) = oneshot::channel();
    let task = tokio::spawn(session_actor(process, command_rx, stop_rx));
    sessions.insert(
        session_id,
        SessionHandle {
            package_name: spawn.package_name,
            contribution_id: spawn.contribution_id,
            descriptor_fingerprint: spawn.descriptor_fingerprint,
            cwd: spawn.cwd,
            command_tx,
            stop_tx: Some(stop_tx),
            task: Some(task),
        },
    );
    Ok(())
}

fn route_actor_command(
    sessions: &mut HashMap<LanguageServerSessionId, SessionHandle>,
    session: LanguageServerSessionId,
    package: &str,
    contribution: &str,
    descriptor_fingerprint: u64,
    command: SessionActorCommand,
) {
    let Some(handle) = sessions.get(&session) else {
        command.reply_error(LanguageServerError::UnknownSession);
        return;
    };
    if let Err(error) = verify_identity(handle, package, contribution, descriptor_fingerprint) {
        command.reply_error(error);
        return;
    }
    match handle.command_tx.try_send(command) {
        Ok(()) => {}
        Err(mpsc::error::TrySendError::Full(command)) => {
            command.reply_error(LanguageServerError::SessionBusy)
        }
        Err(mpsc::error::TrySendError::Closed(command)) => {
            command.reply_error(LanguageServerError::UnknownSession);
            sessions.remove(&session);
        }
    }
}

async fn session_actor(
    mut process: SessionProcess,
    mut command_rx: mpsc::Receiver<SessionActorCommand>,
    mut stop_rx: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            biased;
            _ = &mut stop_rx => break,
            command = command_rx.recv() => {
                let Some(command) = command else { break };
                match command {
                    SessionActorCommand::Write { bytes, reply } => {
                        tokio::select! {
                            biased;
                            _ = &mut stop_rx => break,
                            result = actor_write(&mut process, &bytes) => {
                                let _ = reply.send(result);
                            }
                        }
                    }
                    SessionActorCommand::Read { max_bytes, timeout_ms, reply } => {
                        tokio::select! {
                            biased;
                            _ = &mut stop_rx => break,
                            result = actor_read(&mut process, max_bytes, timeout_ms) => {
                                let _ = reply.send(result);
                            }
                        }
                    }
                }
            }
        }
    }
    let _ = process.child.start_kill();
    let _ = process.child.wait().await;
    if let Some(task) = process.stderr_task.take() {
        task.abort();
    }
}

async fn actor_write(
    process: &mut SessionProcess,
    bytes: &[u8],
) -> Result<(), LanguageServerError> {
    process
        .stdin
        .write_all(bytes)
        .await
        .map_err(|error| LanguageServerError::Io(error.to_string()))?;
    process
        .stdin
        .flush()
        .await
        .map_err(|error| LanguageServerError::Io(error.to_string()))
}

async fn actor_read(
    process: &mut SessionProcess,
    max_bytes: usize,
    timeout_ms: u64,
) -> Result<Vec<u8>, LanguageServerError> {
    let limit = max_bytes.max(1);
    let mut buffer = vec![0u8; limit];
    let read_result = timeout(
        Duration::from_millis(if timeout_ms == 0 {
            LANGUAGE_SERVER_READ_TIMEOUT_MS
        } else {
            timeout_ms.min(LANGUAGE_SERVER_READ_TIMEOUT_MS)
        }),
        process.stdout.read(&mut buffer),
    )
    .await;
    match read_result {
        Ok(Ok(0)) => {
            let detail = sanitize_session_stderr(&process.stderr).await;
            Err(LanguageServerError::ChildExitedWith { detail })
        }
        Ok(Ok(count)) => {
            buffer.truncate(count);
            Ok(buffer)
        }
        Ok(Err(error)) => Err(LanguageServerError::Io(error.to_string())),
        Err(_) => Err(LanguageServerError::Timeout),
    }
}

async fn stop_actor(mut handle: SessionHandle) {
    if let Some(stop_tx) = handle.stop_tx.take() {
        let _ = stop_tx.send(());
    }
    if let Some(task) = handle.task.take() {
        let _ = task.await;
    }
}

async fn stop_actors(handles: Vec<SessionHandle>) {
    let mut tasks = tokio::task::JoinSet::new();
    for handle in handles {
        tasks.spawn(stop_actor(handle));
    }
    while tasks.join_next().await.is_some() {}
}

fn verify_identity(
    session: &SessionHandle,
    package: &str,
    contribution: &str,
    descriptor_fingerprint: u64,
) -> Result<(), LanguageServerError> {
    if session.package_name != package
        || session.contribution_id != contribution
        || session.descriptor_fingerprint != descriptor_fingerprint
    {
        return Err(LanguageServerError::IdentityMismatch);
    }
    Ok(())
}

async fn read_capped_stderr<R>(mut reader: R, capture: Arc<tokio::sync::Mutex<StderrCapture>>)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut chunk = [0u8; 4096];
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(read) => {
                let mut capture = capture.lock().await;
                let remaining =
                    LANGUAGE_SERVER_STDERR_BUDGET_BYTES.saturating_sub(capture.bytes.len());
                let retained = read.min(remaining);
                capture.bytes.extend_from_slice(&chunk[..retained]);
                capture.truncated |= retained < read;
                // Continue reading/discarding after retention fills so a child
                // cannot block forever on a full stderr pipe.
            }
            Err(_) => break,
        }
    }
}

fn sanitize_stderr(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|ch| !ch.is_control() || *ch == '\n' || *ch == '\t')
        .take(512)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Best-effort sanitized snapshot of accumulated child stderr. Retention is
/// capped at `LANGUAGE_SERVER_STDERR_BUDGET_BYTES`; the reader keeps draining
/// and discarding after the cap so the child pipe cannot fill.
async fn sanitize_session_stderr(stderr: &Arc<tokio::sync::Mutex<StderrCapture>>) -> String {
    let capture = stderr.lock().await;
    sanitize_stderr(&capture.bytes)
}

#[derive(Debug)]
pub enum LanguageServerError {
    Spawn(io::Error),
    MissingPipe,
    Io(String),
    Timeout,
    ChildExitedWith { detail: String },
    UnknownSession,
    IdentityMismatch,
    SessionAlreadyRunning,
    SessionBusy,
    TooManySessions { max: usize },
    PayloadTooLarge { len: usize, max: usize },
    ServiceStopped,
}

impl std::fmt::Display for LanguageServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(error) => {
                write!(f, "language-server failed to spawn child: {error}")
            }
            Self::MissingPipe => f.write_str("language-server child stdio pipe missing"),
            Self::Io(error) => write!(f, "language-server I/O failed: {error}"),
            Self::Timeout => f.write_str("language-server read timed out"),
            Self::ChildExitedWith { detail } => {
                if detail.is_empty() {
                    f.write_str("language-server child exited")
                } else {
                    write!(f, "language-server child exited: {detail}")
                }
            }
            Self::UnknownSession => f.write_str("language-server session not found"),
            Self::IdentityMismatch => f.write_str("language-server session identity mismatch"),
            Self::SessionAlreadyRunning => {
                f.write_str("language-server session already running for contribution and root")
            }
            Self::SessionBusy => f.write_str("language-server session command queue is full"),
            Self::TooManySessions { max } => {
                write!(f, "language-server session cap reached ({max})")
            }
            Self::PayloadTooLarge { len, max } => {
                write!(f, "language-server payload {len} exceeds maximum {max}")
            }
            Self::ServiceStopped => f.write_str("language-server process service stopped"),
        }
    }
}

impl std::error::Error for LanguageServerError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn session_id_is_opaque_u64() {
        let id = LanguageServerSessionId(42);
        assert_eq!(id.as_u64(), 42);
        assert_eq!(LanguageServerSessionId::from_u64(7).as_u64(), 7);
    }

    #[test]
    fn sanitize_stderr_strips_control_and_caps_length() {
        assert_eq!(sanitize_stderr(b"hello\x00\x07 world\n"), "hello world");
    }

    #[tokio::test]
    async fn capped_stderr_retains_prefix_and_drains_remainder_to_eof() {
        let (mut writer, reader) = tokio::io::duplex(1024);
        let capture = Arc::new(tokio::sync::Mutex::new(StderrCapture::default()));
        let reader_task = tokio::spawn(read_capped_stderr(reader, Arc::clone(&capture)));
        let payload = vec![b'x'; LANGUAGE_SERVER_STDERR_BUDGET_BYTES + 8192];
        let writer_task = tokio::spawn(async move {
            writer.write_all(&payload).await?;
            writer.shutdown().await
        });

        timeout(Duration::from_secs(2), writer_task)
            .await
            .expect("stderr writer must not block after retention fills")
            .expect("writer task")
            .expect("reader must keep the pipe open through EOF");
        reader_task.await.expect("stderr reader task");
        let capture = capture.lock().await;
        assert_eq!(capture.bytes.len(), LANGUAGE_SERVER_STDERR_BUDGET_BYTES);
        assert!(capture.truncated);
        assert!(capture.bytes.iter().all(|byte| *byte == b'x'));
    }

    // Self-check: the service actually spawns a fixed fake child, exchanges a
    // bounded opaque message, and revokes by package. Real LSP transport is
    // the Phase 18.21 adapter's job; this proves the bounded session primitive.
    #[cfg(unix)]
    #[tokio::test]
    async fn fake_child_session_exchanges_bounded_message_and_revokes() {
        let root = fake_workspace_root();
        let executable = fake_echo_child(&root);
        let service = LanguageServerProcessService::new();
        let spawn = LanguageServerSpawn {
            package_name: "example".to_string(),
            contribution_id: "example.echo".to_string(),
            descriptor_fingerprint: 0,
            canonical_executable: executable,
            args: Vec::new(),
            inherit_environment: Vec::new(),
            cwd: root.clone(),
        };
        let session = service.start(spawn).await.expect("session starts");

        service
            .write(
                session,
                "example".to_string(),
                "example.echo".to_string(),
                0,
                b"hello\n".to_vec(),
            )
            .await
            .expect("write bounded message");
        let reply = service
            .read(
                session,
                "example".to_string(),
                "example.echo".to_string(),
                0,
                256,
                2_000,
            )
            .await
            .expect("read bounded message");
        assert!(
            reply.starts_with(b"echo:hello"),
            "unexpected reply: {reply:?}"
        );

        // Identity mismatch is rejected: a different package cannot use the id.
        let mismatch = service
            .read(
                session,
                "other".to_string(),
                "example.echo".to_string(),
                0,
                256,
                100,
            )
            .await;
        assert!(matches!(
            mismatch,
            Err(LanguageServerError::IdentityMismatch)
        ));

        let revoked = service.revoke_for_package("example").await;
        assert_eq!(revoked, 1);
        let after = service
            .read(
                session,
                "example".to_string(),
                "example.echo".to_string(),
                0,
                256,
                100,
            )
            .await;
        assert!(matches!(after, Err(LanguageServerError::UnknownSession)));
    }

    #[cfg(unix)]
    fn fake_workspace_root() -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("clay-lang-server-{unique}"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::canonicalize(&root).unwrap()
    }

    #[cfg(unix)]
    fn fake_echo_child(root: &Path) -> PathBuf {
        use std::{io::Write, os::unix::fs::PermissionsExt};
        let path = root.join("fake-echo");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(
            b"#!/bin/sh\nwhile IFS= read -r line; do printf 'echo:%s\n' \"$line\"; done\n",
        )
        .unwrap();
        file.sync_all().unwrap();
        drop(file);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        path
    }
}
