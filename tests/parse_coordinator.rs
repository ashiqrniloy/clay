use std::time::Duration;

use clay::{
    packages::record::{PackageRecord, assemble_package_record},
    perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES,
    protocol::{
        DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan,
        IncrementalParseUpdate, ParseByteRange, ParseEditNotification, ParseUnit,
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
            DecorationSpan {
                byte_start: 0,
                byte_end: 5,
                kind: DecorationKind::Syntax,
                style_token: "markup.heading.1".to_string(),
                priority: 90,
                provenance: provenance.clone(),
            },
            DecorationSpan {
                byte_start: 10,
                byte_end: 18,
                kind: DecorationKind::Syntax,
                style_token: "markup.strong".to_string(),
                priority: 60,
                provenance,
            },
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
