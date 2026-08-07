use clay::{
    client::ClientConnectionEvent,
    editor::EditorSurface,
    masonry_editor::EditorWidget,
    packages::record::assemble_package_record,
    perf::budgets::{
        DECORATION_NEAR_VIEWPORT_GUARD_BYTES, DIAGNOSTIC_MAX_CODE_BYTES,
        DIAGNOSTIC_MAX_MESSAGE_BYTES, DIAGNOSTIC_MAX_SPANS_PER_SET,
    },
    protocol::{
        BehaviorManifest, DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan,
        DiagnosticSet, DiagnosticSeverity, DiagnosticSpan, DocumentAccess, RuntimeDiagnostic,
        ServerMessage, codec::Codec,
    },
    server::diagnostics::{
        DiagnosticValidationError, TREE_SITTER_DIAGNOSTIC_SOURCE, compose_diagnostic_spans,
        validate_diagnostic_publication, validate_diagnostic_set,
    },
};
use serde_json::json;

fn package() -> clay::packages::record::PackageRecord {
    assemble_package_record(&json!({
        "name": "@clay/rust",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "rust",
            "entry": "./dist/index.js",
            "permissions": ["render-decorations"],
            "modes": ["rust"],
            "docs": "./docs/index.md"
        }
    }))
    .expect("diagnostic package fixture validates")
}

fn provenance() -> DecorationProvenance {
    DecorationProvenance {
        package_name: "@clay/rust".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "rust".to_string(),
    }
}

fn span(start: u64, end: u64) -> DiagnosticSpan {
    DiagnosticSpan {
        byte_start: start,
        byte_end: end,
        severity: DiagnosticSeverity::Error,
        code: "syntax.error".to_string(),
        message: "unexpected token".to_string(),
        source: "tree-sitter".to_string(),
        provenance: provenance(),
    }
}

fn set(version: u64) -> DiagnosticSet {
    DiagnosticSet {
        document_id: 7,
        document_version: version,
        viewport_byte_start: 0,
        viewport_byte_end: 64,
        source: "tree-sitter".to_string(),
        provenance: provenance(),
        spans: vec![span(4, 5)],
    }
}

fn analyzer_set(
    version: u64,
    source: &str,
    start: u64,
    end: u64,
    severity: DiagnosticSeverity,
    code: &str,
) -> DiagnosticSet {
    let mut set = set(version);
    set.source = source.to_string();
    set.spans[0].source = source.to_string();
    set.spans[0].byte_start = start;
    set.spans[0].byte_end = end;
    set.spans[0].severity = severity;
    set.spans[0].code = code.to_string();
    set.spans[0].message = code.to_string();
    set
}

#[test]
fn diagnostic_set_round_trips_all_metadata() {
    let message = ServerMessage::DiagnosticSet(set(3));
    let codec = Codec::default();

    let decoded = codec
        .decode_server_message(&codec.encode_server_message(&message).unwrap())
        .unwrap();

    assert_eq!(decoded, message);
}

#[test]
fn range_diagnostic_is_distinct_from_runtime_diagnostic_and_decoration_span() {
    let range = span(4, 5);
    let status = RuntimeDiagnostic::error("syntax.error", "unexpected token");
    let decoration = DecorationSpan::from_style_token(
        4,
        5,
        DecorationKind::Diagnostic,
        "diagnostic.error",
        0,
        provenance(),
    );

    assert_eq!(
        (
            range.source.as_str(),
            status.code.as_str(),
            decoration.byte_start
        ),
        ("tree-sitter", "syntax.error", 4)
    );
}

#[test]
fn diagnostic_validation_rejects_stale_off_viewport_or_invalid_ranges() {
    assert!(matches!(
        validate_diagnostic_publication(&package(), 4, set(3)),
        Err(DiagnosticValidationError::StaleDocumentVersion { .. })
    ));

    let mut off_viewport = set(3);
    off_viewport.spans[0].byte_end = 65;
    assert!(matches!(
        validate_diagnostic_publication(&package(), 3, off_viewport),
        Err(DiagnosticValidationError::SpanOutsideViewport { .. })
    ));

    let mut empty = set(3);
    empty.spans[0].byte_end = empty.spans[0].byte_start;
    assert!(matches!(
        validate_diagnostic_publication(&package(), 3, empty),
        Err(DiagnosticValidationError::InvalidSpanRange { .. })
    ));
}

