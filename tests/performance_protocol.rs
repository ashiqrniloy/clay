use std::time::{Duration, Instant};

use clay::{
    client::ClientEditQueue,
    editor::{EditorCommand, EditorEditEvent, EditorSurface},
    packages::record::{PackageRecord, assemble_package_record},
    perf::{
        baselines::representative_sdui_tree,
        budgets::{
            BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES, CLIENT_EDIT_PAYLOAD_BUDGET_BYTES,
            COMPLETION_RESULT_MAX_ITEMS, COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES,
            DECORATION_NEAR_VIEWPORT_GUARD_BYTES, DECORATION_PAYLOAD_BUDGET_BYTES,
            DIAGNOSTIC_PAYLOAD_BUDGET_BYTES, DOCUMENT_ANALYSIS_INPUT_MAX_BYTES,
            DOCUMENT_ANALYSIS_INPUT_MAX_EVENTS, DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES,
            DOCUMENT_ANALYSIS_MAX_DOCUMENTS_PER_WORKER, DOCUMENT_ANALYSIS_MAX_PENDING_REQUESTS,
            DOCUMENT_ANALYSIS_MAX_TEXT_BYTES_PER_WORKER, DOCUMENT_ANALYSIS_MAX_WORKERS,
            DOCUMENT_ANALYSIS_OUTPUT_MAX_BYTES, DOCUMENT_ANALYSIS_OUTPUT_MAX_EVENTS,
            DOCUMENT_ANALYSIS_WORKER_HEAP_BYTES, EDIT_ACK_PAYLOAD_BUDGET_BYTES,
            INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES, LANGUAGE_INTELLIGENCE_MAX_HOVER_MARKDOWN_CHARS,
            LANGUAGE_INTELLIGENCE_RESULT_PAYLOAD_BUDGET_BYTES, LANGUAGE_SERVER_MAX_SESSIONS,
            LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES, LANGUAGE_SERVER_STDERR_BUDGET_BYTES,
            PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS, PREVIOUS_BEHAVIOR_GRACE_MS,
            RUNTIME_STATE_BROADCAST_CAPACITY, RUNTIME_STATE_INSTALL_DIFF_REVIEW_P95_MS,
            RUNTIME_STATE_SNAPSHOT_DIFF_REVIEW_PAYLOAD_BYTES,
            RUNTIME_STATE_SNAPSHOT_MAX_DIAGNOSTICS, RUNTIME_STATE_SNAPSHOT_MAX_DOCUMENTS,
            SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES, SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
            SYNTAX_CACHE_BUDGET_BYTES,
        },
        metrics::{
            MetricMetadata, MetricValue, PERF_SNAPSHOT_CAPACITY, PerfConfig, PerfRecorder,
            SYNTAX_CANCELLED_SUPERSEDED, SYNTAX_DECORATION_CHUNKS, SYNTAX_EDIT_TO_PUBLISH,
            SYNTAX_LOGICAL_WORK_ITEMS, SYNTAX_PARSE_FULL, SYNTAX_PARSE_INCREMENTAL,
            SYNTAX_PARSE_INVOCATIONS, SYNTAX_QUERY_BYTES, SYNTAX_QUERY_RANGES,
            install_global_recorder,
        },
    },
    protocol::{
        BehaviorManifest, ClientMessage, CompletionItem, CompletionProvenance,
        CompletionReplacementRange, CompletionResultSet, CompletionStatus, CompletionTrigger,
        DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan, DiagnosticSet,
        DiagnosticSeverity, DiagnosticSpan, DocumentAccess, EditOperation, HoverResult,
        IncrementalParseUpdate, LanguageIntelligenceFeature, LanguageIntelligencePayload,
        LanguageIntelligenceResult, LanguageIntelligenceStatus, ParseByteRange,
        ParseEditNotification, ParsePolicy, ParseUnit, ParseWindowRequest, ParseWindowSnapshot,
        ServerMessage, SyntaxMemoryBudget,
        codec::{Codec, CodecError, DEFAULT_MAX_FRAME_SIZE},
    },
    server::{
        parse_coordinator::{ParseCoordinator, ParseScheduleRequest},
        syntax::{SyntaxGrammarRegistry, TreeSitterSyntaxHandler},
    },
};
use serde_json::json;

const FRAME_PREFIX_BYTES: usize = 4;

fn edit_event(byte_offset: u64, text: &str) -> EditorEditEvent {
    EditorEditEvent {
        document_id: 7,
        base_version: 1,
        behavior_version: 1,
        operation: EditOperation::Insert {
            byte_offset,
            text: text.to_string(),
        },
    }
}

