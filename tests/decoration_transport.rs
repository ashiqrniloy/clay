use clay::client::ClientConnectionEvent;
use clay::editor::EditorSurface;
use clay::masonry_editor::EditorWidget;
use clay::packages::record::assemble_package_record;
use clay::perf::budgets::DECORATION_PAYLOAD_BUDGET_BYTES;
use clay::protocol::{
    BehaviorManifest, DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan,
    DocumentAccess, ServerMessage, TokenType, codec::Codec,
};
use clay::server::decorations::{DecorationValidationError, validate_decoration_publication};
use serde_json::json;

fn decoration_package() -> clay::packages::record::PackageRecord {
    decoration_package_with_identity("@clay/markdown", "markdown", "markdown")
}

fn decoration_package_with_identity(
    name: &str,
    api_prefix: &str,
    mode_id: &str,
) -> clay::packages::record::PackageRecord {
    assemble_package_record(&json!({
        "name": name,
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": api_prefix,
            "entry": "./dist/index.js",
            "permissions": ["render-decorations"],
            "modes": [mode_id],
            "docs": "./docs/index.md",
            "contributions": {
                "decorations": [{ "primitiveId": format!("{api_prefix}.syntax"), "kind": "syntax" }]
            }
        }
    }))
    .expect("decoration package fixture validates")
}

fn provenance() -> DecorationProvenance {
    DecorationProvenance {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "markdown".to_string(),
    }
}

fn valid_set(document_version: u64) -> DecorationSet {
    decoration_set_for_range(document_version, 0, 64)
}

fn decoration_set_for_range(
    document_version: u64,
    byte_start: u64,
    byte_end: u64,
) -> DecorationSet {
    DecorationSet {
        document_id: 7,
        document_version,
        viewport_byte_start: byte_start,
        viewport_byte_end: byte_end,
        spans: vec![DecorationSpan::from_style_token(
            byte_start,
            (byte_start + 5).min(byte_end),
            DecorationKind::Syntax,
            "markup.heading.1",
            10,
            provenance(),
        )],
    }
}

fn byte_range(text: &str, needle: &str) -> (u64, u64) {
    let start = text.find(needle).expect("fixture needle must exist");
    let end = start + needle.len();
    (start as u64, end as u64)
}

#[test]
fn decoration_payload_rejected_when_exceeding_budget() {
    let package = decoration_package();
    let mut set = valid_set(3);
    set.spans = (0..DECORATION_PAYLOAD_BUDGET_BYTES as u64)
        .map(|offset| {
            DecorationSpan::from_style_token(
                offset,
                offset + 1,
                DecorationKind::Syntax,
                "markup.heading.1",
                10,
                provenance(),
            )
        })
        .collect();
    set.viewport_byte_end = DECORATION_PAYLOAD_BUDGET_BYTES as u64 + 1;

    let error = validate_decoration_publication(&package, 3, set).unwrap_err();
    assert!(matches!(
        error,
        DecorationValidationError::PayloadBudgetExceeded { .. }
    ));
}

#[test]
fn decoration_rejected_for_stale_document_version() {
    let package = decoration_package();
    let error = validate_decoration_publication(&package, 4, valid_set(3)).unwrap_err();

    assert_eq!(
        error,
        DecorationValidationError::StaleDocumentVersion {
            decoration_version: 3,
            current_version: 4,
        }
    );
}

#[test]
fn decoration_update_rejects_invalid_ranges_and_unknown_tokens() {
    let package = decoration_package();

    let mut invalid_range = valid_set(3);
    invalid_range.spans[0].byte_end = invalid_range.spans[0].byte_start;
    assert!(matches!(
        validate_decoration_publication(&package, 3, invalid_range).unwrap_err(),
        DecorationValidationError::InvalidSpanRange { .. }
    ));

    let mut unknown_token = valid_set(3);
    unknown_token.spans[0].scope = Some("css:color:red".to_string());
    assert!(matches!(
        validate_decoration_publication(&package, 3, unknown_token).unwrap_err(),
        DecorationValidationError::UnknownStyleToken { .. }
    ));
}

