#![cfg(any(unix, windows))]

//! Phase 19 hot-reload end-to-end coverage through the public developer trigger.
//!
//! Deeper duplex/barrier scenarios live in `server::runtime_generation_tests`:
//! `typing_and_edit_ack_continue_while_candidate_runtime_is_blocked_on_test_barrier`,
//! `failed_reload_broadcasts_diagnostic_but_no_generation_snapshot`,
//! `successful_reload_is_observed_as_one_generation_by_all_clients`, and
//! `reload_preserves_authority_denials_and_cleans_old_lsp_worker`.

use std::{fs, path::PathBuf, time::SystemTime};

use clay::{
    ipc::smoke_endpoint,
    server::{IpcServer, ServerConfig},
};

#[tokio::test]
async fn developer_hot_reload_trigger_reports_success_and_sanitized_failure() {
    let root = temp_config_root(
        "developer-trigger",
        r#"Deno.core.ops.op_clay_runtime_record("reload ok");"#,
    );
    let mut config = ServerConfig::new(smoke_endpoint("developer-hot-reload-trigger"));
    config.configuration_root = Some(root.clone());
    let server = IpcServer::try_new(config).expect("test server config is valid");

    let loaded = server.trigger_developer_hot_reload().await;
    assert!(loaded.reloaded);
    assert_eq!(loaded.previous_generation_id, 1);
    assert_eq!(loaded.active_generation_id, 2);
    assert!(loaded.diagnostics.is_empty());

    fs::write(root.join("init.js"), "export const = ;").unwrap();
    let failed = server.trigger_developer_hot_reload().await;
    assert!(!failed.reloaded);
    assert_eq!(failed.previous_generation_id, 2);
    assert_eq!(failed.active_generation_id, 2);
    assert!(failed.refreshed_documents.is_empty());
    let diagnostic = failed.diagnostics.last().unwrap();
    assert_eq!(diagnostic.code, "clay.runtime.syntax_error");
    assert!(
        !diagnostic
            .message
            .contains(&root.to_string_lossy().to_string())
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn developer_hot_reload_keeps_runtime_authority_denials_after_success() {
    let root = temp_config_root(
        "authority-denial",
        r#"Deno.core.ops.op_clay_runtime_record("reload ok");"#,
    );
    let mut config = ServerConfig::new(smoke_endpoint("developer-hot-reload-authority-denial"));
    config.configuration_root = Some(root.clone());
    let server = IpcServer::try_new(config).expect("test server config is valid");

    let loaded = server.trigger_developer_hot_reload().await;
    assert!(loaded.reloaded);
    assert_eq!(loaded.active_generation_id, 2);

    fs::write(
        root.join("init.js"),
        r#"import "https://example.com/not-allowed.js";"#,
    )
    .unwrap();
    let denied = server.trigger_developer_hot_reload().await;
    assert!(!denied.reloaded);
    assert_eq!(denied.previous_generation_id, 2);
    assert_eq!(denied.active_generation_id, 2);
    let diagnostic = denied.diagnostics.last().unwrap();
    assert_eq!(diagnostic.code, "clay.configuration.invalid_module");
    assert!(
        !diagnostic
            .message
            .contains("https://example.com/not-allowed.js")
    );

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn failed_reload_keeps_generation_and_sanitized_diagnostic_without_advancing() {
    let root = temp_config_root(
        "failed-keeps-generation",
        r#"Deno.core.ops.op_clay_runtime_record("baseline");"#,
    );
    let mut config = ServerConfig::new(smoke_endpoint("developer-hot-reload-failed-keeps"));
    config.configuration_root = Some(root.clone());
    let server = IpcServer::try_new(config).expect("test server config is valid");

    let loaded = server.trigger_developer_hot_reload().await;
    assert!(loaded.reloaded);
    assert_eq!(loaded.active_generation_id, 2);

    fs::write(root.join("init.js"), "export const = ;").unwrap();
    let failed = server.trigger_developer_hot_reload().await;
    assert!(!failed.reloaded);
    assert_eq!(failed.previous_generation_id, 2);
    assert_eq!(failed.active_generation_id, 2);
    assert!(failed.refreshed_documents.is_empty());
    let diagnostic = failed.diagnostics.last().unwrap();
    assert_eq!(diagnostic.code, "clay.runtime.syntax_error");
    assert!(
        !diagnostic
            .message
            .contains(&root.to_string_lossy().to_string())
    );

    let _ = fs::remove_dir_all(root);
}

fn temp_config_root(name: &str, init_js: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "clay-persistent-runtime-hot-reload-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("init.js"), init_js).unwrap();
    root
}
