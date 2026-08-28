//! Phase 28 protocol compatibility for folding, link/inlay decorations,
//! completion recency, and inert hover/activation payloads.
//!
//! This suite guards the shared length-prefixed codec. It does not add a
//! serialization path: `DecorationIntent` is deliberately client-local, so
//! its wire data is the inert `DecorationTarget` carried by a decoration span.

use clay::{
    perf::budgets::{DECORATION_PAYLOAD_BUDGET_BYTES, FOLDING_RANGE_PAYLOAD_BUDGET_BYTES},
    protocol::{
        ClientMessage, CompletionProvenance, CompletionReplacementRange, CompletionRequest,
        CompletionTrigger, DecorationKind, DecorationProvenance, DecorationSet, DecorationSpan,
        DecorationTarget, FoldingProvenance, FoldingRange, FoldingRangeSet, HoverResult,
        InlayHintPayload, InlayPlacement, LanguageIntelligenceFeature, LanguageIntelligencePayload,
        LanguageIntelligenceRequest, LanguageIntelligenceResult, LanguageIntelligenceStatus,
        PROTOCOL_VERSION, ServerMessage, TextByteRange, codec::Codec, codec::CodecError,
    },
};

const FRAME_PREFIX_BYTES: usize = 4;

fn payload_len(frame: &[u8]) -> usize {
    frame.len() - FRAME_PREFIX_BYTES
}

fn provenance(prefix: &str) -> DecorationProvenance {
    DecorationProvenance {
        package_name: format!("@clay/{prefix}"),
        package_version: "0.1.0".to_string(),
        package_prefix: prefix.to_string(),
    }
}

fn link_span(byte_start: u64, target: DecorationTarget) -> DecorationSpan {
    DecorationSpan {
        byte_start,
        byte_end: byte_start + 5,
        kind: DecorationKind::Link,
        token_type: clay::protocol::TokenType::Link,
        modifiers: clay::protocol::Modifiers::NONE,
        scope: None,
        font_role: None,
        priority: 80,
        provenance: provenance("markdown"),
        target: Some(target),
        inlay: None,
    }
}

fn link_set() -> DecorationSet {
    DecorationSet {
        document_id: 7,
        document_version: 42,
        package_prefix: "markdown".to_string(),
        kind: DecorationKind::Link,
        viewport_byte_start: 0,
        viewport_byte_end: 64,
        spans: vec![
            link_span(
                0,
                DecorationTarget::WorkspacePath {
                    relative_path: "docs/readme.md".to_string(),
                    range: Some(TextByteRange::new(8, 12)),
                },
            ),
            link_span(
                16,
                DecorationTarget::DocumentRange {
                    range: TextByteRange::new(20, 24),
                },
            ),
            link_span(
                32,
                DecorationTarget::DisplayOnly {
                    text: "https://example.test".to_string(),
                },
            ),
        ],
        trace_id: None,
    }
}

fn inlay_set() -> DecorationSet {
    DecorationSet {
        document_id: 7,
        document_version: 42,
        package_prefix: "lsp-rust".to_string(),
        kind: DecorationKind::InlayHint,
        viewport_byte_start: 0,
        viewport_byte_end: 64,
        spans: vec![DecorationSpan::from_inlay(
            24,
            25,
            InlayHintPayload {
                label: ": i32".to_string(),
                placement: InlayPlacement::After,
            },
            10,
            provenance("lsp-rust"),
        )],
        trace_id: None,
    }
}

fn folding_set() -> FoldingRangeSet {
    FoldingRangeSet {
        document_id: 7,
        document_version: 42,
        package_prefix: "markdown".to_string(),
        ranges: vec![
            FoldingRange {
                byte_start: 0,
                byte_end: 64,
                label: Some("section".to_string()),
                provenance: FoldingProvenance {
                    package_name: "@clay/markdown".to_string(),
                    package_version: "0.1.0".to_string(),
                    package_prefix: "markdown".to_string(),
                },
            },
            FoldingRange {
                byte_start: 8,
                byte_end: 32,
                label: None,
                provenance: FoldingProvenance {
                    package_name: "@clay/markdown".to_string(),
                    package_version: "0.1.0".to_string(),
                    package_prefix: "markdown".to_string(),
                },
            },
        ],
    }
}

#[test]
fn phase28_protocol_version_is_pinned() {
    assert_eq!(PROTOCOL_VERSION, 29);
}

#[test]
fn folding_range_set_round_trips_through_codec_within_budget() {
    let codec = Codec::default();
    let set = folding_set();
    assert!(set.serialized_bytes().unwrap() <= FOLDING_RANGE_PAYLOAD_BUDGET_BYTES);
    let message = ServerMessage::FoldingRangeSet(set);

    let frame = codec.encode_server_message(&message).expect("folds encode");
    assert!(payload_len(&frame) <= Codec::default().max_frame_size());
    assert_eq!(codec.decode_server_message(&frame).unwrap(), message);
}

