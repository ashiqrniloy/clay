#![cfg(any(unix, windows))]

//! Plan 098 end-to-end large-document verification.
//!
//! Drives a real in-process Clay IPC server with a generated ≥ 50 MiB
//! synthetic UTF-8 fixture through open (head+chunks) / edit / save / reload,
//! asserting chunked transfer bounds and timing budgets, plus the oversized
//! (resident budget) and binary-file refusal paths. Fixtures are synthetic,
//! generated at test time under the OS temp dir, and never committed.

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use clay::{
    ipc::{IpcEndpoint, smoke_endpoint},
    perf::{
        budgets::MAX_CHUNK_BYTES,
        fixtures::{FixtureKind, FixtureSpec, generate_fixture},
    },
    protocol::{
        ClientMessage, DocumentAccess, DocumentTextHead, EditOperation, FileErrorCode,
        PROTOCOL_VERSION, ServerMessage, TabCommand, codec::Codec,
    },
    server::{IpcServer, ServerConfig},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time::timeout,
};

const LARGE_SIZE_BYTES: usize = 50 * 1024 * 1024;
/// One MiB above the 256 MiB server-owned resident document budget.
const OVERSIZE_SIZE_BYTES: usize = 257 * 1024 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(60);

#[tokio::test]
async fn large_document_open_edit_save_reload_roundtrip_is_chunked() {
    let root = temp_dir("large-doc-roundtrip");
    fs::create_dir_all(&root).unwrap();
    let fixture_path = root.join("large.txt");
    let original = write_unicode_fixture(&fixture_path, LARGE_SIZE_BYTES);
    let mut edited = b"clay-\xF0\x9F\xA6\x80-edit\n".to_vec();
    edited.extend_from_slice(&original);

    let (endpoint, server_task) = spawn_server("large-doc-roundtrip").await;
    let result = roundtrip_scenario(&endpoint, &root, &fixture_path, &original, &edited).await;
    server_task.abort();
    let _ = server_task.await;
    cleanup_endpoint(&endpoint);
    let _ = fs::remove_dir_all(&root);

    result.expect("large-document roundtrip scenario");
}

#[tokio::test]
async fn oversize_and_binary_files_refuse_with_visible_errors() {
    let root = temp_dir("large-doc-refusals");
    fs::create_dir_all(&root).unwrap();

    // Sparse file: the budget refusal triggers on metadata size before any
    // content is read, so no bytes need to be written.
    let oversize_path = root.join("oversize.txt");
    let oversize = fs::File::create(&oversize_path).unwrap();
    oversize.set_len(OVERSIZE_SIZE_BYTES as u64).unwrap();
    drop(oversize);

    let binary_path = root.join("binary.dat");
    let mut binary_bytes = b"hello binary".to_vec();
    binary_bytes.push(0);
    binary_bytes.extend_from_slice(b"content after a NUL byte");
    fs::write(&binary_path, &binary_bytes).unwrap();

    let (endpoint, server_task) = spawn_server("large-doc-refusals").await;
    let result = refusal_scenario(&endpoint, &root, &oversize_path, &binary_path).await;
    server_task.abort();
    let _ = server_task.await;
    cleanup_endpoint(&endpoint);
    let _ = fs::remove_dir_all(&root);

    result.expect("oversize/binary refusal scenario");
}

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

