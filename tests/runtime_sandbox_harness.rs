use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use clay::server::runtime_sandbox::{RuntimeSandboxError, RuntimeSandboxSupervisor};

fn sandbox_bin() -> &'static str {
    env!("CARGO_BIN_EXE_clay-runtime-sandbox")
}

#[cfg(target_os = "linux")]
fn hostile_sandbox_script(name: &str, bytes: usize, newline: bool) -> (PathBuf, PathBuf) {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!(
        "clay-runtime-sandbox-{name}-{}-{id}",
        std::process::id()
    ));
    let script = base.with_extension("sh");
    let pid_file = base.with_extension("pid");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' $$ > '{}'\nIFS= read -r _\nprintf '{{\"kind\":\"ready\",\"protocolVersion\":1}}\\n'\nIFS= read -r _\nprintf '%{bytes}s' '' | tr ' ' x\n{}sleep 10\n",
            pid_file.display(),
            if newline { "printf '\\n'\n" } else { "" }
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    (script, pid_file)
}

#[cfg(target_os = "linux")]
fn assert_child_reaped(pid_file: &Path) {
    let pid = fs::read_to_string(pid_file).unwrap();
    assert!(
        !Path::new("/proc").join(pid.trim()).exists(),
        "sandbox child {pid} was not reaped"
    );
}

#[tokio::test]
async fn sandbox_child_starts_and_evaluates_controlled_fixture() {
    let started = Instant::now();
    let mut supervisor = RuntimeSandboxSupervisor::spawn(sandbox_bin(), 16 * 1024)
        .await
        .expect("sandbox starts");
    let startup_elapsed = started.elapsed();

    let evaluation = supervisor
        .evaluate(
            "({ package: '@clay/test', ok: true, value: 2 + 2 })",
            Duration::from_secs(2),
        )
        .await
        .expect("fixture evaluates");

    assert_eq!(evaluation.value["value"], 4);
    assert!(startup_elapsed < Duration::from_secs(2));
    assert!(evaluation.elapsed < Duration::from_secs(2));
}

#[tokio::test]
async fn sandbox_timeout_kills_child_and_new_child_restarts() {
    let mut supervisor = RuntimeSandboxSupervisor::spawn(sandbox_bin(), 16 * 1024)
        .await
        .expect("sandbox starts");

    let error = supervisor
        .evaluate("for (;;) {}", Duration::from_millis(100))
        .await
        .expect_err("infinite loop times out");
    assert!(matches!(error, RuntimeSandboxError::Timeout));

    let mut replacement = RuntimeSandboxSupervisor::spawn(sandbox_bin(), 16 * 1024)
        .await
        .expect("replacement starts");
    let evaluation = replacement
        .evaluate("'fresh'", Duration::from_secs(2))
        .await
        .expect("replacement evaluates");
    assert_eq!(evaluation.value, "fresh");
}

#[tokio::test]
async fn sandbox_oversized_output_is_rejected_by_parent_budget() {
    let mut supervisor = RuntimeSandboxSupervisor::spawn(sandbox_bin(), 128)
        .await
        .expect("sandbox starts");

    let error = supervisor
        .evaluate("'x'.repeat(1000)", Duration::from_secs(2))
        .await
        .expect_err("large output rejected");

    assert!(matches!(error, RuntimeSandboxError::PayloadTooLarge { .. }));
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn sandbox_terminated_and_unterminated_overflow_are_bounded_and_reaped() {
    const MAX: usize = 128;
    for (name, newline) in [("terminated", true), ("unterminated", false)] {
        let (script, pid_file) = hostile_sandbox_script(name, MAX + 1, newline);
        let mut supervisor = RuntimeSandboxSupervisor::spawn(&script, MAX)
            .await
            .expect("hostile fixture handshakes");
        let started = Instant::now();
        let error = supervisor
            .evaluate("'ok'", Duration::from_secs(2))
            .await
            .expect_err("oversized child frame is rejected");

        assert!(matches!(
            error,
            RuntimeSandboxError::PayloadTooLarge { len, max } if len == MAX + 1 && max == MAX
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_child_reaped(&pid_file);
        let _ = fs::remove_file(script);
        let _ = fs::remove_file(pid_file);
    }
}

#[tokio::test]
async fn sandbox_protocol_exposes_no_filesystem_network_or_shell_authority() {
    let mut supervisor = RuntimeSandboxSupervisor::spawn(sandbox_bin(), 16 * 1024)
        .await
        .expect("sandbox starts");

    let evaluation = supervisor
        .evaluate(
            "({
                readTextFile: typeof Deno.readTextFile,
                command: typeof Deno.Command,
                fetch: typeof fetch,
                websocket: typeof WebSocket,
                process: typeof process,
                require: typeof require,
            })",
            Duration::from_secs(2),
        )
        .await
        .expect("authority probe evaluates");

    assert_eq!(
        evaluation.value,
        serde_json::json!({
            "readTextFile": "undefined",
            "command": "undefined",
            "fetch": "undefined",
            "websocket": "undefined",
            "process": "undefined",
            "require": "undefined",
        })
    );
}