#[test]
fn generic_decoration_publication_accepts_language_package_spans() {
    let package = decoration_package_with_identity("@clay/python", "python", "python");
    let provenance = DecorationProvenance {
        package_name: "@clay/python".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "python".to_string(),
    };
    let set = DecorationSet {
        document_id: 11,
        document_version: 4,
        viewport_byte_start: 0,
        viewport_byte_end: 32,
        spans: vec![
            DecorationSpan::from_style_token(
                0,
                3,
                DecorationKind::Syntax,
                "keyword.control",
                70,
                provenance.clone(),
            ),
            DecorationSpan::from_style_token(
                8,
                15,
                DecorationKind::Syntax,
                "string.quoted",
                60,
                provenance,
            ),
        ],
    };

    let validated = validate_decoration_publication(&package, 4, set).unwrap();

    assert_eq!(validated.spans.len(), 2);
    assert!(
        validated
            .spans
            .iter()
            .all(|span| span.provenance.package_prefix == "python")
    );
}

#[test]
fn markdown_representative_decoration_payload_fits_budget_and_client_applies() {
    let package = decoration_package();
    let text = "# Hé 🦀\n\nSome **bold** and *em* and `code`.\n\n```rust\nfn main() {}\n```\n\n1. item\n- other\n";
    let full_viewport_end = text.len() as u64;
    let spans = [
        ("# Hé 🦀", "markup.heading.1", 90),
        ("**bold**", "markup.strong", 60),
        ("*em*", "markup.emphasis", 50),
        ("`code`", "markup.inline-code", 65),
        ("```rust\nfn main() {}\n```", "markup.code-block", 70),
        ("1.", "markup.list-marker", 80),
        ("-", "markup.list-marker", 80),
    ]
    .into_iter()
    .map(|(needle, style_token, priority)| {
        let (byte_start, byte_end) = byte_range(text, needle);
        DecorationSpan::from_style_token(
            byte_start,
            byte_end,
            DecorationKind::Syntax,
            style_token,
            priority,
            provenance(),
        )
    })
    .collect();
    let set = DecorationSet {
        document_id: 7,
        document_version: 3,
        viewport_byte_start: 0,
        viewport_byte_end: full_viewport_end,
        spans,
    };

    let validated = validate_decoration_publication(&package, 3, set).unwrap();
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&validated)
        .expect("representative markdown decoration set serializes")
        .len();
    assert!(
        bytes <= DECORATION_PAYLOAD_BUDGET_BYTES,
        "markdown decorations {bytes} must fit budget {DECORATION_PAYLOAD_BUDGET_BYTES}"
    );

    let initial_state = clay::client::ClientInitialState {
        client_id: 1,
        document_id: 7,
        document_version: 3,
        text: text.to_string(),
        access: DocumentAccess::Editable { lease_id: 1 },
        behavior_manifest: BehaviorManifest::minimal_text_editing(0),
        active_theme: clay::protocol::ActiveTheme {
            specifier: "@clay/default".to_string(),
            overrides: Vec::new(),
        },
        active_typography: clay::protocol::ActiveTypography::default(),
    };
    let mut widget = EditorWidget::with_initial_state(initial_state);
    assert!(widget.apply_connection_event(ClientConnectionEvent::DecorationSet(validated)));
    assert_eq!(widget.decoration_span_count(), 7);
}

#[test]
fn markdown_decoration_update_rejects_off_viewport_spans() {
    let package = decoration_package();
    let mut set = valid_set(3);
    set.viewport_byte_start = 10;
    set.viewport_byte_end = 20;

    assert!(matches!(
        validate_decoration_publication(&package, 3, set).unwrap_err(),
        DecorationValidationError::SpanOutsideViewport { index: 0 }
    ));
}

#[test]
fn decoration_chunk_updates_stay_under_payload_budget() {
    let package = decoration_package();
    let set = validate_decoration_publication(&package, 3, valid_set(3)).unwrap();
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&set)
        .expect("decoration chunk serializes")
        .len();

    assert!(bytes <= DECORATION_PAYLOAD_BUDGET_BYTES);
    assert!(set.spans.iter().all(|span| {
        span.byte_start >= set.viewport_byte_start && span.byte_end <= set.viewport_byte_end
    }));
}

#[test]
fn stale_decoration_chunks_are_ignored_after_edit() {
    let package = decoration_package();
    let set = validate_decoration_publication(&package, 3, valid_set(3)).unwrap();
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello markdown".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );

    assert!(editor.apply_decoration_set(set.clone()));
    assert_eq!(editor.decoration_span_count(), 1);
    assert!(editor.note_confirmed_version(7, 4));
    assert_eq!(editor.decoration_span_count(), 0);
    assert!(!editor.apply_decoration_set(set));
}