#[test]
fn diagnostic_validation_bounds_count_fields_and_serialized_payload() {
    let mut too_many = set(3);
    too_many.spans = (0..=DIAGNOSTIC_MAX_SPANS_PER_SET)
        .map(|index| span(index as u64, index as u64 + 1))
        .collect();
    too_many.viewport_byte_end = too_many.spans.len() as u64;
    assert!(matches!(
        validate_diagnostic_set(3, too_many, None),
        Err(DiagnosticValidationError::TooManySpans { .. })
    ));

    let mut oversized_code = set(3);
    oversized_code.spans[0].code = "x".repeat(DIAGNOSTIC_MAX_CODE_BYTES + 1);
    assert!(matches!(
        validate_diagnostic_set(3, oversized_code, None),
        Err(DiagnosticValidationError::FieldTooLong { field: "code", .. })
    ));

    let mut control = set(3);
    control.spans[0].message = "unsafe\nmessage".to_string();
    assert!(matches!(
        validate_diagnostic_set(3, control, None),
        Err(DiagnosticValidationError::ControlCharacter {
            field: "message",
            ..
        })
    ));

    let mut oversized_payload = set(3);
    oversized_payload.spans = (0..10)
        .map(|index| {
            let mut item = span(index, index + 1);
            item.message = "x".repeat(DIAGNOSTIC_MAX_MESSAGE_BYTES);
            item
        })
        .collect();
    assert!(matches!(
        validate_diagnostic_set(3, oversized_payload, None),
        Err(DiagnosticValidationError::PayloadBudgetExceeded { .. })
    ));
}

#[test]
fn diagnostic_validation_rejects_invalid_source_provenance_and_permission() {
    let mut source_mismatch = set(3);
    source_mismatch.spans[0].source = "other".to_string();
    assert!(matches!(
        validate_diagnostic_set(3, source_mismatch, None),
        Err(DiagnosticValidationError::SourceMismatch { .. })
    ));

    let mut provenance_mismatch = set(3);
    provenance_mismatch.spans[0].provenance.package_prefix = "other".to_string();
    assert!(matches!(
        validate_diagnostic_set(3, provenance_mismatch, None),
        Err(DiagnosticValidationError::PackageProvenanceMismatch { .. })
    ));

    let mut no_permission = package();
    no_permission.manifest.clay.permissions.clear();
    assert!(matches!(
        validate_diagnostic_publication(&no_permission, 3, set(3)),
        Err(DiagnosticValidationError::MissingPermission { .. })
    ));
}

#[test]
fn empty_source_chunk_replaces_and_clears_prior_diagnostics() {
    let populated = validate_diagnostic_set(3, set(3), None).unwrap();
    let mut cleared = populated.clone();
    cleared.spans.clear();
    let cleared = validate_diagnostic_set(3, cleared, None).unwrap();

    assert_eq!(populated.chunk_key(), cleared.chunk_key());
    assert!(cleared.spans.is_empty());
}

