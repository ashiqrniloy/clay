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
    let oversize = "x".repeat(LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES + 1);
    let result = service
        .write(
            LanguageServerSessionId::from_u64(1),
            "example".to_string(),
            "example.server".to_string(),
            0,
            oversize.into_bytes(),
        )
        .await;
    assert!(
        matches!(result, Err(LanguageServerError::PayloadTooLarge { .. })),
        "oversize write must be rejected by budget, got {result:?}"
    );
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
        .stop(session)
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

    service.stop(first).await.expect("first session stops");
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