fn payload_len(frame: &[u8]) -> usize {
    frame.len().saturating_sub(FRAME_PREFIX_BYTES)
}

fn first_party_package_record(package: &str) -> PackageRecord {
    let path = format!("packages/{package}/package.json");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    let value = serde_json::from_str(&text).unwrap_or_else(|error| panic!("parse {path}: {error}"));
    assemble_package_record(&value).unwrap_or_else(|error| panic!("assemble {path}: {error:?}"))
}

fn package_with_parse_permission() -> PackageRecord {
    assemble_package_record(&json!({
        "name": "@clay/markdown",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "markdown",
            "entry": "./dist/index.js",
            "permissions": ["parse-document"],
            "modes": ["markdown"],
            "docs": "./docs/index.md"
        }
    }))
    .expect("package fixture validates")
}

fn markdown_parse_update_from_notification(
    notification: ParseEditNotification,
) -> IncrementalParseUpdate {
    IncrementalParseUpdate {
        document_id: notification.document_id,
        document_version: notification.document_version,
        behavior_version: notification.behavior_version,
        package_prefix: notification.package_prefix,
        mode_id: notification.mode_id,
        parse_unit: ParseUnit::LineGroup,
        viewport: notification.viewport,
        invalidated_ranges: notification.invalidated_ranges,
        syntax_tree_delta: Some("windowed:visible".to_string()),
        decoration_updates: Vec::new(),
        diagnostic_update: None,
    }
}

fn decoration_set_for_payload() -> DecorationSet {
    DecorationSet {
        document_id: 7,
        document_version: 3,
        package_prefix: "markdown".to_string(),
        kind: DecorationKind::Syntax,
        viewport_byte_start: 8 * 1024 * 1024,
        viewport_byte_end: 8 * 1024 * 1024 + DECORATION_NEAR_VIEWPORT_GUARD_BYTES,
        spans: vec![DecorationSpan::from_style_token(
            8 * 1024 * 1024,
            8 * 1024 * 1024 + 16,
            DecorationKind::Syntax,
            "markup.heading.1",
            10,
            DecorationProvenance {
                package_name: "@clay/markdown".to_string(),
                package_version: "0.1.0".to_string(),
                package_prefix: "markdown".to_string(),
            },
        )],
    }
}

