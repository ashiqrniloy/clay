use clay::client::ClientConnectionEvent;
use clay::editor::EditorSurface;
use clay::masonry_editor::EditorWidget;
use clay::packages::record::assemble_package_record;
use clay::perf::budgets::DECORATION_PAYLOAD_BUDGET_BYTES;
use clay::protocol::{
    BehaviorManifest, DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan,
    DocumentAccess, Modifiers, ServerMessage, TokenType, codec::Codec,
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
        package_prefix: "markdown".to_string(),
        kind: DecorationKind::Syntax,
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

fn two_chunk_comment_editor() -> EditorSurface {
    let package = decoration_package();
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "x".repeat(140),
        DocumentAccess::Editable { lease_id: 1 },
    );
    for (start, end) in [(0, 128), (128, 140)] {
        let set = validate_decoration_publication(
            &package,
            3,
            DecorationSet {
                document_id: 7,
                document_version: 3,
                package_prefix: "markdown".to_string(),
                kind: DecorationKind::Syntax,
                viewport_byte_start: start,
                viewport_byte_end: end,
                spans: vec![DecorationSpan::from_vocabulary(
                    start,
                    end,
                    DecorationKind::Syntax,
                    TokenType::Comment,
                    Modifiers::NONE,
                    70,
                    provenance(),
                )],
            },
        )
        .expect("two-chunk comment fixture validates");
        assert!(editor.apply_decoration_set(set));
    }
    editor
}

fn unpainted_bytes(editor: &EditorSurface, range: std::ops::Range<usize>) -> Vec<usize> {
    let painted = editor.visible_decoration_paint_ranges_for_test();
    range
        .filter(|byte| !painted.iter().any(|(range, _)| range.contains(byte)))
        .collect()
}

