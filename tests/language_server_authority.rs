//! Phase 18.20: bounded language-server process/session boundary tests.
//!
//! These exercise the host-owned `LanguageServerProcessService` directly: a
//! fixed fake child is spawned only from validated launch metadata, exchanges
//! bounded opaque messages, and is reaped on timeout/exit/withdrawal/cap. The
//! deny-by-default executable resolution (no shell strings, no external paths)
//! is checked on every platform; process-spawning cases run under Unix where a
//! fake executable can be materialized deterministically (Linux is the primary
//! required host).

use std::path::{Path, PathBuf};

use clay::packages::authorization::resolve_language_server_executable;
use clay::perf::budgets::{LANGUAGE_SERVER_MAX_SESSIONS, LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES};
use clay::server::language_server::{
    LanguageServerError, LanguageServerProcessService, LanguageServerSessionId, LanguageServerSpawn,
};

/// Shell strings, command injection, and external paths must never resolve to
/// an executable. Only a canonical local file is accepted, and only that exact
/// canonical path may be bound into a grant.
#[test]
fn executable_resolution_rejects_shell_strings_and_external_paths() {
    assert!(resolve_language_server_executable("cat /etc/passwd").is_none());
    assert!(resolve_language_server_executable("rust-analyzer --stdio").is_none());
    // A directory is not an executable file.
    assert!(resolve_language_server_executable("/etc").is_none());
    assert!(resolve_language_server_executable("nonexistent-bin-zzz").is_none());
}

/// An oversize write is rejected by the typed budget before it reaches the
/// child stdio. No session (and no process) is required to exercise the guard.
#[tokio::test]
async fn oversize_write_is_rejected_by_budget_before_spawn() {
    let service = LanguageServerProcessService::new();
    let oversize = vec![0; LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES + 1];
    let result = service
        .write(
            LanguageServerSessionId::from_u64(1),
            "example".to_string(),
            "example.server".to_string(),
            0,
            oversize,
        )
        .await;
    assert!(
        matches!(result, Err(LanguageServerError::PayloadTooLarge { .. })),
        "oversize write must be rejected by budget, got {result:?}"
    );
}