#[test]
fn ordinary_edit_updates_shadow_before_ack() {
    let mut surface = EditorSurface::default();
    surface.load_snapshot(
        7,
        1,
        "abc".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    surface.install_behavior_manifest(BehaviorManifest::minimal_text_editing(1));
    surface.command(EditorCommand::DocumentEnd);

    let (queue, _receiver) = ClientEditQueue::bounded(1);
    let queue = queue
        .with_authority(11, &DocumentAccess::Editable { lease_id: 1 })
        .with_confirmed_version(1);

    let first = surface.command_with_event(EditorCommand::Insert("!"));
    assert!(first.changed);
    queue
        .enqueue_edit_event(first.edit_event.expect("edit event"), 1)
        .expect("first edit should fit queue");

    let second = surface.command_with_event(EditorCommand::Insert("?"));
    assert!(second.changed);
    assert_eq!(surface.visible_text(), "abc!?");

    let second_send = queue.enqueue_edit_event(second.edit_event.expect("edit event"), 2);
    assert!(matches!(
        second_send,
        Err(tokio::sync::mpsc::error::TrySendError::Full(_))
    ));
    assert_eq!(surface.visible_text(), "abc!?");
}

#[test]
fn client_edit_queue_reports_depth_without_blocking_input() {
    let (queue, _receiver) = ClientEditQueue::bounded(1);
    let queue = queue
        .with_authority(11, &DocumentAccess::Editable { lease_id: 1 })
        .with_confirmed_version(1);

    queue
        .enqueue_edit_event(edit_event(0, "x"), 1)
        .expect("first edit should enqueue");

    let started = Instant::now();
    let second = queue.enqueue_edit_event(edit_event(1, "y"), 2);
    let elapsed = started.elapsed();

    assert!(matches!(
        second,
        Err(tokio::sync::mpsc::error::TrySendError::Full(_))
    ));
    assert!(
        elapsed < Duration::from_millis(50),
        "full queue must fail fast via try_send; observed {elapsed:?}"
    );
    assert_eq!(queue.sync_snapshot().pending.len(), 1);
}

#[test]
fn representative_protocol_payloads_fit_phase14_budgets() {
    let codec = Codec::default();

    let client_edit = ClientMessage::Edit {
        document_id: 7,
        client_id: 11,
        lease_id: Some(1),
        base_version: 1,
        behavior_version: 3,
        transaction_id: 99,
        operation: EditOperation::Insert {
            byte_offset: 128,
            text: "x".repeat(96),
        },
    };
    let client_edit_payload = payload_len(&codec.encode_client_message(&client_edit).unwrap());
    assert!(
        client_edit_payload <= CLIENT_EDIT_PAYLOAD_BUDGET_BYTES,
        "client edit payload {client_edit_payload} exceeds budget {CLIENT_EDIT_PAYLOAD_BUDGET_BYTES}"
    );

    let edit_ack = ServerMessage::EditAck {
        document_id: 7,
        confirmed_version: 2,
        transaction_id: 99,
    };
    let edit_ack_payload = payload_len(&codec.encode_server_message(&edit_ack).unwrap());
    assert!(
        edit_ack_payload <= EDIT_ACK_PAYLOAD_BUDGET_BYTES,
        "edit ack payload {edit_ack_payload} exceeds budget {EDIT_ACK_PAYLOAD_BUDGET_BYTES}"
    );

    let manifest = ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(3));
    let manifest_payload = payload_len(&codec.encode_server_message(&manifest).unwrap());
    assert!(
        manifest_payload <= BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES,
        "behavior manifest payload {manifest_payload} exceeds budget {BEHAVIOR_MANIFEST_PAYLOAD_BUDGET_BYTES}"
    );

    let sdui_snapshot = ServerMessage::SduiSnapshot {
        client_id: 11,
        tree: representative_sdui_tree(),
    };
    let sdui_snapshot_payload = payload_len(&codec.encode_server_message(&sdui_snapshot).unwrap());
    assert!(
        sdui_snapshot_payload <= SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES,
        "SDUI snapshot payload {sdui_snapshot_payload} exceeds budget {SDUI_SNAPSHOT_PAYLOAD_BUDGET_BYTES}"
    );

    let sdui_update = ServerMessage::SduiUpdate {
        update: clay::perf::baselines::representative_panel_update(),
    };
    let sdui_update_payload = payload_len(&codec.encode_server_message(&sdui_update).unwrap());
    assert!(
        sdui_update_payload <= SDUI_UPDATE_PAYLOAD_BUDGET_BYTES,
        "SDUI update payload {sdui_update_payload} exceeds budget {SDUI_UPDATE_PAYLOAD_BUDGET_BYTES}"
    );
}

#[test]
fn decoration_chunk_protocol_payload_stays_bounded_for_large_file_viewport() {
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&decoration_set_for_payload())
        .expect("decoration chunk serializes")
        .len();

    assert!(bytes <= DECORATION_PAYLOAD_BUDGET_BYTES);
}

#[test]
fn representative_diagnostic_chunk_payload_stays_bounded() {
    let provenance = DecorationProvenance {
        package_name: "@clay/rust".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "rust".to_string(),
    };
    let set = DiagnosticSet {
        document_id: 7,
        document_version: 3,
        viewport_byte_start: 0,
        viewport_byte_end: 4096,
        source: "tree-sitter".to_string(),
        provenance: provenance.clone(),
        spans: vec![DiagnosticSpan {
            byte_start: 8,
            byte_end: 9,
            severity: DiagnosticSeverity::Error,
            code: "syntax.error".to_string(),
            message: "unexpected token".to_string(),
            source: "tree-sitter".to_string(),
            provenance,
        }],
    };
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&set)
        .expect("diagnostic chunk serializes")
        .len();

    assert!(bytes <= DIAGNOSTIC_PAYLOAD_BUDGET_BYTES);
}

#[test]
fn combined_parse_update_stays_within_incremental_payload_budget() {
    let provenance = DecorationProvenance {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "markdown".to_string(),
    };
    let update = IncrementalParseUpdate {
        document_id: 7,
        document_version: 3,
        behavior_version: 1,
        package_prefix: "markdown".to_string(),
        mode_id: "markdown".to_string(),
        parse_unit: ParseUnit::LineGroup,
        viewport: ParseByteRange::new(0, 4096),
        invalidated_ranges: vec![ParseByteRange::new(8, 9)],
        syntax_tree_delta: None,
        decoration_updates: vec![DecorationSet {
            document_id: 7,
            document_version: 3,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 4096,
            spans: vec![DecorationSpan::from_style_token(
                8,
                9,
                DecorationKind::Syntax,
                "punctuation.definition",
                10,
                provenance.clone(),
            )],
        }],
        diagnostic_update: Some(DiagnosticSet {
            document_id: 7,
            document_version: 3,
            viewport_byte_start: 0,
            viewport_byte_end: 4096,
            source: "markdown-parser".to_string(),
            provenance: provenance.clone(),
            spans: vec![DiagnosticSpan {
                byte_start: 8,
                byte_end: 9,
                severity: DiagnosticSeverity::Error,
                code: "syntax.error".to_string(),
                message: "syntax error".to_string(),
                source: "markdown-parser".to_string(),
                provenance,
            }],
        }),
    };
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&update)
        .expect("combined update serializes")
        .len();

    assert!(bytes <= INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES);
}

