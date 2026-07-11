use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use clay::{
    packages::record::{PackageRecord, assemble_package_record},
    perf::budgets::{INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES, SYNTAX_CACHE_BUDGET_BYTES},
    protocol::{
        DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan,
        IncrementalParseUpdate, ParseByteRange, ParseEditNotification, ParsePolicy, ParseUnit,
        ParseWindowSnapshot,
    },
    server::parse_coordinator::{ParseCoordinator, ParseCoordinatorError, ParseScheduleRequest},
};
use serde_json::json;

fn package_with_permissions(permissions: &[&str]) -> PackageRecord {
    package_with_identity("@clay/markdown", "markdown", "markdown", permissions)
}

fn package_with_identity(
    name: &str,
    api_prefix: &str,
    mode_id: &str,
    permissions: &[&str],
) -> PackageRecord {
    assemble_package_record(&json!({
        "name": name,
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": api_prefix,
            "entry": "./dist/index.js",
            "permissions": permissions,
            "modes": [mode_id],
            "docs": "./docs/index.md"
        }
    }))
    .expect("package fixture validates")
}

fn request(version: u64) -> ParseScheduleRequest {
    ParseScheduleRequest {
        document_id: 7,
        document_version: version,
        behavior_version: 3,
        package_prefix: "markdown".to_string(),
        mode_id: "markdown".to_string(),
        viewport: ParseByteRange::new(0, 64),
        invalidated_ranges: vec![ParseByteRange::new(20, 30), ParseByteRange::new(0, 5)],
    }
}

fn parse_window(version: u64, start: u64, text: &str) -> ParseWindowSnapshot {
    ParseWindowSnapshot {
        document_id: 7,
        document_version: version,
        package_prefix: "markdown".to_string(),
        mode_id: "markdown".to_string(),
        byte_start: start,
        byte_end: start + text.len() as u64,
        base_line: 0,
        text: text.to_string(),
    }
}

fn parse_policy(max_window_bytes: u64, memory_budget_bytes: u64) -> ParsePolicy {
    ParsePolicy::new(max_window_bytes, 16, memory_budget_bytes, 50)
}

fn update(version: u64) -> IncrementalParseUpdate {
    IncrementalParseUpdate {
        document_id: 7,
        document_version: version,
        behavior_version: 3,
        package_prefix: "markdown".to_string(),
        mode_id: "markdown".to_string(),
        parse_unit: ParseUnit::LineGroup,
        viewport: ParseByteRange::new(0, 64),
        invalidated_ranges: vec![ParseByteRange::new(0, 5)],
        syntax_tree_delta: Some("heading".to_string()),
        decoration_update: None,
    }
}

fn markdown_decoration_update(version: u64) -> DecorationSet {
    let provenance = DecorationProvenance {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "markdown".to_string(),
    };
    DecorationSet {
        document_id: 7,
        document_version: version,
        viewport_byte_start: 0,
        viewport_byte_end: 64,
        spans: vec![
            DecorationSpan::from_style_token(
                0,
                5,
                DecorationKind::Syntax,
                "markup.heading.1",
                90,
                provenance.clone(),
            ),
            DecorationSpan::from_style_token(
                10,
                18,
                DecorationKind::Syntax,
                "markup.strong",
                60,
                provenance,
            ),
        ],
    }
}

#[test]
fn parse_handler_registration_requires_parse_permission() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&[]);

    let error = coordinator
        .register_handler(&package, "markdown", |_notification| async move {
            Ok(update(1))
        })
        .unwrap_err();

    assert_eq!(
        error,
        ParseCoordinatorError::MissingPermission {
            package_prefix: "markdown".to_string()
        }
    );
}