async fn roundtrip_scenario(
    endpoint: &IpcEndpoint,
    root: &Path,
    fixture_path: &Path,
    original: &[u8],
    edited: &[u8],
) -> Result<(), String> {
    let mut session = Session::connect(endpoint).await?;
    let token = session.capable_token().await?;
    session.new_tab(root).await?;

    // Open: head must arrive fast and bounded; first paint happens here.
    let open_started = Instant::now();
    let opened = session.open_selected(fixture_path, &token).await?;
    let head_latency = open_started.elapsed();
    println!(
        "open->head (first paint): {head_latency:?} (head {} bytes of {})",
        opened.head.first_chunk.len(),
        opened.head.total_bytes
    );
    assert!(
        head_latency < Duration::from_millis(500),
        "open->head took {head_latency:?}, budget is 500 ms"
    );
    assert_eq!(opened.path, "large.txt");
    assert!(
        opened.head.first_chunk.len() <= MAX_CHUNK_BYTES,
        "head chunk exceeds the wire budget"
    );
    assert_eq!(opened.head.total_bytes, original.len() as u64);

    // Assemble the remainder through bounded chunk requests.
    let full = session
        .assemble(opened.document_id, opened.version, opened.head.clone())
        .await?;
    let full_load = open_started.elapsed();
    println!("open->full load: {full_load:?} ({} bytes)", full.len());
    assert!(
        full_load < Duration::from_secs(5),
        "full load took {full_load:?}, budget is 5 s"
    );
    assert_eq!(full, original, "assembled document differs from fixture");

    // Edit locally-represented state through the optimistic protocol.
    let behavior_version = session.behavior_version.unwrap_or(1);
    session
        .edit(
            opened.document_id,
            opened.access.lease_id(),
            opened.version,
            behavior_version,
            1,
            EditOperation::Insert {
                byte_offset: 0,
                text: "clay-\u{1F980}-edit\n".to_string(),
            },
        )
        .await?;

    // Save and confirm the on-disk file actually changed.
    let save_started = Instant::now();
    session.save(opened.document_id, opened.version + 1).await?;
    println!(
        "save->ack: {:?} ({} bytes)",
        save_started.elapsed(),
        edited.len()
    );
    let saved_on_disk = fs::read(fixture_path).map_err(|error| error.to_string())?;
    assert_eq!(saved_on_disk, edited, "saved file differs from edited text");

    // Reload: server re-reads from disk and re-chunks the whole document.
    let reloaded_head = session
        .reload(opened.document_id, opened.version + 1)
        .await?;
    let reloaded = session
        .assemble(opened.document_id, opened.version + 1, reloaded_head)
        .await?;
    assert_eq!(reloaded, edited, "reload diverges from saved edits");

    Ok(())
}