#[test]
fn representative_completion_result_payload_stays_bounded() {
    // A representative full completion result set (max items, short labels)
    // must stay under `COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES` so completion
    // publication never blows the protocol frame budget.
    let codec = Codec::default();
    let provenance = CompletionProvenance::builtin_core();
    let items: Vec<CompletionItem> = (0..COMPLETION_RESULT_MAX_ITEMS)
        .map(|i| CompletionItem::new(format!("item{i}"), format!("item{i}"), provenance.clone()))
        .collect();
    let result = CompletionResultSet {
        request_id: 42,
        client_id: 9,
        document_id: 7,
        document_version: 31,
        behavior_version: 3,
        provider_generation: 2,
        replacement_range: CompletionReplacementRange::new(10, 12),
        status: CompletionStatus::Ok,
        items,
        provenance: CompletionProvenance::builtin_core(),
    };
    assert!(
        clay::protocol::check_result_payload_budget(&result).is_ok(),
        "representative completion result must pass the pre-publication payload budget check"
    );
    let message = ServerMessage::CompletionResult { result };
    let payload = payload_len(&codec.encode_server_message(&message).unwrap());
    assert!(
        payload <= COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES,
        "representative completion result payload {payload} exceeds budget {COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES}"
    );
}

#[test]
fn representative_language_intelligence_result_payload_stays_bounded() {
    // A representative hover result carrying a large but in-budget Markdown
    // payload must stay under `LANGUAGE_INTELLIGENCE_RESULT_PAYLOAD_BUDGET_BYTES`
    // so language-intelligence publication never blows the protocol frame
    // budget. Code-action/definition/signature payloads are bounded by the
    // per-field/count limits enforced by `validate_result`.
    let codec = Codec::default();
    let result = LanguageIntelligenceResult {
        request_id: 42,
        client_id: 9,
        document_id: 7,
        document_version: 31,
        behavior_version: 3,
        provider_generation: 2,
        feature: LanguageIntelligenceFeature::Hover,
        status: LanguageIntelligenceStatus::Ok,
        payload: LanguageIntelligencePayload::Hover(HoverResult {
            range: Some(clay::protocol::TextByteRange::new(10, 14)),
            markdown: "# Heading\n".repeat(LANGUAGE_INTELLIGENCE_MAX_HOVER_MARKDOWN_CHARS / 10),
        }),
        provenance: CompletionProvenance::builtin_core(),
    };
    assert!(
        clay::server::language_intelligence::validate_result(&result).is_ok(),
        "representative language-intelligence result must pass validation"
    );
    let message = ServerMessage::LanguageIntelligenceResult { result };
    let payload = payload_len(&codec.encode_server_message(&message).unwrap());
    assert!(
        payload <= LANGUAGE_INTELLIGENCE_RESULT_PAYLOAD_BUDGET_BYTES,
        "representative language-intelligence payload {payload} exceeds budget {LANGUAGE_INTELLIGENCE_RESULT_PAYLOAD_BUDGET_BYTES}"
    );
}

#[test]
fn completion_request_payload_stays_bounded() {
    let codec = Codec::default();
    let request = clay::protocol::CompletionRequest {
        request_id: 42,
        client_id: 9,
        document_id: 7,
        document_version: 31,
        behavior_version: 3,
        cursor_byte_offset: 12,
        replacement_range: CompletionReplacementRange::new(10, 12),
        trigger: CompletionTrigger::Character(".".to_string()),
        provider_generation: 2,
    };
    let message = ClientMessage::CompletionRequest { request };
    let payload = payload_len(&codec.encode_client_message(&message).unwrap());
    assert!(
        payload <= clay::perf::budgets::COMPLETION_REQUEST_PAYLOAD_BUDGET_BYTES,
        "completion request payload {payload} exceeds request budget"
    );
}