#[test]
fn diagnostic_set_routes_server_to_client_and_applies_to_matching_document() {
    let package = package();
    let set = validate_diagnostic_publication(&package, 3, set(3)).unwrap();
    let codec = Codec::default();
    let message = ServerMessage::DiagnosticSet(set.clone());
    let decoded = codec
        .decode_server_message(&codec.encode_server_message(&message).unwrap())
        .unwrap();
    let ServerMessage::DiagnosticSet(decoded_set) = decoded else {
        panic!("expected DiagnosticSet");
    };

    let initial_state = clay::client::ClientInitialState {
        client_id: 1,
        document_id: 7,
        document_version: 3,
        text: "hello rust".to_string(),
        access: DocumentAccess::Editable { lease_id: 1 },
        behavior_manifest: BehaviorManifest::minimal_text_editing(0),
        active_theme: clay::protocol::ActiveTheme {
            specifier: "@clay/default".to_string(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        },
        active_typography: clay::protocol::ActiveTypography::default(),
        workspace_root: "/tmp/root".to_string(),
    };
    let mut widget = EditorWidget::with_initial_state(initial_state);

    assert!(widget.apply_connection_event(ClientConnectionEvent::DiagnosticSet(decoded_set)));
    assert_eq!(widget.diagnostic_span_count(), 1);
}

#[test]
fn stale_or_mismatched_diagnostic_set_is_ignored_client_side() {
    let package = package();
    let set = validate_diagnostic_publication(&package, 3, set(3)).unwrap();
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello rust".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );

    assert!(editor.apply_diagnostic_set(set.clone()));
    assert_eq!(editor.diagnostic_span_count(), 1);

    let mut wrong_document = set.clone();
    wrong_document.document_id = 8;
    assert!(!editor.apply_diagnostic_set(wrong_document));
    assert_eq!(editor.diagnostic_span_count(), 1);

    let mut stale = set;
    stale.document_version = 2;
    assert!(!editor.apply_diagnostic_set(stale));
    assert_eq!(editor.diagnostic_span_count(), 1);
}

#[test]
fn multiple_sources_compose_and_empty_source_replacement_clears_only_its_chunk() {
    let package = package();
    let tree_sitter = validate_diagnostic_publication(&package, 3, set(3)).unwrap();
    let mut analyzer = set(3);
    analyzer.source = "analyzer".to_string();
    analyzer.spans[0].source = "analyzer".to_string();
    analyzer.spans[0].byte_start = 10;
    analyzer.spans[0].byte_end = 11;
    let analyzer = validate_diagnostic_publication(&package, 3, analyzer).unwrap();

    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello rust!!".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );

    assert!(editor.apply_diagnostic_set(tree_sitter.clone()));
    assert!(editor.apply_diagnostic_set(analyzer));
    assert_eq!(editor.diagnostic_span_count(), 2);

    let mut clear_tree_sitter = tree_sitter;
    clear_tree_sitter.spans.clear();
    assert!(editor.apply_diagnostic_set(clear_tree_sitter));
    assert_eq!(editor.diagnostic_span_count(), 1);
}

#[test]
fn edit_version_advance_clears_stale_range_diagnostics_before_async_reparse() {
    let package = package();
    let set = validate_diagnostic_publication(&package, 3, set(3)).unwrap();
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello rust".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );

    assert!(editor.apply_diagnostic_set(set.clone()));
    assert_eq!(editor.diagnostic_span_count(), 1);
    assert!(editor.note_confirmed_version(7, 4));
    assert_eq!(editor.diagnostic_span_count(), 0);
    assert!(!editor.apply_diagnostic_set(set));
}

#[test]
fn diagnostic_chunk_cache_stays_near_viewport_and_under_budget() {
    let package = package();
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "x".repeat(2 * 1024 * 1024),
        DocumentAccess::Editable { lease_id: 1 },
    );

    let near = validate_diagnostic_publication(&package, 3, set(3)).unwrap();
    let mut far = set(3);
    far.viewport_byte_start = 1024 * 1024;
    far.viewport_byte_end = 1024 * 1024 + 64;
    far.spans[0].byte_start = far.viewport_byte_start + 4;
    far.spans[0].byte_end = far.viewport_byte_start + 5;
    let far = validate_diagnostic_publication(&package, 3, far).unwrap();

    assert!(editor.apply_diagnostic_set(near));
    assert_eq!(editor.diagnostic_span_count(), 1);
    assert!(editor.apply_diagnostic_set(far.clone()));
    assert_eq!(editor.diagnostic_span_count(), 1);
    assert!(far.viewport_byte_start > DECORATION_NEAR_VIEWPORT_GUARD_BYTES);
    assert_eq!(editor.visible_diagnostic_spans(0, 64).count(), 0);
    assert_eq!(
        editor
            .visible_diagnostic_spans(far.viewport_byte_start, far.viewport_byte_end)
            .count(),
        1
    );
}