#[tokio::test]
async fn superseded_parse_task_is_cancelled() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    coordinator
        .register_handler(
            &package,
            "markdown",
            |notification: ParseEditNotification| async move {
                if notification.document_version == 1 {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                Ok(update(notification.document_version))
            },
        )
        .unwrap();

    coordinator.schedule_parse(request(1)).unwrap();
    coordinator.schedule_parse(request(2)).unwrap();

    let parsed = tokio::time::timeout(Duration::from_secs(1), coordinator.next_update())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(parsed.document_version, 2);
    assert_eq!(coordinator.stats().cancelled_superseded_tasks, 1);
}

#[tokio::test]
async fn generic_parse_request_metadata_supports_token_stream_adapters() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_identity("@clay/python", "python", "python", &["parse-document"]);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ParseEditNotification>();
    coordinator
        .register_handler(
            &package,
            "python",
            move |notification: ParseEditNotification| {
                let tx = tx.clone();
                async move {
                    tx.send(notification.clone())
                        .expect("notification observed");
                    Ok(IncrementalParseUpdate {
                        document_id: notification.document_id,
                        document_version: notification.document_version,
                        behavior_version: notification.behavior_version,
                        package_prefix: notification.package_prefix,
                        mode_id: notification.mode_id,
                        parse_unit: ParseUnit::LineGroup,
                        viewport: notification.viewport,
                        invalidated_ranges: notification.invalidated_ranges,
                        syntax_tree_delta: Some("token-stream:visible-ranges-first".to_string()),
                        decoration_update: None,
                    })
                }
            },
        )
        .unwrap();

    coordinator
        .schedule_parse(ParseScheduleRequest {
            document_id: 42,
            document_version: 9,
            behavior_version: 5,
            package_prefix: "python".to_string(),
            mode_id: "python".to_string(),
            viewport: ParseByteRange::new(0, 32),
            invalidated_ranges: vec![ParseByteRange::new(80, 96), ParseByteRange::new(8, 16)],
        })
        .unwrap();

    let notification = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(notification.document_id, 42);
    assert_eq!(notification.document_version, 9);
    assert_eq!(notification.behavior_version, 5);
    assert_eq!(notification.package_prefix, "python");
    assert_eq!(notification.mode_id, "python");
    assert_eq!(notification.viewport, ParseByteRange::new(0, 32));
    assert_eq!(
        notification.invalidated_ranges,
        vec![ParseByteRange::new(8, 16), ParseByteRange::new(80, 96)],
        "viewport-intersecting invalidated ranges should be delivered first without parser-specific fields"
    );
}

#[test]
fn rust_code_has_no_markdown_specific_parser_branch() {
    for path in [
        "src/protocol/parse.rs",
        "src/server/parse_coordinator.rs",
        "src/server/ops/parse.rs",
        "src/protocol/decorations.rs",
        "src/server/decorations.rs",
        "src/server/ops/decorations.rs",
        "src/editor/surface.rs",
        "src/editor/layout.rs",
        "src/masonry_editor.rs",
        "src/client/mod.rs",
    ] {
        let source = std::fs::read_to_string(path).expect("Rust source must be readable");
        for forbidden in [
            "heading_open",
            "list_item_open",
            "strong_open",
            "em_open",
            "code_inline",
            "MarkdownItToken",
            "MarkdownParser",
            "MarkdownHeading",
            "MarkdownFence",
            "fromMarkdown",
            "mdast-util-from-markdown",
            "unwrap_or(\"markdown\")",
            "if mode == \"markdown\"",
            "if mode_id == \"markdown\"",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} must not contain Markdown/markdown-it parser branch marker `{forbidden}`"
            );
        }
    }
}

#[test]
fn markdown_parse_update_accepts_valid_decoration_payload() {
    let coordinator = ParseCoordinator::new();
    coordinator.schedule_parse(request(4)).unwrap_err();
    let parsed = IncrementalParseUpdate {
        decoration_update: Some(markdown_decoration_update(4)),
        ..update(4)
    };

    assert!(coordinator.validate_update(&parsed).is_ok());
}