#[test]
fn parse_window_policy_keeps_large_file_snapshot_budget_bounded() {
    let policy = ParsePolicy::new(64 * 1024, 4 * 1024, SYNTAX_CACHE_BUDGET_BYTES as u64, 50);
    let request = ParseWindowRequest {
        document_id: 7,
        document_version: 12,
        behavior_version: 3,
        package_prefix: "plain".to_string(),
        mode_id: "plain".to_string(),
        requested_ranges: vec![ParseByteRange::new(8 * 1024 * 1024, 8 * 1024 * 1024 + 4096)],
        viewport: ParseByteRange::new(8 * 1024 * 1024, 8 * 1024 * 1024 + 4096),
        policy,
    };
    let metadata_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&request)
        .expect("parse window request serializes")
        .len();
    let budget = SyntaxMemoryBudget::new(policy.memory_budget_bytes, 4096);

    assert!(metadata_bytes <= INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES);
    assert_eq!(budget.remaining_bytes(), policy.memory_budget_bytes - 4096);
    assert!(!budget.is_exceeded());
}

#[test]
fn syntax_pipeline_metrics_are_source_safe_and_retention_bounded() {
    let recorder = PerfRecorder::for_test(true);
    let names = [
        SYNTAX_LOGICAL_WORK_ITEMS,
        SYNTAX_PARSE_INVOCATIONS,
        SYNTAX_PARSE_INCREMENTAL,
        SYNTAX_PARSE_FULL,
        SYNTAX_QUERY_RANGES,
        SYNTAX_QUERY_BYTES,
        SYNTAX_DECORATION_CHUNKS,
        SYNTAX_CANCELLED_SUPERSEDED,
        SYNTAX_EDIT_TO_PUBLISH,
    ];
    for index in 0..=PERF_SNAPSHOT_CAPACITY {
        recorder.record_with_metadata(
            names[index % names.len()],
            MetricValue::Counter { amount: 1 },
            MetricMetadata::document(7, 3),
        );
    }

    let snapshots = recorder.snapshots();
    assert_eq!(snapshots.len(), PERF_SNAPSHOT_CAPACITY);
    assert!(snapshots.iter().all(|snapshot| {
        snapshot.metadata == MetricMetadata::document(7, 3)
            && snapshot.metadata.sanitized_path.is_none()
    }));
    let debug = format!("{snapshots:?}");
    assert!(!debug.contains("source text"));
    assert!(!debug.contains("/home/"));
    assert!(!debug.contains("clipboard"));
}

#[tokio::test]
async fn markdown_large_file_typing_does_not_wait_for_windowed_parse() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_parse_permission();
    coordinator
        .register_handler(
            &package,
            "markdown",
            |notification: ParseEditNotification| async move {
                tokio::time::sleep(Duration::from_millis(250)).await;
                Ok(markdown_parse_update_from_notification(notification))
            },
        )
        .expect("Markdown parse handler registers");

    let mut surface = EditorSurface::default();
    surface.load_snapshot(
        7,
        1,
        "# Large visible heading\n".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    surface.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
    surface.command(EditorCommand::DocumentEnd);

    let window_text = "# Large visible heading\n".repeat(256);
    let byte_start = 8 * 1024 * 1024;
    let started = Instant::now();
    coordinator
        .schedule_parse_with_windows(
            ParseScheduleRequest {
                document_id: 7,
                document_version: 1,
                behavior_version: 3,
                package_prefix: "markdown".to_string(),
                mode_id: "markdown".to_string(),
                viewport: ParseByteRange::new(byte_start, byte_start + 4096),
                invalidated_ranges: vec![ParseByteRange::new(byte_start, byte_start + 32)],
                accepted_edit: None,
            },
            vec![ParseWindowSnapshot {
                document_id: 7,
                document_version: 1,
                package_prefix: "markdown".to_string(),
                mode_id: "markdown".to_string(),
                window_id: byte_start,
                byte_start,
                byte_end: byte_start + window_text.len() as u64,
                base_line: 0,
                base_column: 0,
                incremental_edit: false,
                text: window_text,
            }],
            Some(ParsePolicy::new(
                64 * 1024,
                4 * 1024,
                SYNTAX_CACHE_BUDGET_BYTES as u64,
                50,
            )),
        )
        .expect("windowed parse schedules in background");
    let outcome = surface.command_with_event(EditorCommand::Insert("!"));

    assert!(outcome.changed);
    assert_eq!(surface.visible_text(), "# Large visible heading\n!");
    assert!(
        started.elapsed() < Duration::from_millis(25),
        "local large-file typing must not wait for slow windowed parser"
    );
}