fn full_comment_authority(document_version: u64) -> DecorationSet {
    DecorationSet {
        document_id: 7,
        document_version,
        package_prefix: "markdown".to_string(),
        kind: DecorationKind::Syntax,
        viewport_byte_start: 0,
        viewport_byte_end: 128,
        spans: vec![DecorationSpan::from_vocabulary(
            0,
            128,
            DecorationKind::Syntax,
            TokenType::Comment,
            Modifiers::NONE,
            70,
            provenance(),
        )],
    }
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
        package_prefix: "python".to_string(),
        kind: DecorationKind::Syntax,
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
        package_prefix: "markdown".to_string(),
        kind: DecorationKind::Syntax,
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
fn edit_ack_retains_current_chunks_and_rejects_older_publications() {
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
    assert_eq!(editor.decoration_span_count(), 1);
    assert_eq!(editor.decoration_state_version(), Some(4));
    assert!(!editor.apply_decoration_set(set));
}

#[test]
fn plan058_empty_authority_after_insertion_preserves_shifted_right_residual() {
    let package = decoration_package();
    let mut editor = two_chunk_comment_editor();
    assert!(editor.navigate_to_byte_offset(10));
    assert!(editor.insert_text("x"));
    assert!(editor.note_confirmed_version(7, 4));

    let empty = validate_decoration_publication(
        &package,
        4,
        DecorationSet {
            document_id: 7,
            document_version: 4,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 128,
            spans: Vec::new(),
        },
    )
    .expect("empty insertion authority validates");
    assert!(editor.apply_decoration_set(empty));

    let missing = unpainted_bytes(&editor, 128..141);
    assert!(
        missing.is_empty(),
        "authority 0..128 removed provisional bytes outside its viewport: {missing:?}"
    );
}

#[test]
fn plan058_empty_authority_after_deletion_preserves_shifted_right_residual() {
    let package = decoration_package();
    let mut editor = two_chunk_comment_editor();
    assert!(editor.navigate_to_byte_offset(10));
    assert!(editor.backspace());
    assert!(editor.note_confirmed_version(7, 4));

    let empty = validate_decoration_publication(
        &package,
        4,
        DecorationSet {
            document_id: 7,
            document_version: 4,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 128,
            spans: Vec::new(),
        },
    )
    .expect("empty deletion authority validates");
    assert!(editor.apply_decoration_set(empty));

    let missing = unpainted_bytes(&editor, 128..139);
    assert!(
        missing.is_empty(),
        "authority 0..128 removed the right side of an overlapping provisional chunk: {missing:?}"
    );
}

#[test]
fn plan058_repeated_insert_delete_authority_cycles_preserve_boundary_geometry() {
    let package = decoration_package();
    let mut editor = two_chunk_comment_editor();
    let mut document_len = 140;
    let mut version = 3;

    for cycle in 0..128 {
        let _ = editor.navigate_to_byte_offset(10);
        assert!(editor.insert_text("x"));
        document_len += 1;
        version += 1;
        assert!(editor.note_confirmed_version(7, version));
        let authority =
            validate_decoration_publication(&package, version, full_comment_authority(version))
                .expect("insertion authority validates");
        assert!(editor.apply_decoration_set(authority));
        assert!(
            unpainted_bytes(&editor, 0..document_len).is_empty(),
            "cycle {cycle}: insertion exposed base-colored bytes"
        );

        let _ = editor.navigate_to_byte_offset(11);
        assert!(editor.backspace());
        document_len -= 1;
        version += 1;
        assert!(editor.note_confirmed_version(7, version));
        let authority =
            validate_decoration_publication(&package, version, full_comment_authority(version))
                .expect("deletion authority validates");
        assert!(editor.apply_decoration_set(authority));
        assert!(
            unpainted_bytes(&editor, 0..document_len).is_empty(),
            "cycle {cycle}: deletion exposed base-colored bytes"
        );
    }
}

#[test]
fn optimistic_comment_style_outside_authority_survives_exact_replacement() {
    let package = decoration_package();
    let mut set = valid_set(3);
    set.viewport_byte_end = 7;
    set.spans[0].byte_end = 7;
    set.spans[0].token_type = TokenType::Comment;
    let set = validate_decoration_publication(&package, 3, set).unwrap();
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "comment".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    assert!(editor.apply_decoration_set(set));
    for _ in 0..3 {
        assert!(editor.move_right());
    }

    assert!(editor.insert_text("x"));
    assert_eq!(editor.visible_decoration_paint_ranges_for_test()[0].0, 0..8);
    assert!(editor.note_confirmed_version(7, 4));
    let empty = validate_decoration_publication(
        &package,
        4,
        DecorationSet {
            document_id: 7,
            document_version: 4,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 7,
            spans: Vec::new(),
        },
    )
    .unwrap();
    assert!(editor.apply_decoration_set(empty));
    assert_eq!(editor.decoration_span_count(), 1);
    assert_eq!(editor.visible_decoration_paint_ranges_for_test()[0].0, 7..8);

    let tail = validate_decoration_publication(
        &package,
        4,
        DecorationSet {
            document_id: 7,
            document_version: 4,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 7,
            viewport_byte_end: 8,
            spans: Vec::new(),
        },
    )
    .unwrap();
    assert!(editor.apply_decoration_set(tail));
    assert_eq!(editor.decoration_span_count(), 0);
}

#[test]
fn authoritative_syntax_corrects_inherited_suffix_without_clearing_unrelated_spans() {
    let package = decoration_package();
    let initial = validate_decoration_publication(
        &package,
        3,
        DecorationSet {
            document_id: 7,
            document_version: 3,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 8,
            spans: vec![
                DecorationSpan::from_vocabulary(
                    0,
                    4,
                    DecorationKind::Syntax,
                    TokenType::Function,
                    Modifiers::NONE,
                    70,
                    provenance(),
                ),
                DecorationSpan::from_vocabulary(
                    5,
                    8,
                    DecorationKind::Syntax,
                    TokenType::Keyword,
                    Modifiers::NONE,
                    70,
                    provenance(),
                ),
            ],
        },
    )
    .unwrap();
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "main let".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    assert!(editor.apply_decoration_set(initial));
    assert!(editor.navigate_to_byte_offset(4));

    assert!(editor.insert_text("x"));
    let provisional = editor.visible_decoration_paint_ranges_for_test();
    assert!(provisional.iter().any(|(range, _)| range == &(0..5)));
    assert!(provisional.iter().any(|(range, _)| range == &(6..9)));
    assert!(editor.note_confirmed_version(7, 4));

    let authoritative = validate_decoration_publication(
        &package,
        4,
        DecorationSet {
            document_id: 7,
            document_version: 4,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 9,
            spans: vec![DecorationSpan::from_vocabulary(
                6,
                9,
                DecorationKind::Syntax,
                TokenType::Keyword,
                Modifiers::NONE,
                70,
                provenance(),
            )],
        },
    )
    .unwrap();

    assert!(editor.apply_decoration_set(authoritative));
    assert_eq!(editor.decoration_span_count(), 1);
    assert_eq!(
        editor
            .visible_decoration_paint_ranges_for_test()
            .into_iter()
            .map(|(range, _)| range)
            .collect::<Vec<_>>(),
        vec![6..9]
    );
}

#[test]
fn rapid_local_versions_reject_stale_authority_without_losing_provisional_geometry() {
    let package = decoration_package();
    let initial = validate_decoration_publication(
        &package,
        1,
        DecorationSet {
            document_id: 7,
            document_version: 1,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 4,
            spans: vec![DecorationSpan::from_vocabulary(
                0,
                4,
                DecorationKind::Syntax,
                TokenType::Function,
                Modifiers::NONE,
                70,
                provenance(),
            )],
        },
    )
    .unwrap();
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        1,
        "main".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    assert!(editor.apply_decoration_set(initial));
    assert!(editor.navigate_to_byte_offset(4));

    assert!(editor.insert_text("x"));
    assert!(editor.note_confirmed_version(7, 2));
    assert!(editor.insert_text("y"));
    assert!(editor.note_confirmed_version(7, 3));
    assert!(
        editor
            .visible_decoration_paint_ranges_for_test()
            .iter()
            .any(|(range, _)| range == &(0..6))
    );

    let stale = validate_decoration_publication(
        &package,
        2,
        DecorationSet {
            document_id: 7,
            document_version: 2,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 5,
            spans: Vec::new(),
        },
    )
    .unwrap();
    assert!(!editor.apply_decoration_set(stale));
    assert!(
        editor
            .visible_decoration_paint_ranges_for_test()
            .iter()
            .any(|(range, _)| range == &(0..6))
    );

    let current = validate_decoration_publication(
        &package,
        3,
        DecorationSet {
            document_id: 7,
            document_version: 3,
            package_prefix: "markdown".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 6,
            spans: vec![DecorationSpan::from_vocabulary(
                0,
                6,
                DecorationKind::Syntax,
                TokenType::Function,
                Modifiers::NONE,
                70,
                provenance(),
            )],
        },
    )
    .unwrap();
    assert!(editor.apply_decoration_set(current));
    assert_eq!(
        editor
            .visible_decoration_paint_ranges_for_test()
            .into_iter()
            .map(|(range, _)| range)
            .collect::<Vec<_>>(),
        vec![0..6]
    );
}

#[test]
fn empty_syntax_chunk_clears_affected_range() {
    let package = decoration_package();
    let syntax = validate_decoration_publication(&package, 3, valid_set(3)).unwrap();
    let empty = validate_decoration_publication(
        &package,
        3,
        DecorationSet {
            spans: Vec::new(),
            ..syntax.clone()
        },
    )
    .unwrap();
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello markdown".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );

    assert!(editor.apply_decoration_set(syntax));
    assert!(editor.apply_decoration_set(empty));
    assert_eq!(editor.decoration_span_count(), 0);
}

#[test]
fn syntax_chunk_replacement_preserves_semantic_layer() {
    let package = decoration_package();
    let syntax = validate_decoration_publication(&package, 3, valid_set(3)).unwrap();
    let mut semantic = syntax.clone();
    semantic.kind = DecorationKind::Semantic;
    semantic.spans[0].kind = DecorationKind::Semantic;
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello markdown".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );

    assert!(editor.apply_decoration_set(semantic));
    assert!(editor.apply_decoration_set(syntax));
    assert_eq!(editor.decoration_span_count(), 2);
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