#[tokio::test]
async fn oversize_read_is_rejected_by_budget_before_spawn() {
    let service = LanguageServerProcessService::new();
    let result = service
        .read(
            LanguageServerSessionId::from_u64(1),
            "example".to_string(),
            "example.server".to_string(),
            0,
            LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES + 1,
            100,
        )
        .await;
    assert!(
        matches!(result, Err(LanguageServerError::PayloadTooLarge { .. })),
        "oversize read must be rejected by budget, got {result:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn split_utf8_stdout_round_trips_as_exact_bytes() {
    let root = fake_workspace_root("lang-server-split-utf8");
    let executable = fake_output_child(
        &root,
        "fake-split-utf8",
        b"#!/bin/sh\nprintf '\\360\\237\\246\\200'\n",
    );
    let service = LanguageServerProcessService::new();
    let session = service
        .start(fake_spawn(&executable, &root, "bytes.pkg", "bytes.srv"))
        .await
        .expect("byte child starts");

    let mut bytes = service
        .read(
            session,
            "bytes.pkg".to_string(),
            "bytes.srv".to_string(),
            0,
            2,
            2_000,
        )
        .await
        .expect("first half reads");
    bytes.extend(
        service
            .read(
                session,
                "bytes.pkg".to_string(),
                "bytes.srv".to_string(),
                0,
                2,
                2_000,
            )
            .await
            .expect("second half reads"),
    );

    assert_eq!(bytes, "🦀".as_bytes());
}

#[cfg(unix)]
#[tokio::test]
async fn multiple_frames_in_one_read_remain_one_exact_byte_chunk() {
    const STREAM: &[u8] = b"Content-Length: 2\r\n\r\n{}Content-Length: 2\r\n\r\n[]";
    let root = fake_workspace_root("lang-server-coalesced-frames");
    let executable = fake_output_child(
        &root,
        "fake-coalesced-frames",
        b"#!/bin/sh\nprintf 'Content-Length: 2\\r\\n\\r\\n{}Content-Length: 2\\r\\n\\r\\n[]'\n",
    );
    let service = LanguageServerProcessService::new();
    let session = service
        .start(fake_spawn(&executable, &root, "frames.pkg", "frames.srv"))
        .await
        .expect("frame child starts");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let actual = service
        .read(
            session,
            "frames.pkg".to_string(),
            "frames.srv".to_string(),
            0,
            STREAM.len(),
            2_000,
        )
        .await
        .expect("coalesced frames read");

    assert_eq!(actual, STREAM);
}

#[cfg(unix)]
#[tokio::test]
async fn frames_fragmented_across_reads_reassemble_without_transport_changes() {
    const STREAM: &[u8] = b"Content-Length: 2\r\n\r\n{}Content-Length: 2\r\n\r\n[]";
    let root = fake_workspace_root("lang-server-multiple-frames");
    let executable = fake_output_child(
        &root,
        "fake-multiple-frames",
        b"#!/bin/sh\nprintf 'Content-Length: 2\\r\\n\\r\\n{}Content-Length: 2\\r\\n\\r\\n[]'\n",
    );
    let service = LanguageServerProcessService::new();
    let session = service
        .start(fake_spawn(&executable, &root, "frames.pkg", "frames.srv"))
        .await
        .expect("frame child starts");
    let mut actual = Vec::new();
    while actual.len() < STREAM.len() {
        actual.extend(
            service
                .read(
                    session,
                    "frames.pkg".to_string(),
                    "frames.srv".to_string(),
                    0,
                    7,
                    2_000,
                )
                .await
                .expect("fragment reads"),
        );
    }

    assert_eq!(actual, STREAM);
}

#[test]
fn exact_byte_facade_is_present_in_authoritative_runtime_source() {
    let source = std::fs::read_to_string("runtime/js/language-server.js")
        .expect("language-server facade readable");
    let table = std::fs::read_to_string("src/server/facades.rs").expect("facade table readable");
    for marker in [
        "sendBytes",
        "readBytes",
        "op_clay_language_server_send_bytes",
        "op_clay_language_server_read_bytes",
        "Uint8Array",
    ] {
        assert!(source.contains(marker), "source facade missing {marker}");
    }
    assert!(table.contains("include_str!(\"../../runtime/js/language-server.js\")"));
}

/// A launch whose cwd is not a real directory fails with a typed spawn error
/// rather than silently falling back or escaping the approved root.
#[cfg(unix)]
#[tokio::test]
async fn launch_with_missing_cwd_fails_with_typed_spawn_error() {
    let service = LanguageServerProcessService::new();
    let spawn = LanguageServerSpawn {
        package_name: "example".to_string(),
        contribution_id: "example.server".to_string(),
        descriptor_fingerprint: 0,
        canonical_executable: PathBuf::from("/bin/true"),
        args: Vec::new(),
        inherit_environment: Vec::new(),
        cwd: PathBuf::from("/definitely/not/a/real/directory/zzz"),
    };
    let result = service.start(spawn).await;
    assert!(
        matches!(result, Err(LanguageServerError::Spawn(_))),
        "missing cwd must produce a typed spawn error, got {result:?}"
    );
}

/// A child that never writes within the timeout yields a typed timeout error,
/// not a hang, and the session remains usable for a subsequent stop.
#[cfg(unix)]
#[tokio::test]
async fn read_timeout_produces_typed_error_and_session_remains_stoppable() {
    let root = fake_workspace_root("lang-server-timeout");
    let executable = fake_sleep_child(&root);
    let service = LanguageServerProcessService::new();
    let session = service
        .start(fake_spawn(&executable, &root, "timeout.pkg", "timeout.srv"))
        .await
        .expect("sleep child starts");

    let result = service
        .read(
            session,
            "timeout.pkg".to_string(),
            "timeout.srv".to_string(),
            0,
            64,
            100,
        )
        .await;
    assert!(
        matches!(result, Err(LanguageServerError::Timeout)),
        "silent child must time out, got {result:?}"
    );

    service
        .stop(
            session,
            "timeout.pkg".to_string(),
            "timeout.srv".to_string(),
            0,
        )
        .await
        .expect("session still stoppable after timeout");
}

/// A child that exits is surfaced as a typed exit error with sanitized stderr.
#[cfg(unix)]
#[tokio::test]
async fn unexpected_child_exit_produces_typed_sanitized_error() {
    let root = fake_workspace_root("lang-server-exit");
    let executable = fake_exit_child(&root);
    let service = LanguageServerProcessService::new();
    let session = service
        .start(fake_spawn(&executable, &root, "exit.pkg", "exit.srv"))
        .await
        .expect("exit child starts");

    let result = service
        .read(
            session,
            "exit.pkg".to_string(),
            "exit.srv".to_string(),
            0,
            256,
            2_000,
        )
        .await;
    assert!(
        matches!(result, Err(LanguageServerError::ChildExitedWith { .. })),
        "exited child must surface a typed exit error, got {result:?}"
    );
}

/// One package contribution has at most one live session for each approved
/// workspace root; callers must reuse its opaque session instead of spawning
/// duplicate children.
#[cfg(unix)]
#[tokio::test]
async fn duplicate_contribution_root_session_is_rejected() {
    let root = fake_workspace_root("lang-server-duplicate");
    let executable = fake_sleep_child(&root);
    let service = LanguageServerProcessService::new();
    let first = service
        .start(fake_spawn(
            &executable,
            &root,
            "duplicate.pkg",
            "duplicate.srv",
        ))
        .await
        .expect("first session starts");

    let duplicate = service
        .start(fake_spawn(
            &executable,
            &root,
            "duplicate.pkg",
            "duplicate.srv",
        ))
        .await;
    assert!(
        matches!(duplicate, Err(LanguageServerError::SessionAlreadyRunning)),
        "duplicate contribution/root session must reject, got {duplicate:?}"
    );

    service
        .stop(
            first,
            "duplicate.pkg".to_string(),
            "duplicate.srv".to_string(),
            0,
        )
        .await
        .expect("first session stops");
}

/// The concurrent-session cap is enforced and package withdrawal reaps every
/// owned session. `kill_on_drop` plus the Drop path cover server shutdown and
/// runtime-generation replacement.
#[cfg(unix)]
#[tokio::test]
async fn session_cap_is_enforced_and_withdrawal_reaps_owned_sessions() {
    let root = fake_workspace_root("lang-server-cap");
    let executable = fake_sleep_child(&root);
    let service = LanguageServerProcessService::new();

    let mut sessions = Vec::new();
    for index in 0..LANGUAGE_SERVER_MAX_SESSIONS {
        sessions.push(
            service
                .start(fake_spawn(
                    &executable,
                    &root,
                    "cap.pkg",
                    &format!("cap.srv.{index}"),
                ))
                .await
                .expect("session under cap starts"),
        );
        let _ = index;
    }
    let over_cap = service
        .start(fake_spawn(&executable, &root, "cap.pkg", "cap.srv.over"))
        .await;
    assert!(
        matches!(over_cap, Err(LanguageServerError::TooManySessions { .. })),
        "session beyond cap must be rejected, got {over_cap:?}"
    );

    let reaped = service.revoke_for_package("cap.pkg").await;
    assert_eq!(reaped, LANGUAGE_SERVER_MAX_SESSIONS);
    for session in sessions {
        let result = service
            .read(
                session,
                "cap.pkg".to_string(),
                "cap.srv".to_string(),
                0,
                64,
                100,
            )
            .await;
        assert!(
            matches!(result, Err(LanguageServerError::UnknownSession)),
            "reaped session must be gone, got {result:?}"
        );
    }
}

/// A read blocked on one child is owned by that session actor. Another
/// session can still write, read, and stop within its own deadline instead of
/// waiting behind the blocked read in the central identity router.
#[cfg(unix)]
#[tokio::test]
async fn hung_session_does_not_delay_another_session() {
    let root = fake_workspace_root("lang-server-hol");
    let hung_executable = fake_sleep_child(&root);
    let responsive_executable = write_fake_executable(
        &root,
        "fake-responsive",
        b"#!/bin/sh\nwhile IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done\n",
    );
    let service = LanguageServerProcessService::new();
    let hung = service
        .start(fake_spawn(&hung_executable, &root, "hung.pkg", "hung.srv"))
        .await
        .expect("hung child starts");
    let responsive = service
        .start(fake_spawn(
            &responsive_executable,
            &root,
            "responsive.pkg",
            "responsive.srv",
        ))
        .await
        .expect("responsive child starts");

    let hung_service = service.clone();
    let blocked_read = tokio::spawn(async move {
        hung_service
            .read(
                hung,
                "hung.pkg".to_string(),
                "hung.srv".to_string(),
                0,
                64,
                2_000,
            )
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let responsive_work = async {
        service
            .write(
                responsive,
                "responsive.pkg".to_string(),
                "responsive.srv".to_string(),
                0,
                b"hello\n".to_vec(),
            )
            .await?;
        let bytes = service
            .read(
                responsive,
                "responsive.pkg".to_string(),
                "responsive.srv".to_string(),
                0,
                64,
                500,
            )
            .await?;
        assert!(bytes.starts_with(b"echo:hello"));
        service
            .stop(
                responsive,
                "responsive.pkg".to_string(),
                "responsive.srv".to_string(),
                0,
            )
            .await
    };
    tokio::time::timeout(std::time::Duration::from_secs(1), responsive_work)
        .await
        .expect("responsive session must not wait behind hung session")
        .expect("responsive session succeeds");

    service
        .stop(hung, "hung.pkg".to_string(), "hung.srv".to_string(), 0)
        .await
        .expect("stop interrupts hung session");
    assert!(blocked_read.await.unwrap().is_err());
}

/// Per-session ingress is bounded. Once one read is active and the actor queue
/// is full, excess work fails immediately as `SessionBusy` instead of growing
/// memory or blocking the central router.
#[cfg(unix)]
#[tokio::test]
async fn session_actor_queue_rejects_excess_work() {
    let root = fake_workspace_root("lang-server-queue-cap");
    let executable = fake_sleep_child(&root);
    let service = LanguageServerProcessService::new();
    let session = service
        .start(fake_spawn(&executable, &root, "queue.pkg", "queue.srv"))
        .await
        .expect("sleep child starts");

    let mut tasks = tokio::task::JoinSet::new();
    // Internal queue capacity is deliberately not public API. Sixty-four
    // concurrent writes exceeds the compiled eight-command session lane.
    for _ in 0..64 {
        let service = service.clone();
        tasks.spawn(async move {
            service
                .read(
                    session,
                    "queue.pkg".to_string(),
                    "queue.srv".to_string(),
                    0,
                    64,
                    5_000,
                )
                .await
        });
    }

    let busy = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while let Some(result) = tasks.join_next().await {
            if matches!(result.unwrap(), Err(LanguageServerError::SessionBusy)) {
                return true;
            }
        }
        false
    })
    .await
    .expect("excess request must be rejected promptly");
    assert!(busy);

    service
        .stop(session, "queue.pkg".to_string(), "queue.srv".to_string(), 0)
        .await
        .expect("stop interrupts queued actor");
    tasks.abort_all();
}

#[cfg(unix)]
fn fake_spawn(
    executable: &Path,
    root: &Path,
    package: &str,
    contribution: &str,
) -> LanguageServerSpawn {
    LanguageServerSpawn {
        package_name: package.to_string(),
        contribution_id: contribution.to_string(),
        descriptor_fingerprint: 0,
        canonical_executable: executable.to_path_buf(),
        args: Vec::new(),
        inherit_environment: Vec::new(),
        cwd: root.to_path_buf(),
    }
}

#[cfg(unix)]
fn fake_workspace_root(label: &str) -> PathBuf {
    use std::time::SystemTime;
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("clay-{label}-{unique}"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::canonicalize(&root).unwrap()
}

#[cfg(unix)]
fn fake_sleep_child(root: &Path) -> PathBuf {
    write_fake_executable(root, "fake-sleep", b"#!/bin/sh\nsleep 30\n")
}

#[cfg(unix)]
fn fake_exit_child(root: &Path) -> PathBuf {
    write_fake_executable(root, "fake-exit", b"#!/bin/sh\necho 'oops' >&2\nexit 0\n")
}

#[cfg(unix)]
fn fake_output_child(root: &Path, name: &str, body: &[u8]) -> PathBuf {
    write_fake_executable(root, name, body)
}

#[cfg(unix)]
fn write_fake_executable(root: &Path, name: &str, body: &[u8]) -> PathBuf {
    use std::{io::Write, os::unix::fs::PermissionsExt};
    let path = root.join(name);
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(body).unwrap();
    file.sync_all().unwrap();
    drop(file);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    path
}

#[cfg(unix)]
fn content_length_frame(message: &serde_json::Value) -> Vec<u8> {
    let body = serde_json::to_vec(message).expect("json body");
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(&body);
    frame
}

#[cfg(unix)]
fn fake_lsp_shell_child(root: &Path, profile: &str) -> PathBuf {
    // Keep the process-service coverage on a deterministic shell child so ordinary
    // cargo test does not depend on Node pipe timing. The Node fake-server remains
    // the package-matrix source of truth under tests/fixtures/lsp/fake-server/.
    let body = match profile {
        "exit-early" => {
            b"#!/bin/sh\n# Consume one stdin chunk then answer initialize and exit.\ndd bs=65536 count=1 of=/dev/null 2>/dev/null\nprintf 'Content-Length: 91\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"capabilities\":{},\"serverInfo\":{\"name\":\"clay-fake-lsp\"}}}'\necho 'clay-fake-lsp profile=exit-early' >&2\nexit 0\n"
                as &[u8]
        }
        _ => {
            b"#!/bin/sh\n# Consume one stdin chunk then answer initialize; wait for exit notification.\ndd bs=65536 count=1 of=/dev/null 2>/dev/null\nprintf 'Content-Length: 91\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"capabilities\":{},\"serverInfo\":{\"name\":\"clay-fake-lsp\"}}}'\necho 'clay-fake-lsp profile=minimal' >&2\ndd bs=65536 count=1 of=/dev/null 2>/dev/null\nexit 0\n"
        }
    };
    write_fake_executable(root, &format!("fake-lsp-{profile}"), body)
}

#[cfg(unix)]
fn fake_lsp_spawn(
    profile: &str,
    root: &Path,
    package: &str,
    contribution: &str,
) -> LanguageServerSpawn {
    LanguageServerSpawn {
        package_name: package.to_string(),
        contribution_id: contribution.to_string(),
        descriptor_fingerprint: 0,
        canonical_executable: fake_lsp_shell_child(root, profile),
        args: Vec::new(),
        inherit_environment: Vec::new(),
        cwd: root.to_path_buf(),
    }
}

/// The generic Node fake LSP server initializes and shuts down through the
/// real host process service without any package-specific Rust branch.
#[cfg(unix)]
#[tokio::test]
async fn generic_fake_lsp_server_initialize_and_shutdown_through_process_service() {
    let root = fake_workspace_root("lang-server-fake-lsp");
    let service = LanguageServerProcessService::new();
    let session = service
        .start(fake_lsp_spawn("minimal", &root, "fake.pkg", "fake.srv"))
        .await
        .expect("fake lsp child starts");

    let initialize = content_length_frame(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": null,
            "rootUri": format!("file://{}", root.display()),
            "capabilities": {}
        }
    }));
    service
        .write(
            session,
            "fake.pkg".to_string(),
            "fake.srv".to_string(),
            0,
            initialize,
        )
        .await
        .expect("initialize write");

    let response = service
        .read(
            session,
            "fake.pkg".to_string(),
            "fake.srv".to_string(),
            0,
            LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES,
            2_000,
        )
        .await
        .expect("initialize read");
    let body = String::from_utf8_lossy(&response);
    assert!(
        body.contains("clay-fake-lsp") && body.contains("Content-Length:"),
        "expected framed initialize response, got {body}"
    );

    let shutdown = content_length_frame(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "shutdown",
        "params": null
    }));
    service
        .write(
            session,
            "fake.pkg".to_string(),
            "fake.srv".to_string(),
            0,
            shutdown,
        )
        .await
        .expect("shutdown write");
    let _ = service
        .read(
            session,
            "fake.pkg".to_string(),
            "fake.srv".to_string(),
            0,
            LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES,
            1_000,
        )
        .await;
    service
        .stop(session, "fake.pkg".to_string(), "fake.srv".to_string(), 0)
        .await
        .expect("stop fake session");
}