#[tokio::test]
async fn valid_to_invalid_edit_keeps_local_typing_non_blocking() {
    let coordinator = ParseCoordinator::new();
    let package = package_with_parse_permission();
    coordinator
        .register_handler(
            &package,
            "markdown",
            |notification: ParseEditNotification| async move {
                tokio::time::sleep(Duration::from_millis(250)).await;
                let mut update = markdown_parse_update_from_notification(notification.clone());
                let provenance = DecorationProvenance {
                    package_name: "@clay/markdown".to_string(),
                    package_version: "0.1.0".to_string(),
                    package_prefix: "markdown".to_string(),
                };
                update.diagnostic_update = Some(DiagnosticSet {
                    document_id: notification.document_id,
                    document_version: notification.document_version,
                    viewport_byte_start: notification.viewport.start,
                    viewport_byte_end: notification.viewport.end,
                    source: "tree-sitter".to_string(),
                    provenance: provenance.clone(),
                    spans: vec![DiagnosticSpan {
                        byte_start: notification.viewport.start,
                        byte_end: notification.viewport.start + 1,
                        severity: DiagnosticSeverity::Error,
                        code: "syntax.error".to_string(),
                        message: "syntax error".to_string(),
                        source: "tree-sitter".to_string(),
                        provenance,
                    }],
                });
                Ok(update)
            },
        )
        .expect("Markdown parse handler registers");

    let mut surface = EditorSurface::default();
    surface.load_snapshot(
        7,
        1,
        "fn main() {}\n".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    surface.install_behavior_manifest(BehaviorManifest::minimal_text_editing(3));
    surface.command(EditorCommand::DocumentEnd);

    let window_text = "fn main() {}\n".to_string();
    let window_end = window_text.len() as u64;
    let started = Instant::now();
    coordinator
        .schedule_parse_with_windows(
            ParseScheduleRequest {
                document_id: 7,
                document_version: 1,
                behavior_version: 3,
                package_prefix: "markdown".to_string(),
                mode_id: "markdown".to_string(),
                viewport: ParseByteRange::new(0, window_end),
                invalidated_ranges: vec![ParseByteRange::new(0, window_end)],
                accepted_edit: None,
            },
            vec![ParseWindowSnapshot {
                document_id: 7,
                document_version: 1,
                package_prefix: "markdown".to_string(),
                mode_id: "markdown".to_string(),
                window_id: 0,
                byte_start: 0,
                byte_end: window_end,
                base_line: 0,
                base_column: 0,
                incremental_edit: false,
                text: window_text,
            }],
            Some(ParsePolicy::new(
                64 * 1024,
                4 * 1024,
                SYNTAX_CACHE_BUDGET_BYTES as u64,
                50,
            )),
        )
        .expect("diagnostic parse schedules in background");
    let outcome = surface.command_with_event(EditorCommand::Insert("!"));

    assert!(outcome.changed);
    assert_eq!(surface.visible_text(), "fn main() {}\n!");
    assert_eq!(surface.diagnostic_span_count(), 0);
    assert!(
        started.elapsed() < Duration::from_millis(25),
        "local typing must not wait for slow diagnostic-producing parse"
    );
}

#[cfg(any(unix, windows))]
#[test]
fn first_party_decoration_payloads_stay_within_budget_per_language() {
    let registry = SyntaxGrammarRegistry::with_first_party_native();

    for (label, package, contribution_id, language, query, text) in [
        (
            "rust",
            "rust",
            "rust.rust",
            tree_sitter_rust::LANGUAGE.into(),
            include_str!("../packages/rust/queries/highlights.scm"),
            include_str!("fixtures/syntax/rust.rs"),
        ),
        (
            "typescript",
            "typescript",
            "typescript.typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            include_str!("fixtures/syntax/typescript.ts"),
        ),
        (
            "tsx",
            "typescript",
            "typescript.tsx",
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            include_str!("../packages/typescript/queries/highlights.scm"),
            include_str!("fixtures/syntax/typescript.tsx"),
        ),
        (
            "javascript",
            "javascript",
            "javascript.javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            include_str!("../packages/javascript/queries/highlights.scm"),
            include_str!("fixtures/syntax/javascript.js"),
        ),
        (
            "markdown",
            "markdown",
            "markdown.markdown",
            tree_sitter_md_025::LANGUAGE.into(),
            include_str!("../packages/markdown/queries/highlights.scm"),
            include_str!("fixtures/syntax/markdown.md"),
        ),
    ] {
        let contribution = registry
            .get(contribution_id)
            .unwrap_or_else(|| panic!("registered {contribution_id}"))
            .clone();
        assert_eq!(contribution.max_window_bytes, Some(4096));
        assert_eq!(contribution.timeout_ms, Some(5000));
        let handler = TreeSitterSyntaxHandler::new(contribution, language, query)
            .unwrap_or_else(|error| panic!("compile {label} query: {error}"));
        let update = handler
            .parse_sync(parse_notification_for_fixture(package, 1, text))
            .unwrap_or_else(|error| panic!("parse {label}: {error}"));
        assert!(
            !update.decoration_updates.is_empty(),
            "{label} emits decorations"
        );
        for decoration in &update.decoration_updates {
            let decoration_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(decoration)
                .unwrap_or_else(|error| panic!("serialize {label} decorations: {error}"))
                .len();
            let mut member = update.clone();
            member.decoration_updates = vec![decoration.clone()];
            let update_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&member)
                .unwrap_or_else(|error| panic!("serialize {label} update: {error}"))
                .len();

            assert!(decoration_bytes <= DECORATION_PAYLOAD_BUDGET_BYTES);
            assert!(update_bytes <= INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES);
        }
        assert!(text.len() <= SYNTAX_CACHE_BUDGET_BYTES);
    }
}