async fn refusal_scenario(
    endpoint: &IpcEndpoint,
    root: &Path,
    oversize_path: &Path,
    binary_path: &Path,
) -> Result<(), String> {
    let mut session = Session::connect(endpoint).await?;
    let token = session.capable_token().await?;
    session.new_tab(root).await?;

    // Binary refusal: NUL sniff produces a typed, visible error.
    let failure = session.try_open_selected(binary_path, &token).await?;
    match failure {
        OpenOutcome::Failed { code, message } => {
            assert_eq!(code, clay::protocol::FileErrorCode::BinaryFileNotSupported);
            assert!(
                message.contains("binary"),
                "binary refusal message should mention binary: {message}"
            );
        }
        OpenOutcome::Opened(_) => return Err("binary file unexpectedly opened".to_string()),
    }

    // Oversize refusal: resident-budget gate fires on metadata size.
    let retry_token = session.capable_token().await?;
    let failure = session
        .try_open_selected(oversize_path, &retry_token)
        .await?;
    match failure {
        OpenOutcome::Failed { code, message } => {
            assert_eq!(code, clay::protocol::FileErrorCode::DocumentBudgetExceeded);
            assert!(
                message.contains("resident document budget"),
                "budget refusal message should name the budget: {message}"
            );
        }
        OpenOutcome::Opened(_) => return Err("oversize file unexpectedly opened".to_string()),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Session harness
// ---------------------------------------------------------------------------

struct OpenedDocument {
    #[allow(dead_code)]
    workspace_root_id: u64,
    document_id: u64,
    version: u64,
    access: DocumentAccess,
    path: String,
    head: DocumentTextHead,
}

enum OpenOutcome {
    Opened(OpenedDocument),
    Failed {
        code: FileErrorCode,
        message: String,
    },
}

struct Session<S> {
    codec: Codec,
    stream: S,
    client_id: u64,
    /// Latest observed behavior-manifest version (manifests arrive
    /// unprompted around opens).
    behavior_version: Option<u64>,
}

impl Session<SStream> {
    async fn connect(endpoint: &IpcEndpoint) -> Result<Session<SStream>, String> {
        let mut stream = connect_with_retry(endpoint).await;
        let codec = Codec::default();
        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "large-document-verification".to_string(),
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        let client_id = loop {
            match session_read(&codec, &mut stream).await? {
                ServerMessage::Welcome { client_id, .. } => break client_id,
                message if protocol_failure(&message).is_none() => continue,
                message => return Err(format!("expected Welcome, got {message:?}")),
            }
        };
        Ok(Self {
            codec,
            stream,
            client_id,
            behavior_version: None,
        })
    }

    async fn read(&mut self) -> Result<ServerMessage, String> {
        let message = session_read(&self.codec, &mut self.stream).await?;
        if let ServerMessage::BehaviorManifest(manifest) = &message {
            self.behavior_version = Some(manifest.behavior_version);
        }
        Ok(message)
    }

    /// Read until a fresh selected-file capability arrives.
    async fn capable_token(&mut self) -> Result<String, String> {
        for _ in 0..32 {
            match self.read().await? {
                ServerMessage::FileOpenCapabilityIssued { token } => return Ok(token),
                message if protocol_failure(&message).is_none() => continue,
                message => return Err(format!("expected capability, got {message:?}")),
            }
        }
        Err("server never issued a file-open capability".to_string())
    }

    async fn new_tab(&mut self, root: &Path) -> Result<(), String> {
        self.codec
            .write_client_message(
                &mut self.stream,
                &ClientMessage::TabCommand {
                    client_id: self.client_id,
                    command: TabCommand::New {
                        workspace_root: root.to_string_lossy().into_owned(),
                    },
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        loop {
            match self.read().await? {
                ServerMessage::TabRegistry(_) => break,
                message if protocol_failure(&message).is_none() => continue,
                message => return Err(format!("expected tab registry, got {message:?}")),
            }
        }
        Ok(())
    }

    /// Send an authorized selected-path open and expect it to succeed.
    async fn open_selected(&mut self, path: &Path, token: &str) -> Result<OpenedDocument, String> {
        match self.try_open_selected(path, token).await? {
            OpenOutcome::Opened(document) => Ok(document),
            OpenOutcome::Failed { code, message } => {
                Err(format!("open failed ({code:?}): {message}"))
            }
        }
    }

    async fn try_open_selected(&mut self, path: &Path, token: &str) -> Result<OpenOutcome, String> {
        self.codec
            .write_client_message(
                &mut self.stream,
                &ClientMessage::OpenSelectedFile {
                    client_id: self.client_id,
                    capability: token.to_string(),
                    selected_path: path.to_string_lossy().into_owned(),
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        loop {
            match self.read().await? {
                ServerMessage::DocumentOpened { metadata, head } => {
                    return Ok(OpenOutcome::Opened(OpenedDocument {
                        workspace_root_id: metadata.workspace_root_id,
                        document_id: metadata.document_id,
                        version: metadata.version,
                        access: metadata.access,
                        path: metadata.path,
                        head,
                    }));
                }
                ServerMessage::FileOperationFailed { code, message, .. } => {
                    return Ok(OpenOutcome::Failed { code, message });
                }
                message if protocol_failure(&message).is_none() => continue,
                message => return Err(format!("expected DocumentOpened, got {message:?}")),
            }
        }
    }

    /// Fetch chunks until `total` wire bytes are assembled. Every response is
    /// asserted against the per-chunk wire budget, and the sequential
    /// request continues from the received end (server replies are clamped
    /// to UTF-8 char boundaries).
    async fn assemble(
        &mut self,
        document_id: u64,
        version: u64,
        head: DocumentTextHead,
    ) -> Result<Vec<u8>, String> {
        let total = head.total_bytes as usize;
        let mut data = head.first_chunk.into_bytes();
        while data.len() < total {
            self.codec
                .write_client_message(
                    &mut self.stream,
                    &ClientMessage::DocumentChunkRequest {
                        client_id: self.client_id,
                        document_id,
                        document_version: version,
                        offset: data.len() as u64,
                        max_bytes: MAX_CHUNK_BYTES as u32,
                    },
                )
                .await
                .map_err(|error| error.to_string())?;
            loop {
                match self.read().await? {
                    ServerMessage::DocumentChunk { offset, text, .. } => {
                        assert_eq!(offset, data.len() as u64, "chunk gap/overlap");
                        assert!(
                            text.len() <= MAX_CHUNK_BYTES,
                            "chunk of {} bytes exceeds the {} byte wire budget",
                            text.len(),
                            MAX_CHUNK_BYTES
                        );
                        data.extend_from_slice(text.as_bytes());
                        break;
                    }
                    message if protocol_failure(&message).is_none() => continue,
                    message => {
                        return Err(format!("expected DocumentChunk, got {message:?}"));
                    }
                }
            }
        }
        assert_eq!(data.len(), total, "assembled length mismatch");
        Ok(data)
    }

    async fn edit(
        &mut self,
        document_id: u64,
        lease_id: Option<u64>,
        base_version: u64,
        behavior_version: u64,
        transaction_id: u64,
        operation: EditOperation,
    ) -> Result<(), String> {
        self.codec
            .write_client_message(
                &mut self.stream,
                &ClientMessage::Edit {
                    document_id,
                    client_id: self.client_id,
                    lease_id,
                    base_version,
                    behavior_version,
                    transaction_id,
                    operation,
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        loop {
            match self.read().await? {
                ServerMessage::EditAck {
                    confirmed_version, ..
                } => {
                    assert_eq!(confirmed_version, base_version + 1);
                    return Ok(());
                }
                message if protocol_failure(&message).is_none() => continue,
                message => return Err(format!("expected EditAck, got {message:?}")),
            }
        }
    }

    async fn save(&mut self, document_id: u64, known_version: u64) -> Result<(), String> {
        self.codec
            .write_client_message(
                &mut self.stream,
                &ClientMessage::SaveDocument {
                    client_id: self.client_id,
                    document_id,
                    known_version,
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        loop {
            match self.read().await? {
                ServerMessage::DocumentSaved { dirty, .. } => {
                    assert!(!dirty, "saved document should be clean");
                    return Ok(());
                }
                message if protocol_failure(&message).is_none() => continue,
                message => return Err(format!("expected DocumentSaved, got {message:?}")),
            }
        }
    }

    async fn reload(
        &mut self,
        document_id: u64,
        known_version: u64,
    ) -> Result<DocumentTextHead, String> {
        self.codec
            .write_client_message(
                &mut self.stream,
                &ClientMessage::ReloadDocument {
                    client_id: self.client_id,
                    document_id,
                    known_version,
                    force: false,
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        loop {
            match self.read().await? {
                ServerMessage::DocumentReloaded { head, .. } => return Ok(head),
                message if protocol_failure(&message).is_none() => continue,
                message => return Err(format!("expected DocumentReloaded, got {message:?}")),
            }
        }
    }
}

/// Only protocol-level failures interrupt wait loops; every other
/// asynchronous server push (themes, manifests, decorations, caret style,
/// ...) is tolerated while awaiting the message under test.
fn protocol_failure(message: &ServerMessage) -> Option<String> {
    match message {
        ServerMessage::FileOperationFailed { code, message, .. } => {
            Some(format!("file operation failed ({code:?}): {message}"))
        }
        ServerMessage::Error { code, message } => {
            Some(format!("server error ({code:?}): {message}"))
        }
        ServerMessage::EditRejected { reason, .. } => Some(format!("edit rejected: {reason:?}")),
        ServerMessage::DocumentChunkRejected { reason, .. } => {
            Some(format!("chunk rejected: {reason:?}"))
        }
        _ => None,
    }
}

async fn session_read<S: AsyncRead + AsyncWrite + Unpin>(
    codec: &Codec,
    stream: &mut S,
) -> Result<ServerMessage, String> {
    timeout(READ_TIMEOUT, codec.read_server_message(stream))
        .await
        .map_err(|_| "timed out waiting for server message".to_string())?
        .map_err(|error| error.to_string())
}

// ---------------------------------------------------------------------------
// Fixtures and process plumbing
// ---------------------------------------------------------------------------

/// Generate the deterministic mixed-unicode fixture and return its exact
/// bytes (regenerated from the same spec, never read back from disk).
fn write_unicode_fixture(path: &Path, size_bytes: usize) -> Vec<u8> {
    let spec = FixtureSpec::new(FixtureKind::MixedUnicode, size_bytes);
    let mut expected = Vec::with_capacity(size_bytes);
    generate_fixture(&spec, &mut expected).expect("fixture generation into memory");
    let mut file = fs::File::create(path).expect("create fixture file");
    file.write_all(&expected).expect("write fixture file");
    file.flush().expect("flush fixture file");
    expected
}

async fn spawn_server(label: &str) -> (IpcEndpoint, tokio::task::JoinHandle<()>) {
    let endpoint = smoke_endpoint(label);
    let config = ServerConfig::new(endpoint.clone());
    let server = IpcServer::try_new(config).expect("test server config is valid");
    let task = tokio::spawn(async move {
        let _ = server.run().await;
    });
    (endpoint, task)
}

async fn connect_with_retry(endpoint: &IpcEndpoint) -> SStream {
    #[cfg(unix)]
    {
        let path = endpoint.as_unix_socket_path();
        let mut last_error = None;
        for _ in 0..100 {
            match tokio::net::UnixStream::connect(path).await {
                Ok(stream) => return SStream::Unix(stream),
                Err(error) => last_error = Some(error),
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("failed to connect to test server: {last_error:?}");
    }
    #[cfg(windows)]
    {
        let name = endpoint.as_windows_named_pipe();
        use tokio::net::windows::named_pipe::ClientOptions;
        let mut last_error = None;
        for _ in 0..100 {
            match ClientOptions::new().open(name) {
                Ok(stream) => return SStream::NamedPipe(stream),
                Err(error) => last_error = Some(error),
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("failed to connect to test server: {last_error:?}");
    }
}

enum SStream {
    #[cfg(unix)]
    Unix(tokio::net::UnixStream),
    #[cfg(windows)]
    NamedPipe(tokio::net::windows::named_pipe::NamedPipeClient),
}

impl AsyncRead for SStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            #[cfg(unix)]
            SStream::Unix(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
            #[cfg(windows)]
            SStream::NamedPipe(stream) => std::pin::Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for SStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match &mut *self {
            #[cfg(unix)]
            SStream::Unix(stream) => std::pin::Pin::new(stream).poll_write(cx, buf),
            #[cfg(windows)]
            SStream::NamedPipe(stream) => std::pin::Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            #[cfg(unix)]
            SStream::Unix(stream) => std::pin::Pin::new(stream).poll_flush(cx),
            #[cfg(windows)]
            SStream::NamedPipe(stream) => std::pin::Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            #[cfg(unix)]
            SStream::Unix(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
            #[cfg(windows)]
            SStream::NamedPipe(stream) => std::pin::Pin::new(stream).poll_shutdown(cx),
        }
    }
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "clay-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn cleanup_endpoint(endpoint: &IpcEndpoint) {
    #[cfg(unix)]
    {
        let _ = fs::remove_file(endpoint.as_unix_socket_path());
    }
    #[cfg(windows)]
    {
        let _ = endpoint;
    }
}