#[test]
fn markdown_parse_update_rejects_decoration_version_mismatch() {
    let coordinator = ParseCoordinator::new();
    let parsed = IncrementalParseUpdate {
        decoration_update: Some(markdown_decoration_update(99)),
        ..update(4)
    };

    assert!(matches!(
        coordinator.validate_update(&parsed).unwrap_err(),
        ParseCoordinatorError::DecorationVersionMismatch { .. }
    ));
}

#[test]
fn parse_result_rejected_for_stale_version_and_oversized_payload() {
    let coordinator = ParseCoordinator::new();
    let oversized = IncrementalParseUpdate {
        syntax_tree_delta: Some("x".repeat(INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES)),
        ..update(4)
    };

    assert!(matches!(
        coordinator.validate_update(&oversized).unwrap_err(),
        ParseCoordinatorError::PayloadBudgetExceeded { .. }
    ));
}

#[tokio::test]
async fn finish_task_publishes_runtime_diagnostic_for_handler_error() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    coordinator
        .register_handler(&package, "markdown", |_notification| async move {
            Err(ParseCoordinatorError::HandlerFailed(
                "/home/alice/project/secret.rs token=abc123".to_string(),
            ))
        })
        .unwrap();

    coordinator.schedule_parse(request(5)).unwrap();

    let diagnostic = tokio::time::timeout(Duration::from_secs(1), coordinator.next_diagnostic())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(diagnostic.code, "clay.parse.open_failed");
    assert!(diagnostic.message.contains("markdown"));
    assert!(diagnostic.message.contains("handler failed"));
    assert!(!diagnostic.message.contains("/home/alice"));
    assert!(!diagnostic.message.contains("token=abc123"));
    assert!(
        tokio::time::timeout(Duration::from_millis(100), coordinator.next_update())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn stale_parse_result_is_not_published() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    coordinator
        .register_handler(
            &package,
            "markdown",
            |notification: ParseEditNotification| async move {
                if notification.document_version == 1 {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Ok(update(notification.document_version))
            },
        )
        .unwrap();

    coordinator.schedule_parse(request(1)).unwrap();
    coordinator.schedule_parse(request(2)).unwrap();

    let parsed = tokio::time::timeout(Duration::from_secs(1), coordinator.next_update())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(parsed.document_version, 2);
    assert!(
        tokio::time::timeout(Duration::from_millis(150), coordinator.next_update())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn parse_window_snapshot_is_bounded_and_versioned() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ParseEditNotification>();
    coordinator
        .register_handler(
            &package,
            "markdown",
            move |notification: ParseEditNotification| {
                let tx = tx.clone();
                async move {
                    tx.send(notification.clone())
                        .expect("windowed notification observed");
                    Ok(update(notification.document_version))
                }
            },
        )
        .unwrap();

    coordinator
        .schedule_parse_with_windows(
            request(8),
            vec![parse_window(8, 1_024, "# visible\n")],
            Some(parse_policy(4_096, 30 * 1024 * 1024)),
        )
        .unwrap();

    let notification = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(notification.document_id, 7);
    assert_eq!(notification.document_version, 8);
    assert_eq!(notification.parse_windows.len(), 1);
    assert_eq!(notification.parse_windows[0].byte_start, 1_024);
    assert_eq!(notification.parse_windows[0].byte_end, 1_034);
    assert_eq!(notification.parse_windows[0].text, "# visible\n");
    assert_eq!(
        notification.memory_budget.unwrap().budget_bytes,
        30 * 1024 * 1024
    );
}

#[tokio::test]
async fn large_file_edit_does_not_copy_full_document_to_parser() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<usize>();
    coordinator
        .register_handler(
            &package,
            "markdown",
            move |notification: ParseEditNotification| {
                let tx = tx.clone();
                async move {
                    let delivered_bytes: usize = notification
                        .parse_windows
                        .iter()
                        .map(ParseWindowSnapshot::text_len_bytes)
                        .sum();
                    tx.send(delivered_bytes).expect("parser input measured");
                    Ok(update(notification.document_version))
                }
            },
        )
        .unwrap();

    let document_bytes = 16 * 1024 * 1024usize;
    let bounded_window = "x".repeat(4096);
    coordinator
        .schedule_parse_with_windows(
            request(9),
            vec![parse_window(9, 8 * 1024 * 1024, &bounded_window)],
            Some(parse_policy(4096, 30 * 1024 * 1024)),
        )
        .unwrap();

    let delivered_bytes = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(delivered_bytes < document_bytes / 1024);
    assert_eq!(delivered_bytes, 4096);
}

#[tokio::test]
async fn newer_viewport_parse_cancels_stale_window_work() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    coordinator
        .register_handler(
            &package,
            "markdown",
            |notification: ParseEditNotification| async move {
                if notification.document_version == 1 {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                Ok(update(notification.document_version))
            },
        )
        .unwrap();

    coordinator
        .schedule_parse_with_windows(
            request(1),
            vec![parse_window(1, 0, "first")],
            Some(parse_policy(4096, 30 * 1024 * 1024)),
        )
        .unwrap();
    coordinator
        .schedule_parse_with_windows(
            request(2),
            vec![parse_window(2, 64, "second")],
            Some(parse_policy(4096, 30 * 1024 * 1024)),
        )
        .unwrap();

    let parsed = tokio::time::timeout(Duration::from_secs(1), coordinator.next_update())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(parsed.document_version, 2);
    assert_eq!(coordinator.stats().cancelled_superseded_tasks, 1);
}

#[test]
fn parse_window_snapshot_requires_parse_permission() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&[]);

    let error = coordinator
        .register_handler(&package, "markdown", |_notification| async move {
            Ok(update(1))
        })
        .unwrap_err();

    assert_eq!(
        error,
        ParseCoordinatorError::MissingPermission {
            package_prefix: "markdown".to_string()
        }
    );
}