fn parse_notification_for_fixture(
    package: &str,
    version: u64,
    text: &str,
) -> ParseEditNotification {
    ParseEditNotification {
        document_id: 77,
        document_version: version,
        behavior_version: 1,
        package_prefix: package.to_string(),
        mode_id: package.to_string(),
        viewport: ParseByteRange::new(0, text.len() as u64),
        invalidated_ranges: vec![ParseByteRange::new(0, text.len() as u64)],
        accepted_edit: None,
        parse_windows: vec![ParseWindowSnapshot {
            document_id: 77,
            document_version: version,
            package_prefix: package.to_string(),
            mode_id: package.to_string(),
            window_id: 0,
            byte_start: 0,
            byte_end: text.len() as u64,
            base_line: 0,
            base_column: 0,
            incremental_edit: false,
            text: text.to_string(),
        }],
        memory_budget: Some(SyntaxMemoryBudget::new(
            SYNTAX_CACHE_BUDGET_BYTES as u64,
            text.len() as u64,
        )),
    }
}

#[tokio::test]
async fn first_party_open_parse_does_not_block_initial_render_per_language() {
    for (package, text) in [
        ("rust", include_str!("fixtures/syntax/rust.rs")),
        ("typescript", include_str!("fixtures/syntax/typescript.ts")),
        ("javascript", include_str!("fixtures/syntax/javascript.js")),
        ("markdown", include_str!("fixtures/syntax/markdown.md")),
    ] {
        let coordinator = ParseCoordinator::new();
        let record = first_party_package_record(package);
        coordinator
            .register_handler(
                &record,
                package,
                |notification: ParseEditNotification| async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(IncrementalParseUpdate {
                        document_id: notification.document_id,
                        document_version: notification.document_version,
                        behavior_version: notification.behavior_version,
                        package_prefix: notification.package_prefix,
                        mode_id: notification.mode_id,
                        parse_unit: ParseUnit::Region,
                        viewport: notification.viewport,
                        invalidated_ranges: notification.invalidated_ranges,
                        syntax_tree_delta: None,
                        decoration_updates: Vec::new(),
                        diagnostic_update: None,
                    })
                },
            )
            .unwrap_or_else(|error| panic!("register {package} handler: {error:?}"));

        let mut surface = EditorSurface::default();
        surface.load_snapshot(
            77,
            1,
            text.to_string(),
            DocumentAccess::Editable { lease_id: 1 },
        );
        let started = Instant::now();
        coordinator
            .schedule_parse_with_windows(
                ParseScheduleRequest {
                    document_id: 77,
                    document_version: 1,
                    behavior_version: 1,
                    package_prefix: package.to_string(),
                    mode_id: package.to_string(),
                    viewport: ParseByteRange::new(0, text.len() as u64),
                    invalidated_ranges: vec![ParseByteRange::new(0, text.len() as u64)],
                    accepted_edit: None,
                },
                parse_notification_for_fixture(package, 1, text).parse_windows,
                Some(ParsePolicy::new(
                    4096,
                    4096,
                    SYNTAX_CACHE_BUDGET_BYTES as u64,
                    5000,
                )),
            )
            .unwrap_or_else(|error| panic!("schedule {package} parse: {error:?}"));

        assert_eq!(surface.visible_text(), text);
        assert!(
            started.elapsed() < Duration::from_millis(25),
            "{package} initial render must not wait for background parse"
        );
    }
}

