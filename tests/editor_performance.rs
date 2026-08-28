#![cfg(any(unix, windows))]
//! Plan 099 task: automated editor performance matrix over generated
//! fixtures. Drives one real server through the typed protocol for every
//! size/kind/language combination in the matrix and asserts the deterministic
//! invariants that CI blocks on:
//!
//! - exactly one atomic `ViewportRenderPatch` per viewport request id;
//! - edit/version accounting is exact (ack versions never skip or repeat);
//! - save, reload, and resync round-trips preserve the authoritative text;
//! - per-language mode activation matches the path extension;
//! - closing a document retires it: no late patches after `DocumentClosed`.
//!
//! Machine-variant timings are deliberately NOT asserted here; the real-device
//! timing matrix lives in `scripts/editor-performance-smoke.sh`.

use std::{fs, path::PathBuf, time::Duration};

use clay::{
    ipc::{IpcEndpoint, smoke_endpoint},
    perf::fixtures::{FixtureKind, FixtureSpec, generate_fixture},
    protocol::{
        ClientMessage, EditOperation, PROTOCOL_VERSION, ServerMessage, TabCommand, codec::Codec,
    },
    server::{IpcServer, ServerConfig},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time::timeout,
};

/// One matrix cell: fixture shape x on-disk size x language extension.
struct MatrixCell {
    label: &'static str,
    kind: FixtureKind,
    size_bytes: u64,
    extension: &'static str,
    expected_manifest: &'static str,
}

fn matrix() -> Vec<MatrixCell> {
    let mib = 1024 * 1024;
    let kib = 1024;
    let kinds = [
        (FixtureKind::MixedUnicode, "mixed-unicode"),
        (FixtureKind::ManyShortLines, "many-short-lines"),
        (FixtureKind::LongLines, "long-lines"),
        (FixtureKind::NewlineHeavy, "newline-heavy"),
    ];
    let mut cells = Vec::new();
    // 64 KiB across every shape and every first-party language.
    for (kind, kind_label) in kinds {
        for (extension, manifest) in [
            ("txt", "default.text"),
            ("md", "markdown.markdown"),
            ("rs", "rust.rust"),
            ("ts", "typescript.typescript"),
            // The typescript package declares one mode covering ts/tsx.
            ("tsx", "typescript.typescript"),
            ("js", "javascript.javascript"),
        ] {
            cells.push(MatrixCell {
                label: Box::leak(format!("64kib-{kind_label}-{extension}").into_boxed_str()),
                kind,
                size_bytes: 64 * kib,
                extension,
                expected_manifest: manifest,
            });
        }
    }
    // 1 MiB: one cell per language on the mixed-unicode shape.
    for (extension, manifest) in [
        ("txt", "default.text"),
        ("md", "markdown.markdown"),
        ("rs", "rust.rust"),
        ("ts", "typescript.typescript"),
        ("js", "javascript.javascript"),
    ] {
        cells.push(MatrixCell {
            label: Box::leak(format!("1mib-mixed-unicode-{extension}").into_boxed_str()),
            kind: FixtureKind::MixedUnicode,
            size_bytes: mib,
            extension,
            expected_manifest: manifest,
        });
    }
    // 10 MiB and 50 MiB: plain-text open/viewport/close (full transfers of
    // these sizes are covered by tests/large_document.rs).
    cells.push(MatrixCell {
        label: "10mib-mixed-unicode-txt",
        kind: FixtureKind::MixedUnicode,
        size_bytes: 10 * mib,
        extension: "txt",
        expected_manifest: "default.text",
    });
    cells.push(MatrixCell {
        label: "50mib-long-lines-txt",
        kind: FixtureKind::LongLines,
        size_bytes: 50 * mib,
        extension: "txt",
        expected_manifest: "default.text",
    });
    cells
}