#[test]
fn diagnostics_compose_without_erasing_syntax_semantic_or_selection() {
    let package = package();
    let diagnostics = validate_diagnostic_publication(&package, 3, set(3)).unwrap();
    let decorations = DecorationSet {
        document_id: 7,
        document_version: 3,
        package_prefix: "rust".to_string(),
        kind: DecorationKind::Syntax,
        viewport_byte_start: 0,
        viewport_byte_end: 64,
        spans: vec![DecorationSpan::from_style_token(
            0,
            5,
            DecorationKind::Syntax,
            "keyword.control",
            10,
            provenance(),
        )],
    };

    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello rust".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    assert!(editor.apply_decoration_set(decorations));
    assert!(editor.apply_diagnostic_set(diagnostics));

    assert_eq!(editor.decoration_span_count(), 1);
    assert_eq!(editor.diagnostic_span_count(), 1);
    assert_eq!(editor.visible_decoration_paint_ranges_for_test().len(), 1);
    assert_eq!(editor.visible_diagnostic_paint_ranges_for_test().len(), 1);
    assert_ne!(
        editor.visible_decoration_paint_ranges_for_test()[0].1,
        editor.visible_diagnostic_paint_ranges_for_test()[0].1
    );
}

#[test]
fn diagnostic_arrival_requests_render_without_invalidating_text_layout() {
    let package = package();
    let diagnostics = validate_diagnostic_publication(&package, 3, set(3)).unwrap();
    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello rust".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    let before = editor.layout_style_revision_for_test();
    assert!(editor.apply_diagnostic_set(diagnostics));
    assert_eq!(editor.layout_style_revision_for_test(), before);
    assert_eq!(editor.visible_diagnostic_paint_ranges_for_test().len(), 1);
}

#[test]
fn diagnostic_paint_is_viewport_clipped_and_uses_no_hardcoded_color() {
    let package = package();
    let mut warning = set(3);
    warning.spans[0].severity = DiagnosticSeverity::Warning;
    warning.spans[0].byte_start = 0;
    warning.spans[0].byte_end = 5;
    let warning = validate_diagnostic_publication(&package, 3, warning).unwrap();

    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello rust".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    assert!(editor.apply_diagnostic_set(warning));

    let ranges = editor.visible_diagnostic_paint_ranges_for_test();
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].0, 0..5);
    assert_eq!(
        ranges[0].1,
        clay::editor::theme::StyleRegistry::default()
            .diagnostic_style(DiagnosticSeverity::Warning)
            .color
    );

    let layout_src = std::fs::read_to_string("src/editor/layout.rs").unwrap();
    let paint = layout_src
        .split("fn paint_squiggle")
        .nth(1)
        .expect("paint_squiggle");
    assert!(
        !paint.contains("Color::from_rgb"),
        "squiggle paint must take theme color, not hardcoded rgb"
    );
    assert!(paint.contains("scene.stroke"));
}

#[test]
fn invalid_to_valid_edit_clears_squiggle_after_current_parse() {
    let package = package();
    let invalid = validate_diagnostic_publication(&package, 3, set(3)).unwrap();
    let mut clear = set(3);
    clear.spans.clear();
    let clear = validate_diagnostic_publication(&package, 3, clear).unwrap();

    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello rust".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    assert!(editor.apply_diagnostic_set(invalid));
    assert_eq!(editor.diagnostic_span_count(), 1);
    assert_eq!(editor.visible_diagnostic_paint_ranges_for_test().len(), 1);

    assert!(editor.apply_diagnostic_set(clear));
    assert_eq!(editor.diagnostic_span_count(), 0);
    assert!(editor.visible_diagnostic_paint_ranges_for_test().is_empty());
}