#[test]
fn client_decoration_cache_keeps_near_viewport_chunks_only() {
    let package = decoration_package();
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "x".repeat(2 * 1024 * 1024),
        DocumentAccess::Editable { lease_id: 1 },
    );

    let near =
        validate_decoration_publication(&package, 3, decoration_set_for_range(3, 0, 64)).unwrap();
    let far = validate_decoration_publication(
        &package,
        3,
        decoration_set_for_range(3, 1024 * 1024, 1024 * 1024 + 64),
    )
    .unwrap();

    assert!(editor.apply_decoration_set(near));
    assert_eq!(editor.decoration_span_count(), 1);
    assert!(editor.apply_decoration_set(far));
    assert_eq!(editor.decoration_span_count(), 1);
}

#[test]
fn decoration_transport_round_trips_through_protocol_codec() {
    let codec = Codec::default();
    let message = ServerMessage::DecorationSet(valid_set(3));

    let frame = codec.encode_server_message(&message).unwrap();
    let decoded = codec.decode_server_message(&frame).unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn two_axis_decoration_span_round_trips_token_type_modifiers_and_scope() {
    // Plan 046 task 3: lock the two-axis shape through the codec. The span
    // carries `token_type` + `modifiers` + optional `scope` instead of the
    // old free-form `style_token`; rkyv must preserve all three fields and the
    // compat mapper must reproduce the closed-vocabulary classification.
    use clay::protocol::Modifiers;
    let mut set = valid_set(3);
    set.spans[0].token_type = TokenType::Heading1;
    set.spans[0].modifiers = Modifiers::BOLD | Modifiers::ITALIC;
    set.spans[0].scope = Some("markup.heading.1".to_string());

    let codec = Codec::default();
    let frame = codec
        .encode_server_message(&ServerMessage::DecorationSet(set.clone()))
        .unwrap();
    let decoded = codec.decode_server_message(&frame).unwrap();
    let decoded_set = match decoded {
        ServerMessage::DecorationSet(s) => s,
        other => panic!("expected DecorationSet, got {other:?}"),
    };

    assert_eq!(
        decoded_set, set,
        "codec preserves token_type+modifiers+scope"
    );
    assert_eq!(decoded_set.spans[0].token_type, TokenType::Heading1);
    assert!(decoded_set.spans[0].modifiers.contains(Modifiers::BOLD));
    assert!(decoded_set.spans[0].modifiers.contains(Modifiers::ITALIC));
    assert_eq!(
        decoded_set.spans[0].scope.as_deref(),
        Some("markup.heading.1")
    );

    // Compat mapper reproduces the closed-vocabulary classification for the
    // baseline families so existing packages render unchanged.
    assert_eq!(
        TokenType::classify_style_token("keyword.control"),
        (TokenType::Keyword, Modifiers::NONE)
    );
    assert_eq!(
        TokenType::classify_style_token("markup.strong"),
        (TokenType::Paragraph, Modifiers::BOLD)
    );
    assert_eq!(
        TokenType::classify_style_token("punctuation.definition"),
        (TokenType::Operator, Modifiers::NONE)
    );
}

#[test]
fn decoration_render_hook_applies_validated_spans_without_package_js() {
    let package = decoration_package();
    let set = validate_decoration_publication(&package, 3, valid_set(3)).unwrap();
    let initial_state = clay::client::ClientInitialState {
        client_id: 1,
        document_id: 7,
        document_version: 3,
        text: "hello markdown".to_string(),
        access: DocumentAccess::Editable { lease_id: 1 },
        behavior_manifest: BehaviorManifest::minimal_text_editing(0),
        active_theme: clay::protocol::ActiveTheme {
            specifier: "@clay/default".to_string(),
            overrides: Vec::new(),
        },
        active_typography: clay::protocol::ActiveTypography::default(),
    };
    let mut widget = EditorWidget::with_initial_state(initial_state);

    assert!(widget.apply_connection_event(ClientConnectionEvent::DecorationSet(set)));

    assert_eq!(widget.decoration_span_count(), 1);
}
