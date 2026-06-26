use std::time::Duration;

use clay::server::runtime_sandbox::{RuntimeSandboxError, RuntimeSandboxSupervisor};

fn sandbox_bin() -> &'static str {
    env!("CARGO_BIN_EXE_clay-runtime-sandbox")
}

#[tokio::test]
async fn sandbox_child_starts_and_evaluates_controlled_fixture() {
    let mut supervisor = RuntimeSandboxSupervisor::spawn(sandbox_bin(), 16 * 1024)
        .await
        .expect("sandbox starts");

    let evaluation = supervisor
        .evaluate(
            "({ package: '@clay/test', ok: true, value: 2 + 2 })",
            Duration::from_secs(2),
        )
        .await
        .expect("fixture evaluates");

    assert_eq!(evaluation.value["value"], 4);
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