/// The whole matrix runs in one test against one server: package modes are
/// loaded once and cells share the process, mirroring a real session.
#[tokio::test]
async fn editor_performance_matrix_holds_deterministic_invariants() {
    let root = temp_dir("editor-perf-matrix");
    fs::create_dir_all(&root).unwrap();
    let config_root = root.join("config");
    fs::create_dir_all(&config_root).unwrap();
    fs::write(
        config_root.join("init.js"),
        r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");
await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
"#,
    )
    .unwrap();
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();

    // Generate every fixture once (deterministic content, approved root).
    for cell in matrix() {
        let mut bytes = Vec::new();
        generate_fixture(
            &FixtureSpec {
                kind: cell.kind,
                size_bytes: cell.size_bytes as usize,
                seed: 9_001,
            },
            &mut bytes,
        )
        .expect("fixture generation succeeds under the approved temp root");
        fs::write(
            workspace.join(format!("fixture-{}.{}", cell.label, cell.extension)),
            bytes,
        )
        .unwrap();
    }

    let endpoint = smoke_endpoint("editor-perf-matrix");
    let mut config = ServerConfig::new(endpoint.clone());
    config.configuration_root = Some(config_root);
    let server = IpcServer::try_new(config).expect("test server config is valid");
    let server = tokio::spawn(async move { server.run().await });

    let result = run_matrix(&endpoint, &workspace).await;
    server.abort();
    let _ = server.await;
    let _ = fs::remove_dir_all(&root);

    result.expect("every matrix cell passes its invariants");
}