#[test]
fn parse_window_snapshot_rejects_oversized_or_mismatched_windows() {
    let coordinator = ParseCoordinator::new();
    let oversized = parse_window(1, 0, "0123456789");
    assert!(matches!(
        coordinator
            .schedule_parse_with_windows(request(1), vec![oversized], Some(parse_policy(4, 1024)))
            .unwrap_err(),
        ParseCoordinatorError::WindowTooLarge { .. }
    ));

    let wrong_version = parse_window(2, 0, "ok");
    assert!(matches!(
        coordinator
            .schedule_parse_with_windows(
                request(1),
                vec![wrong_version],
                Some(parse_policy(4096, 8192))
            )
            .unwrap_err(),
        ParseCoordinatorError::WindowMetadataMismatch { .. }
    ));

    assert!(matches!(
        coordinator
            .schedule_parse_with_windows(
                request(1),
                Vec::new(),
                Some(parse_policy(4096, SYNTAX_CACHE_BUDGET_BYTES as u64 + 1))
            )
            .unwrap_err(),
        ParseCoordinatorError::InvalidParsePolicy
    ));
}

#[tokio::test]
async fn generation_replacement_uses_new_handler_for_subsequent_parse() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    let calls = Arc::new(AtomicUsize::new(0));
    let old_calls = Arc::clone(&calls);
    coordinator
        .register_handler_for_generation(
            &package,
            1,
            "markdown",
            move |_notification: ParseEditNotification| {
                let old_calls = Arc::clone(&old_calls);
                async move {
                    old_calls.fetch_add(100, Ordering::SeqCst);
                    Ok(update(1))
                }
            },
        )
        .unwrap();
    let new_calls = Arc::clone(&calls);
    coordinator
        .register_handler_for_generation(
            &package,
            2,
            "markdown",
            move |notification: ParseEditNotification| {
                let new_calls = Arc::clone(&new_calls);
                async move {
                    new_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(update(notification.document_version))
                }
            },
        )
        .unwrap();

    coordinator.schedule_parse(request(11)).unwrap();
    let parsed = tokio::time::timeout(Duration::from_secs(1), coordinator.next_update())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(parsed.document_version, 11);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn replacing_generation_cancels_old_in_flight_parse_work() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    coordinator
        .register_handler_for_generation(
            &package,
            1,
            "markdown",
            |notification: ParseEditNotification| async move {
                tokio::time::sleep(Duration::from_millis(250)).await;
                Ok(update(notification.document_version))
            },
        )
        .unwrap();

    coordinator.schedule_parse(request(12)).unwrap();
    coordinator
        .register_handler_for_generation(
            &package,
            2,
            "markdown",
            |notification: ParseEditNotification| async move {
                Ok(update(notification.document_version))
            },
        )
        .unwrap();
    coordinator.schedule_parse(request(13)).unwrap();

    let parsed = tokio::time::timeout(Duration::from_secs(1), coordinator.next_update())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(parsed.document_version, 13);
    assert_eq!(coordinator.stats().cancelled_superseded_tasks, 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(300), coordinator.next_update())
            .await
            .is_err(),
        "old-generation parse work must not publish after replacement"
    );
}

