#![cfg(any(unix, windows))]

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use clay::{
    ipc::{IpcEndpoint, smoke_endpoint},
    protocol::{
        BehaviorScope, ClientMessage, DecorationKind, DiagnosticSeverity, PROTOCOL_VERSION,
        ServerMessage, codec::Codec,
    },
    server::{IpcServer, ServerConfig},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time::timeout,
};

#[tokio::test]
async fn selected_markdown_open_smoke_uses_capability_and_publishes_markdown_state() {
    let root = temp_dir("selected-markdown-open-smoke");
    fs::create_dir_all(&root).unwrap();
    let config_root = root.join("config");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");
"#,
    )
    .unwrap();
    let selected = root.join("note.md");
    fs::write(&selected, "# Smoke note\n\n- item with `code`\n").unwrap();

    let endpoint = smoke_endpoint("selected-markdown-open");
    let mut config = ServerConfig::new(endpoint.clone());
    config.configuration_root = Some(config_root);
    let server = IpcServer::try_new(config).expect("test server config is valid");
    let server = tokio::spawn(async move { server.run().await });

    let result = run_smoke(&endpoint, &selected).await;
    server.abort();
    let _ = server.await;
    cleanup_endpoint(&endpoint);
    let _ = fs::remove_dir_all(&root);

    result
}

async fn run_smoke(endpoint: &IpcEndpoint, selected: &Path) {
    let mut stream = connect_with_retry(endpoint).await;
    let codec = Codec::default();
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "selected-markdown-open-smoke".to_string(),
            },
        )
        .await
        .unwrap();

    let client_id = match read_message(&codec, &mut stream).await {
        ServerMessage::Welcome { client_id, .. } => client_id,
        message => panic!("expected Welcome, got {message:?}"),
    };
    assert!(matches!(
        read_message(&codec, &mut stream).await,
        ServerMessage::InitialDocument { .. }
    ));
    assert!(matches!(
        read_message(&codec, &mut stream).await,
        ServerMessage::BehaviorManifest(_)
    ));
    let first_token = read_until_capability(&codec, &mut stream).await;

    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::OpenSelectedFile {
                client_id,
                capability: String::new(),
                selected_path: selected.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();
    let retry_token = expect_capability(read_message(&codec, &mut stream).await);
    assert_ne!(first_token, retry_token);
    match read_message(&codec, &mut stream).await {
        ServerMessage::RuntimeDiagnostic(diagnostic) => {
            assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
            assert_eq!(
                diagnostic.code,
                "clay.client.selected_file_open.unauthorized"
            );
        }
        message => panic!("expected unauthorized RuntimeDiagnostic, got {message:?}"),
    }

    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::OpenSelectedFile {
                client_id,
                capability: retry_token,
                selected_path: selected.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();

    let opened_document_id = match read_message(&codec, &mut stream).await {
        ServerMessage::DocumentOpened { metadata, text } => {
            assert_eq!(metadata.path, "note.md");
            assert_eq!(text, "# Smoke note\n\n- item with `code`\n");
            metadata.document_id
        }
        message => panic!("expected DocumentOpened, got {message:?}"),
    };
    match read_message(&codec, &mut stream).await {
        ServerMessage::BehaviorManifest(manifest) => {
            assert_eq!(manifest.manifest_id, "markdown.markdown");
            assert!(matches!(
                manifest.scope,
                BehaviorScope::Document { document_id } if document_id == opened_document_id
            ));
        }
        message => panic!("expected Markdown BehaviorManifest, got {message:?}"),
    }
    match read_message(&codec, &mut stream).await {
        ServerMessage::DecorationSet(set) => {
            assert_eq!(set.document_id, opened_document_id);
            assert!(set.spans.iter().any(|span| {
                span.kind == DecorationKind::Syntax && span.style_token == "markup.heading.1"
            }));
            assert!(set.spans.iter().any(|span| {
                span.kind == DecorationKind::Syntax && span.style_token == "markup.inline-code"
            }));
        }
        message => panic!("expected Markdown DecorationSet, got {message:?}"),
    }
    let _next_token = expect_capability(read_message(&codec, &mut stream).await);
}

async fn read_message<S>(codec: &Codec, stream: &mut S) -> ServerMessage
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    timeout(Duration::from_secs(5), codec.read_server_message(stream))
        .await
        .expect("timed out waiting for server message")
        .expect("read server message")
}

async fn read_until_capability<S>(codec: &Codec, stream: &mut S) -> String
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    for _ in 0..8 {
        match read_message(codec, stream).await {
            ServerMessage::FileOpenCapabilityIssued { token } => return token,
            ServerMessage::SduiSnapshot { .. } | ServerMessage::RuntimeDiagnostic(_) => continue,
            message => panic!("expected FileOpenCapabilityIssued, got {message:?}"),
        }
    }
    panic!("server did not issue selected-file capability")
}

fn expect_capability(message: ServerMessage) -> String {
    match message {
        ServerMessage::FileOpenCapabilityIssued { token } => token,
        message => panic!("expected FileOpenCapabilityIssued, got {message:?}"),
    }
}

#[cfg(unix)]
async fn connect_with_retry(endpoint: &IpcEndpoint) -> tokio::net::UnixStream {
    let path = endpoint.as_unix_socket_path();
    let mut last_error = None;
    for _ in 0..100 {
        match tokio::net::UnixStream::connect(path).await {
            Ok(stream) => return stream,
            Err(error) => last_error = Some(error),
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("failed to connect to test server: {last_error:?}");
}

#[cfg(windows)]
async fn connect_with_retry(
    endpoint: &IpcEndpoint,
) -> tokio::net::windows::named_pipe::NamedPipeClient {
    use tokio::net::windows::named_pipe::ClientOptions;

    let name = endpoint.as_windows_named_pipe();
    let mut last_error = None;
    for _ in 0..100 {
        match ClientOptions::new().open(name) {
            Ok(stream) => return stream,
            Err(error) => last_error = Some(error),
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("failed to connect to test server: {last_error:?}");
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "clay-{label}-{}-{}",
        std::process::id(),
        unique_suffix()
    ))
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
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