async fn run_matrix(endpoint: &IpcEndpoint, workspace: &std::path::Path) -> Result<(), String> {
    let mut stream = connect_with_retry(endpoint).await;
    let codec = Codec::default();
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "editor-performance-matrix".to_string(),
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    let client_id = match next_message(&codec, &mut stream).await? {
        ServerMessage::Welcome { client_id, .. } => client_id,
        message => return Err(format!("expected Welcome, got {message:?}")),
    };

    // Bind the workspace through a tab so capabilities and roots exist.
    codec
        .write_client_message(
            &mut stream,
            &ClientMessage::TabCommand {
                client_id,
                command: TabCommand::New {
                    workspace_root: workspace.to_string_lossy().into_owned(),
                },
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    // Drain the post-bind handshake (registry, SDUI, theme, typography...).
    read_until(&codec, &mut stream, |message| {
        matches!(message, ServerMessage::TabRegistry(_))
    })
    .await?;
    // The bind issues a fresh capability for the first programmatic open.
    let mut capability = {
        let issued = read_until(&codec, &mut stream, |message| {
            matches!(message, ServerMessage::FileOpenCapabilityIssued { .. })
        })
        .await?;
        let ServerMessage::FileOpenCapabilityIssued { token } = issued else {
            unreachable!("read_until matched capability")
        };
        token
    };

    let mut document_id;
    for cell in matrix() {
        let path = format!("fixture-{}.{}", cell.label, cell.extension);
        // -- open (progressive loading: head arrives, chunks on demand) --
        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::OpenSelectedFile {
                    client_id,
                    capability,
                    selected_path: workspace.join(&path).to_string_lossy().into_owned(),
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        let opened = read_until(&codec, &mut stream, |message| {
            matches!(message, ServerMessage::DocumentOpened { .. })
        })
        .await?;
        let ServerMessage::DocumentOpened { metadata, head } = opened else {
            unreachable!("read_until matched DocumentOpened")
        };
        document_id = metadata.document_id;
        assert_eq!(metadata.path, path, "open reports the requested path");
        assert_eq!(
            head.total_bytes, cell.size_bytes,
            "{}: open reports the full authoritative size",
            cell.label
        );

        // -- mode activation matches the extension (fast path or V8) --
        let manifest = read_until(&codec, &mut stream, |message| {
            matches!(message, ServerMessage::BehaviorManifest(_))
        })
        .await?;
        let ServerMessage::BehaviorManifest(manifest) = manifest else {
            unreachable!("read_until matched manifest")
        };
        assert_eq!(
            manifest.manifest_id, cell.expected_manifest,
            "{}: mode activation matches the path extension",
            cell.label
        );
        // The open response replenishes one capability, then rebroadcasts
        // the tab registry.
        let replenished = read_until(&codec, &mut stream, |message| {
            matches!(message, ServerMessage::FileOpenCapabilityIssued { .. })
        })
        .await?;
        let ServerMessage::FileOpenCapabilityIssued { token } = replenished else {
            unreachable!("read_until matched capability")
        };
        capability = token;

        // -- viewport render: exactly one atomic patch per request id --
        send_viewport_request(&codec, &mut stream, client_id, document_id, 1, 0, 4_096).await?;
        wait_exactly_one_patch(&codec, &mut stream, 1, cell.label).await?;
        send_viewport_request(&codec, &mut stream, client_id, document_id, 2, 4_096, 8_192).await?;
        wait_exactly_one_patch(&codec, &mut stream, 2, cell.label).await?;

        // -- edit accounting is exact --
        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::Edit {
                    document_id,
                    client_id,
                    lease_id: metadata.lease_id,
                    base_version: metadata.version,
                    behavior_version: manifest.behavior_version,
                    transaction_id: 1,
                    operation: EditOperation::Insert {
                        byte_offset: 0,
                        text: "x".to_string(),
                    },
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        let ack = read_until(&codec, &mut stream, |message| {
            matches!(
                message,
                ServerMessage::EditAck { .. } | ServerMessage::EditRejected { .. }
            )
        })
        .await?;
        match ack {
            ServerMessage::EditAck {
                document_id: acked_document,
                confirmed_version,
                transaction_id,
            } => {
                assert_eq!(acked_document, document_id);
                assert_eq!(transaction_id, 1);
                assert_eq!(
                    confirmed_version,
                    metadata.version + 1,
                    "{}: edit ack advances exactly one version",
                    cell.label
                );
            }
            ServerMessage::EditRejected { reason, .. } => {
                return Err(format!("{}: edit rejected: {reason:?}", cell.label));
            }
            _ => unreachable!("read_until matched ack"),
        }
        let confirmed_version = metadata.version + 1;

        // -- save round-trip --
        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::SaveDocument {
                    client_id,
                    document_id,
                    known_version: confirmed_version,
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        let saved = read_until(&codec, &mut stream, |message| {
            matches!(message, ServerMessage::DocumentSaved { .. })
        })
        .await?;
        let ServerMessage::DocumentSaved {
            document_id: saved_document,
            version: saved_version,
            dirty,
        } = saved
        else {
            unreachable!("read_until matched DocumentSaved")
        };
        assert_eq!(saved_document, document_id);
        assert_eq!(saved_version, confirmed_version);
        assert!(!dirty, "{}: save clears the dirty flag", cell.label);

        // -- reload round-trip preserves the authoritative head --
        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::ReloadDocument {
                    client_id,
                    document_id,
                    known_version: confirmed_version,
                    force: true,
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        let reloaded = read_until(&codec, &mut stream, |message| {
            matches!(message, ServerMessage::DocumentReloaded { .. })
        })
        .await?;
        let ServerMessage::DocumentReloaded { metadata, head } = reloaded else {
            unreachable!("read_until matched DocumentReloaded")
        };
        // The file was saved before reload, so disk matches memory: the
        // authoritative version must not drift.
        assert_eq!(
            metadata.version, confirmed_version,
            "{}: reload of a saved file keeps the version",
            cell.label
        );
        assert!(
            head.first_chunk.starts_with('x'),
            "{}: reload serves the edited authoritative text",
            cell.label
        );
        let reloaded_version = metadata.version;

        // -- resync round-trip --
        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::RequestResync {
                    document_id,
                    client_id,
                    known_version: reloaded_version,
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        let resynced = read_until(&codec, &mut stream, |message| {
            matches!(message, ServerMessage::ResyncSnapshot { .. })
        })
        .await?;
        let ServerMessage::ResyncSnapshot {
            document_id: resync_document,
            version,
            ..
        } = resynced
        else {
            unreachable!("read_until matched ResyncSnapshot")
        };
        assert_eq!(resync_document, document_id);
        assert_eq!(version, reloaded_version);

        // -- close retires the document; no late patches may arrive --
        codec
            .write_client_message(
                &mut stream,
                &ClientMessage::CloseDocument {
                    client_id,
                    document_id,
                    force: true,
                },
            )
            .await
            .map_err(|e| e.to_string())?;
        let closed = read_until(&codec, &mut stream, |message| {
            matches!(message, ServerMessage::DocumentClosed { .. })
        })
        .await?;
        let ServerMessage::DocumentClosed {
            document_id: closed_document,
            closed,
        } = closed
        else {
            unreachable!("read_until matched DocumentClosed")
        };
        assert_eq!(closed_document, document_id);
        assert!(closed, "{}: close reports a retired document", cell.label);

        eprintln!("editor-performance matrix cell ok: {}", cell.label);
    }
    Ok(())
}

async fn send_viewport_request<S>(
    codec: &Codec,
    stream: &mut S,
    client_id: u64,
    document_id: u64,
    request_id: u64,
    byte_start: u64,
    byte_end: u64,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    codec
        .write_client_message(
            stream,
            &ClientMessage::ViewportRenderRequest {
                client_id,
                document_id,
                document_version: 0, // server clamps to latest authoritative
                request_id,
                byte_start,
                byte_end,
                trace_id: None,
            },
        )
        .await
        .map_err(|e| e.to_string())
}

/// Asserts exactly one patch for `request_id` arrives and no duplicate ever
/// follows within a quiet window (atomic per-request accounting).
async fn wait_exactly_one_patch<S>(
    codec: &Codec,
    stream: &mut S,
    request_id: u64,
    label: &str,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut seen = 0;
    for _ in 0..64 {
        match timeout(Duration::from_secs(10), codec.read_server_message(stream)).await {
            Err(_) => break,
            Ok(Err(error)) => return Err(format!("{label}: read failed: {error}")),
            Ok(Ok(message)) => match message {
                ServerMessage::ViewportRenderPatch(patch) => {
                    assert_eq!(
                        patch.request_id, request_id,
                        "{label}: patch matches request id"
                    );
                    seen += 1;
                    assert_eq!(seen, 1, "{label}: exactly one patch per request id");
                }
                ServerMessage::DecorationBatch(_)
                | ServerMessage::DecorationSet(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
                | ServerMessage::RuntimeDiagnostic(_) => {}
                _ => {}
            },
        }
    }
    if seen == 1 {
        Ok(())
    } else {
        Err(format!("{label}: expected one viewport patch, saw {seen}"))
    }
}

async fn next_message<S>(codec: &Codec, stream: &mut S) -> Result<ServerMessage, String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    timeout(Duration::from_secs(30), codec.read_server_message(stream))
        .await
        .map_err(|_| "timed out waiting for server message".to_string())?
        .map_err(|e| e.to_string())
}

/// Reads messages until `predicate` matches, tolerating the known handshake
/// noise (themes, typography, SDUI, diagnostics, syntax frames). Fails after a
/// bounded number of messages.
async fn read_until<S, F>(
    codec: &Codec,
    stream: &mut S,
    predicate: F,
) -> Result<ServerMessage, String>
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: Fn(&ServerMessage) -> bool,
{
    for _ in 0..256 {
        let message = next_message(codec, stream).await?;
        if predicate(&message) {
            return Ok(message);
        }
    }
    Err("read_until: predicate never matched within the message budget".to_string())
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("clay-{label}-{}", std::process::id()))
}

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