#[test]
fn link_and_inlay_decoration_messages_round_trip_through_codec() {
    let codec = Codec::default();
    let link = link_set();
    let inlay = inlay_set();
    for set in [&link, &inlay] {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(set).expect("decoration set encodes");
        assert!(bytes.len() <= DECORATION_PAYLOAD_BUDGET_BYTES);
    }

    let messages = [
        ServerMessage::DecorationSet(link.clone()),
        ServerMessage::DecorationBatch(vec![link, inlay]),
    ];
    for message in messages {
        let frame = codec
            .encode_server_message(&message)
            .expect("decorations encode");
        assert!(payload_len(&frame) <= codec.max_frame_size());
        assert_eq!(codec.decode_server_message(&frame).unwrap(), message);
    }

    let oversized_target = DecorationSet {
        document_id: 7,
        document_version: 42,
        package_prefix: "markdown".to_string(),
        kind: DecorationKind::Link,
        viewport_byte_start: 0,
        viewport_byte_end: 8,
        spans: vec![link_span(
            0,
            DecorationTarget::DisplayOnly {
                text: "x".repeat(128),
            },
        )],
        trace_id: None,
    };
    assert!(matches!(
        Codec::new(128).encode_server_message(&ServerMessage::DecorationSet(oversized_target)),
        Err(CodecError::FrameTooLarge { max: 128, .. })
    ));
}

#[test]
fn completion_recency_and_hover_intelligence_messages_round_trip() {
    let codec = Codec::default();
    let completion = ClientMessage::CompletionRequest {
        request: CompletionRequest {
            request_id: 11,
            client_id: 9,
            document_id: 7,
            document_version: 42,
            behavior_version: 3,
            cursor_byte_offset: 12,
            replacement_range: CompletionReplacementRange::new(10, 12),
            trigger: CompletionTrigger::Manual,
            provider_generation: 2,
            recent_completions: vec!["println!".to_string(), "String".to_string()]
                .into_boxed_slice(),
        },
    };
    let hover_request = ClientMessage::LanguageIntelligenceRequest {
        request: LanguageIntelligenceRequest {
            request_id: 12,
            client_id: 9,
            document_id: 7,
            document_version: 42,
            behavior_version: 3,
            cursor_byte_offset: 12,
            feature: LanguageIntelligenceFeature::Hover,
            provider_generation: 2,
        },
    };
    let hover_result = ServerMessage::LanguageIntelligenceResult {
        result: LanguageIntelligenceResult {
            request_id: 12,
            client_id: 9,
            document_id: 7,
            document_version: 42,
            behavior_version: 3,
            provider_generation: 2,
            feature: LanguageIntelligenceFeature::Hover,
            status: LanguageIntelligenceStatus::Ok,
            payload: LanguageIntelligencePayload::Hover(HoverResult {
                range: Some(TextByteRange::new(8, 12)),
                markdown: "A bounded hover result.".to_string(),
            }),
            provenance: CompletionProvenance::builtin_core(),
        },
    };

    for message in [completion, hover_request] {
        let frame = codec
            .encode_client_message(&message)
            .expect("client message encodes");
        assert!(payload_len(&frame) <= codec.max_frame_size());
        assert_eq!(codec.decode_client_message(&frame).unwrap(), message);
    }
    let frame = codec
        .encode_server_message(&hover_result)
        .expect("hover result encodes");
    assert!(payload_len(&frame) <= codec.max_frame_size());
    assert_eq!(codec.decode_server_message(&frame).unwrap(), hover_result);
}

#[test]
fn truncated_invalid_and_oversized_phase28_frames_fail_closed() {
    let codec = Codec::default();
    let valid = codec
        .encode_server_message(&ServerMessage::FoldingRangeSet(folding_set()))
        .expect("fixture encodes");
    let declared = payload_len(&valid);

    let truncated = valid[..valid.len() - 1].to_vec();
    assert!(matches!(
        codec.decode_server_message(&truncated),
        Err(CodecError::LengthMismatch { declared: got, actual })
            if got == declared && actual == truncated.len() - FRAME_PREFIX_BYTES
    ));

    let invalid = [4_u32.to_be_bytes().as_slice(), &[0xde, 0xad, 0xbe, 0xef]].concat();
    let invalid_result = std::panic::catch_unwind(|| codec.decode_server_message(&invalid))
        .expect("invalid archive must not panic");
    assert!(matches!(invalid_result, Err(CodecError::Deserialize(_))));

    let small_codec = Codec::new(64);
    let oversized = (65_u32).to_be_bytes().to_vec();
    assert!(matches!(
        small_codec.decode_server_message(&oversized),
        Err(CodecError::FrameTooLarge { len: 65, max: 64 })
    ));
}