#[test]
fn oversized_and_invalid_frames_still_rejected_with_metrics_enabled() {
    install_global_recorder(PerfConfig::enabled());

    let codec = Codec::new(32);
    let oversized = ServerMessage::BehaviorManifest(BehaviorManifest::minimal_text_editing(1));
    assert!(matches!(
        codec.encode_server_message(&oversized),
        Err(CodecError::FrameTooLarge { max: 32, .. })
    ));

    let invalid = [0, 0, 0, 4, 0xde, 0xad, 0xbe, 0xef];
    assert!(matches!(
        codec.decode_client_message(&invalid),
        Err(CodecError::Deserialize(_))
    ));

    let mut oversize_declared = vec![];
    oversize_declared.extend_from_slice(&64_u32.to_be_bytes());
    oversize_declared.extend_from_slice(&[0; 64]);
    assert!(matches!(
        codec.decode_server_message(&oversize_declared),
        Err(CodecError::FrameTooLarge { len: 64, max: 32 })
    ));
}

#[test]
fn phase18_21_language_server_and_document_analysis_budgets_are_locked() {
    assert_eq!(LANGUAGE_SERVER_MESSAGE_BUDGET_BYTES, 1024 * 1024);
    assert_eq!(LANGUAGE_SERVER_STDERR_BUDGET_BYTES, 64 * 1024);
    assert_eq!(LANGUAGE_SERVER_MAX_SESSIONS, 16);
    assert_eq!(DOCUMENT_ANALYSIS_MAX_WORKERS, 4);
    assert_eq!(DOCUMENT_ANALYSIS_WORKER_HEAP_BYTES, 64 * 1024 * 1024);
    assert_eq!(DOCUMENT_ANALYSIS_MAX_DOCUMENTS_PER_WORKER, 32);
    assert_eq!(DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES, 256 * 1024);
    assert_eq!(DOCUMENT_ANALYSIS_MAX_TEXT_BYTES_PER_WORKER, 8 * 1024 * 1024);
    assert_eq!(DOCUMENT_ANALYSIS_INPUT_MAX_EVENTS, 64);
    assert_eq!(DOCUMENT_ANALYSIS_INPUT_MAX_BYTES, 2 * 1024 * 1024);
    assert_eq!(DOCUMENT_ANALYSIS_OUTPUT_MAX_EVENTS, 64);
    assert_eq!(DOCUMENT_ANALYSIS_OUTPUT_MAX_BYTES, 512 * 1024);
    assert_eq!(DOCUMENT_ANALYSIS_MAX_PENDING_REQUESTS, 8);
    assert_eq!(DECORATION_PAYLOAD_BUDGET_BYTES, 8 * 1024);
    assert_eq!(DIAGNOSTIC_PAYLOAD_BUDGET_BYTES, 8 * 1024);
    assert_eq!(COMPLETION_RESULT_PAYLOAD_BUDGET_BYTES, 16 * 1024);
    assert_eq!(LANGUAGE_INTELLIGENCE_RESULT_PAYLOAD_BUDGET_BYTES, 16 * 1024);
}

#[test]
fn phase19_runtime_state_snapshot_and_grace_budgets_are_locked() {
    assert_eq!(RUNTIME_STATE_BROADCAST_CAPACITY, 16);
    assert_eq!(RUNTIME_STATE_SNAPSHOT_MAX_DOCUMENTS, 64);
    assert_eq!(RUNTIME_STATE_SNAPSHOT_MAX_DIAGNOSTICS, 32);
    assert_eq!(RUNTIME_STATE_SNAPSHOT_DIFF_REVIEW_PAYLOAD_BYTES, 768 * 1024);
    assert_eq!(RUNTIME_STATE_INSTALL_DIFF_REVIEW_P95_MS, 16);
    assert_eq!(PREVIOUS_BEHAVIOR_GRACE_MS, 2_000);
    assert_eq!(PREVIOUS_BEHAVIOR_GRACE_MAX_TRANSACTIONS, 256);
    assert_eq!(DEFAULT_MAX_FRAME_SIZE, 1024 * 1024);
    const {
        assert!(RUNTIME_STATE_SNAPSHOT_DIFF_REVIEW_PAYLOAD_BYTES < DEFAULT_MAX_FRAME_SIZE);
    }
}

#[test]
fn fake_server_bridge_matrix_stays_within_deterministic_latency_budget() {
    let started = Instant::now();
    let output = std::process::Command::new("node")
        .args([
            "--test",
            "tests/fixtures/lsp/fake-server/fake-server.test.mjs",
            "tests/fixtures/lsp/fake-server/matrix.test.mjs",
        ])
        .output()
        .expect("node fake-server matrix");
    assert!(
        output.status.success(),
        "fake-server matrix failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "fake-server open/init/request/shutdown matrix must stay under 5s; observed {elapsed:?}"
    );
}