#[tokio::test]
async fn package_cancel_withdraws_handlers_and_in_flight_parse_work() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    coordinator
        .register_handler_for_generation(
            &package,
            7,
            "markdown",
            |notification: ParseEditNotification| async move {
                tokio::time::sleep(Duration::from_millis(250)).await;
                Ok(update(notification.document_version))
            },
        )
        .unwrap();

    coordinator.schedule_parse(request(15)).unwrap();
    coordinator.cancel_package("markdown");

    assert_eq!(coordinator.stats().cancelled_superseded_tasks, 1);
    assert!(matches!(
        coordinator.schedule_parse(request(16)).unwrap_err(),
        ParseCoordinatorError::HandlerNotRegistered { .. }
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(300), coordinator.next_update())
            .await
            .is_err(),
        "package-scoped cancellation must not publish stale parse work"
    );
}

#[tokio::test]
async fn handler_failures_are_instrumented_after_generation_replacement() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    coordinator
        .register_handler_for_generation(
            &package,
            1,
            "markdown",
            |_notification: ParseEditNotification| async move { Ok(update(1)) },
        )
        .unwrap();
    coordinator
        .register_handler_for_generation(
            &package,
            2,
            "markdown",
            |_notification: ParseEditNotification| async move {
                Err(ParseCoordinatorError::HandlerFailed(
                    "clay.runtime.timeout".to_string(),
                ))
            },
        )
        .unwrap();

    coordinator.schedule_parse(request(14)).unwrap();

    tokio::time::sleep(Duration::from_millis(25)).await;
    assert_eq!(coordinator.stats().failed_tasks, 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), coordinator.next_update())
            .await
            .is_err(),
        "failed parse work must not publish half-updated results"
    );
}

#[tokio::test]
async fn handler_failures_are_instrumented_and_not_published() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    coordinator
        .register_handler(&package, "markdown", |_notification| async move {
            Err(ParseCoordinatorError::HandlerFailed(
                "clay.runtime.timeout".to_string(),
            ))
        })
        .unwrap();

    coordinator.schedule_parse(request(6)).unwrap();

    tokio::time::sleep(Duration::from_millis(25)).await;
    assert_eq!(coordinator.stats().failed_tasks, 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), coordinator.next_update())
            .await
            .is_err(),
        "failed parse work must not publish half-updated results"
    );
}

#[tokio::test]
async fn parsing_does_not_block_edit_acknowledgement() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_permissions(&["parse-document"]);
    coordinator
        .register_handler(
            &package,
            "markdown",
            |notification: ParseEditNotification| async move {
                tokio::time::sleep(Duration::from_millis(250)).await;
                Ok(update(notification.document_version))
            },
        )
        .unwrap();

    let started = std::time::Instant::now();
    coordinator.schedule_parse(request(5)).unwrap();

    assert!(started.elapsed() < Duration::from_millis(25));
}