#[test]
fn runtime_diagnostics_remain_status_level_and_range_diagnostics_remain_inline() {
    let package = package();
    let range = validate_diagnostic_publication(&package, 3, set(3)).unwrap();
    let initial_state = clay::client::ClientInitialState {
        client_id: 1,
        document_id: 7,
        document_version: 3,
        text: "hello rust".to_string(),
        access: DocumentAccess::Editable { lease_id: 1 },
        behavior_manifest: BehaviorManifest::minimal_text_editing(0),
        active_theme: clay::protocol::ActiveTheme {
            specifier: "@clay/default".to_string(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        },
        active_typography: clay::protocol::ActiveTypography::default(),
        workspace_root: "/tmp/root".to_string(),
    };
    let mut widget = EditorWidget::with_initial_state(initial_state);
    let status_before = widget.status_text();

    assert!(
        widget.apply_connection_event(ClientConnectionEvent::RuntimeDiagnostic(
            RuntimeDiagnostic::error("clay.parse.open_failed", "parse handler failed")
        ))
    );
    assert!(
        widget
            .status_text()
            .contains("Runtime clay.parse.open_failed")
    );
    assert_eq!(widget.diagnostic_span_count(), 0);

    assert!(widget.apply_connection_event(ClientConnectionEvent::DiagnosticSet(range)));
    assert_eq!(widget.diagnostic_span_count(), 1);
    assert!(
        widget
            .status_text()
            .contains("Runtime clay.parse.open_failed"),
        "range diagnostics must not erase status-level RuntimeDiagnostic"
    );
    assert_ne!(widget.status_text(), status_before);
}

#[test]
fn diagnostics_facade_exposes_no_raw_op_or_additional_authority() {
    let facade = std::fs::read_to_string("runtime/js/diagnostics.js").unwrap();
    assert!(facade.contains("export function serverPublishDiagnostics"));
    assert!(facade.contains("op_clay_diagnostics_publish_diagnostics"));
    assert!(!facade.contains("export function op_clay"));
    assert!(!facade.contains("filesystem"));
    assert!(!facade.contains("spawn"));
    assert!(facade.contains("FORBIDDEN_KEYS"));
    for key in ["handler", "callback", "rawOps", "css", "draw"] {
        assert!(
            facade.contains(key),
            "facade must reject authority field {key}"
        );
    }

    let op = std::fs::read_to_string("src/server/ops/diagnostics.rs").unwrap();
    assert!(op.contains("validate_diagnostic_publication"));
    // Provenance/permission checks resolve the host-stamped executing package
    // against the enabled set with an approved render-decorations capability.
    assert!(op.contains("require_current_package_capability"));
    assert!(op.contains("RenderDecorations"));
    assert!(!op.contains("LanguageServer"));
    assert!(!op.contains("std::process"));
}

#[test]
fn lsp_compatible_diagnostic_mapping_preserves_fields_and_source_replacement() {
    // Phase 18.20 handoff: analyzer/LSP publishDiagnostics maps onto DiagnosticSet
    // without a parallel transport. Severity/code/message/source survive validation
    // and empty source-keyed replacement clears only that source's chunk.
    let package = package();
    let mut first = set(5);
    first.source = "analyzer".to_string();
    first.spans[0].source = "analyzer".to_string();
    first.spans[0].severity = DiagnosticSeverity::Warning;
    first.spans[0].code = "unused_mut".to_string();
    first.spans[0].message = "variable does not need to be mutable".to_string();
    let first = validate_diagnostic_publication(&package, 5, first).unwrap();

    assert_eq!(first.source, "analyzer");
    assert_eq!(first.spans[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(first.spans[0].code, "unused_mut");
    assert_eq!(
        first.spans[0].message,
        "variable does not need to be mutable"
    );
    assert_eq!(first.spans[0].source, "analyzer");

    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        5,
        "let mut x = 1;".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    assert!(editor.apply_diagnostic_set(first.clone()));
    assert_eq!(editor.diagnostic_span_count(), 1);

    let mut replacement = first;
    replacement.spans.clear();
    assert!(editor.apply_diagnostic_set(replacement));
    assert_eq!(
        editor.diagnostic_span_count(),
        0,
        "empty source replacement must clear only the analyzer chunk"
    );
}

#[test]
fn overlapping_tree_sitter_recovery_is_suppressed_by_lsp_error_and_warning_only() {
    let package = package();
    let tree_sitter = validate_diagnostic_publication(&package, 3, set(3)).unwrap();
    let overlapping_error = validate_diagnostic_publication(
        &package,
        3,
        analyzer_set(3, "rust-analyzer", 4, 5, DiagnosticSeverity::Error, "E0001"),
    )
    .unwrap();
    let mut non_overlap = set(3);
    non_overlap.spans[0].byte_start = 20;
    non_overlap.spans[0].byte_end = 21;
    let non_overlap = validate_diagnostic_publication(&package, 3, non_overlap).unwrap();
    let info = validate_diagnostic_publication(
        &package,
        3,
        analyzer_set(3, "lsp-markdown", 20, 21, DiagnosticSeverity::Info, "hint"),
    )
    .unwrap();

    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello rust!!!!!!!!!!!!!!!!".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );

    assert!(editor.apply_diagnostic_set(tree_sitter.clone()));
    assert!(editor.apply_diagnostic_set(overlapping_error.clone()));
    assert_eq!(editor.diagnostic_span_count(), 2);
    let visible: Vec<_> = editor
        .visible_diagnostic_spans(0, 64)
        .map(|span| (span.source.as_str(), span.code.as_str()))
        .collect();
    assert_eq!(visible, vec![("rust-analyzer", "E0001")]);

    let mut clear_lsp = overlapping_error;
    clear_lsp.spans.clear();
    assert!(editor.apply_diagnostic_set(clear_lsp));
    assert!(editor.apply_diagnostic_set(non_overlap));
    assert!(editor.apply_diagnostic_set(info));
    let visible: Vec<_> = editor
        .visible_diagnostic_spans(0, 64)
        .map(|span| (span.source.as_str(), span.code.as_str()))
        .collect();
    assert!(
        visible.contains(&(TREE_SITTER_DIAGNOSTIC_SOURCE, "syntax.error")),
        "non-overlapping tree-sitter recovery remains: {visible:?}"
    );
    assert!(
        visible.contains(&("lsp-markdown", "hint")),
        "LSP info remains additive and does not suppress tree-sitter: {visible:?}"
    );
}

#[test]
fn lsp_source_version_replacement_and_empty_clear_are_package_scoped() {
    let package = package();
    let first = validate_diagnostic_publication(
        &package,
        3,
        analyzer_set(3, "rust-analyzer", 4, 5, DiagnosticSeverity::Error, "E0001"),
    )
    .unwrap();
    let mut second = analyzer_set(
        3,
        "rust-analyzer",
        8,
        9,
        DiagnosticSeverity::Warning,
        "W0001",
    );
    second.spans[0].message = "replaced".to_string();
    let second = validate_diagnostic_publication(&package, 3, second).unwrap();
    let peer = validate_diagnostic_publication(
        &package,
        3,
        analyzer_set(3, "typescript", 12, 13, DiagnosticSeverity::Error, "ts2304"),
    )
    .unwrap();

    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "hello rust code!!".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    assert!(editor.apply_diagnostic_set(first));
    assert!(editor.apply_diagnostic_set(peer.clone()));
    assert_eq!(editor.diagnostic_span_count(), 2);
    assert!(editor.apply_diagnostic_set(second.clone()));
    let visible: Vec<_> = editor
        .visible_diagnostic_spans(0, 64)
        .map(|span| (span.source.as_str(), span.code.as_str()))
        .collect();
    assert_eq!(
        visible,
        vec![("rust-analyzer", "W0001"), ("typescript", "ts2304")]
    );

    let mut clear_rust = second;
    clear_rust.spans.clear();
    assert!(editor.apply_diagnostic_set(clear_rust));
    assert_eq!(editor.diagnostic_span_count(), 1);
    assert_eq!(
        editor
            .visible_diagnostic_spans(0, 64)
            .next()
            .map(|span| span.source.as_str()),
        Some("typescript")
    );
}

#[test]
fn compose_diagnostic_spans_is_exported_for_generic_reuse() {
    let tree = span(4, 5);
    let mut lsp = span(4, 5);
    lsp.source = "rust-analyzer".to_string();
    lsp.code = "E0001".to_string();
    lsp.message = "cannot find value".to_string();
    let composed = compose_diagnostic_spans([&tree, &lsp]);
    assert_eq!(composed.len(), 1);
    assert_eq!(composed[0].source, "rust-analyzer");
}