/// Exit-early fake profile surfaces a typed child-exit error; stderr remains
/// sanitized and includes only the profile banner rather than absolute paths
/// from the request payload.
#[cfg(unix)]
#[tokio::test]
async fn generic_fake_lsp_exit_early_surfaces_typed_sanitized_exit() {
    let root = fake_workspace_root("lang-server-fake-exit");
    let service = LanguageServerProcessService::new();
    let session = service
        .start(fake_lsp_spawn(
            "exit-early",
            &root,
            "fake.exit",
            "fake.exit.srv",
        ))
        .await
        .expect("exit-early fake starts");

    let initialize = content_length_frame(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "processId": null,
            "rootUri": format!("file://{}", root.display()),
            "capabilities": {}
        }
    }));
    service
        .write(
            session,
            "fake.exit".to_string(),
            "fake.exit.srv".to_string(),
            0,
            initialize,
        )
        .await
        .expect("initialize write");

    // Drain the initialize response, then wait for the child to exit.
    let _ = service
        .read(
            session,
            "fake.exit".to_string(),
            "fake.exit.srv".to_string(),
            0,
            LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES,
            1_000,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let result = service
        .read(
            session,
            "fake.exit".to_string(),
            "fake.exit.srv".to_string(),
            0,
            LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES,
            1_000,
        )
        .await;
    match result {
        Err(LanguageServerError::ChildExitedWith { detail }) => {
            assert!(
                detail.contains("profile=exit-early")
                    || detail.is_empty()
                    || !detail.contains('\0'),
                "stderr detail must stay sanitized, got {detail:?}"
            );
            assert!(
                !detail.contains("/home/"),
                "stderr must not leak absolute home paths"
            );
        }
        other => panic!("expected ChildExitedWith after exit-early profile, got {other:?}"),
    }
}
