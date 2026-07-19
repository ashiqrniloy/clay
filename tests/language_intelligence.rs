//! Phase 18.20 engine-neutral language-intelligence protocol and validation.
//!
//! Locks the canonical byte-offset contract, deterministic empty/error states,
//! and validation rejections: stale/oversize/nested payloads, invalid byte
//! ranges, unsafe/external locations, out-of-range active indexes, unknown
//! command targets, and disallowed control characters. Also asserts the generic
//! types carry no LSP/JSON-RPC/URI/UTF-16 fields and round-trip a non-LSP fake
//! analyzer.

use clay::editor::EditorSurface;
use clay::editor::theme::StyleRegistry;
use clay::perf::budgets::*;
use clay::protocol::{
    CodeAction, CodeActionResult, CompletionProvenance, DecorationKind, DecorationProvenance,
    DecorationSet, DecorationSpan, DocumentAccess, EditPreview, GoToDefinitionResult, HoverResult,
    LanguageIntelligenceFeature, LanguageIntelligencePayload, LanguageIntelligenceRejection,
    LanguageIntelligenceRequest, LanguageIntelligenceResult, LanguageIntelligenceStatus, Modifiers,
    ParameterInformation, RangeEdit, SignatureHelpResult, SignatureInformation, TextByteRange,
    TextLocation, TokenType,
};
use clay::server::decorations::{DecorationValidationError, validate_decoration_publication};
use clay::server::language_intelligence::validate_result;

fn core_envelope(
    feature: LanguageIntelligenceFeature,
    payload: LanguageIntelligencePayload,
) -> LanguageIntelligenceResult {
    LanguageIntelligenceResult {
        request_id: 1,
        client_id: 9,
        document_id: 7,
        document_version: 42,
        behavior_version: 3,
        provider_generation: 1,
        feature,
        status: LanguageIntelligenceStatus::Ok,
        payload,
        provenance: CompletionProvenance::builtin_core(),
    }
}

#[test]
fn hover_result_round_trips_and_validates() {
    let result = core_envelope(
        LanguageIntelligenceFeature::Hover,
        LanguageIntelligencePayload::Hover(HoverResult {
            range: Some(TextByteRange::new(10, 14)),
            markdown: "# Rust\n`String` is owned.".to_string(),
        }),
    );
    let archived = rkyv::to_bytes::<rkyv::rancor::Error>(&result).unwrap();
    let restored: LanguageIntelligenceResult =
        rkyv::from_bytes::<_, rkyv::rancor::Error>(&archived).unwrap();
    assert_eq!(restored, result);
    validate_result(&result).unwrap();
}

#[test]
fn definition_result_round_trips_with_open_and_workspace_locations() {
    let result = core_envelope(
        LanguageIntelligenceFeature::GoToDefinition,
        LanguageIntelligencePayload::GoToDefinition(GoToDefinitionResult {
            locations: vec![
                TextLocation::OpenDocument {
                    document_id: 7,
                    range: TextByteRange::new(0, 6),
                },
                TextLocation::WorkspaceFile {
                    workspace_root_id: 1,
                    relative_path: "src/lib.rs".to_string(),
                    range: TextByteRange::new(100, 112),
                },
            ],
        }),
    );
    let archived = rkyv::to_bytes::<rkyv::rancor::Error>(&result).unwrap();
    let restored: LanguageIntelligenceResult =
        rkyv::from_bytes::<_, rkyv::rancor::Error>(&archived).unwrap();
    assert_eq!(restored, result);
    validate_result(&result).unwrap();
}

#[test]
fn code_action_with_inert_edit_preview_round_trips_and_validates() {
    let result = core_envelope(
        LanguageIntelligenceFeature::CodeAction,
        LanguageIntelligencePayload::CodeAction(CodeActionResult {
            actions: vec![CodeAction {
                range: TextByteRange::new(5, 9),
                title: "Extract variable".to_string(),
                command_id: Some("acme.extractVariable".to_string()),
                edit: Some(EditPreview {
                    document_id: 7,
                    document_version: 42,
                    edits: vec![RangeEdit {
                        range: TextByteRange::new(5, 9),
                        replacement: "const x = value;".to_string(),
                    }],
                }),
            }],
        }),
    );
    let archived = rkyv::to_bytes::<rkyv::rancor::Error>(&result).unwrap();
    let restored: LanguageIntelligenceResult =
        rkyv::from_bytes::<_, rkyv::rancor::Error>(&archived).unwrap();
    assert_eq!(restored, result);
    validate_result(&result).unwrap();
}

#[test]
fn signature_help_round_trips_and_validates_active_indexes() {
    let result = core_envelope(
        LanguageIntelligenceFeature::SignatureHelp,
        LanguageIntelligencePayload::SignatureHelp(SignatureHelpResult {
            signatures: vec![SignatureInformation {
                label: "foo(a: i32, b: i32)".to_string(),
                documentation: "Adds two numbers.".to_string(),
                parameters: vec![
                    ParameterInformation {
                        label: "a".to_string(),
                        documentation: "first".to_string(),
                    },
                    ParameterInformation {
                        label: "b".to_string(),
                        documentation: "second".to_string(),
                    },
                ],
            }],
            active_signature: Some(0),
            // Equal to parameter count is allowed (cursor after last param).
            active_parameter: Some(2),
        }),
    );
    let archived = rkyv::to_bytes::<rkyv::rancor::Error>(&result).unwrap();
    let restored: LanguageIntelligenceResult =
        rkyv::from_bytes::<_, rkyv::rancor::Error>(&archived).unwrap();
    assert_eq!(restored, result);
    validate_result(&result).unwrap();
}

#[test]
fn request_round_trips_through_rkyv() {
    let request = LanguageIntelligenceRequest {
        request_id: 5,
        client_id: 9,
        document_id: 7,
        document_version: 42,
        behavior_version: 3,
        cursor_byte_offset: 100,
        feature: LanguageIntelligenceFeature::SignatureHelp,
        provider_generation: 2,
    };
    let archived = rkyv::to_bytes::<rkyv::rancor::Error>(&request).unwrap();
    let restored: LanguageIntelligenceRequest =
        rkyv::from_bytes::<_, rkyv::rancor::Error>(&archived).unwrap();
    assert_eq!(restored, request);
}

#[test]
fn unordered_byte_range_is_rejected() {
    let result = core_envelope(
        LanguageIntelligenceFeature::Hover,
        LanguageIntelligencePayload::Hover(HoverResult {
            range: Some(TextByteRange::new(20, 10)),
            markdown: String::new(),
        }),
    );
    assert!(matches!(
        validate_result(&result),
        Err(LanguageIntelligenceRejection::UnorderedByteRange { .. })
    ));
}

#[test]
fn too_many_definition_locations_is_rejected() {
    let locations: Vec<TextLocation> = (0..(LANGUAGE_INTELLIGENCE_MAX_DEFINITION_LOCATIONS + 1))
        .map(|i| TextLocation::OpenDocument {
            document_id: 7,
            range: TextByteRange::new(i as u64, i as u64),
        })
        .collect();
    let result = core_envelope(
        LanguageIntelligenceFeature::GoToDefinition,
        LanguageIntelligencePayload::GoToDefinition(GoToDefinitionResult { locations }),
    );
    assert!(matches!(
        validate_result(&result),
        Err(LanguageIntelligenceRejection::TooManyDefinitionLocations { .. })
    ));
}

#[test]
fn active_signature_out_of_range_is_rejected() {
    let result = core_envelope(
        LanguageIntelligenceFeature::SignatureHelp,
        LanguageIntelligencePayload::SignatureHelp(SignatureHelpResult {
            signatures: vec![SignatureInformation {
                label: "foo()".to_string(),
                documentation: String::new(),
                parameters: Vec::new(),
            }],
            active_signature: Some(5),
            active_parameter: None,
        }),
    );
    assert!(matches!(
        validate_result(&result),
        Err(LanguageIntelligenceRejection::ActiveSignatureOutOfRange { .. })
    ));
}

#[test]
fn active_parameter_strictly_over_count_is_rejected() {
    let result = core_envelope(
        LanguageIntelligenceFeature::SignatureHelp,
        LanguageIntelligencePayload::SignatureHelp(SignatureHelpResult {
            signatures: vec![SignatureInformation {
                label: "foo(a)".to_string(),
                documentation: String::new(),
                parameters: vec![ParameterInformation {
                    label: "a".to_string(),
                    documentation: String::new(),
                }],
            }],
            active_signature: None,
            active_parameter: Some(3),
        }),
    );
    assert!(matches!(
        validate_result(&result),
        Err(LanguageIntelligenceRejection::ActiveParameterOutOfRange { .. })
    ));
}

#[test]
fn empty_code_action_title_is_rejected() {
    let result = core_envelope(
        LanguageIntelligenceFeature::CodeAction,
        LanguageIntelligencePayload::CodeAction(CodeActionResult {
            actions: vec![CodeAction {
                range: TextByteRange::new(0, 1),
                title: String::new(),
                command_id: None,
                edit: None,
            }],
        }),
    );
    assert!(matches!(
        validate_result(&result),
        Err(LanguageIntelligenceRejection::EmptyCodeActionTitle)
    ));
}

#[test]
fn empty_command_id_is_rejected() {
    let result = core_envelope(
        LanguageIntelligenceFeature::CodeAction,
        LanguageIntelligencePayload::CodeAction(CodeActionResult {
            actions: vec![CodeAction {
                range: TextByteRange::new(0, 1),
                title: "Run".to_string(),
                command_id: Some(String::new()),
                edit: None,
            }],
        }),
    );
    assert!(matches!(
        validate_result(&result),
        Err(LanguageIntelligenceRejection::EmptyCommandId)
    ));
}

#[test]
fn unsafe_relative_paths_are_rejected() {
    for path in ["../escape", "/etc/passwd", "a/../../b", "C:/x", "a\\..\\b"] {
        let result = core_envelope(
            LanguageIntelligenceFeature::GoToDefinition,
            LanguageIntelligencePayload::GoToDefinition(GoToDefinitionResult {
                locations: vec![TextLocation::WorkspaceFile {
                    workspace_root_id: 1,
                    relative_path: path.to_string(),
                    range: TextByteRange::new(0, 1),
                }],
            }),
        );
        assert!(
            matches!(
                validate_result(&result),
                Err(LanguageIntelligenceRejection::UnsafeRelativePath { .. })
                    | Err(LanguageIntelligenceRejection::EmptyRelativePath)
            ),
            "unsafe path {path:?} should be rejected"
        );
    }
}

#[test]
fn workspace_root_id_zero_is_rejected() {
    let result = core_envelope(
        LanguageIntelligenceFeature::GoToDefinition,
        LanguageIntelligencePayload::GoToDefinition(GoToDefinitionResult {
            locations: vec![TextLocation::WorkspaceFile {
                workspace_root_id: 0,
                relative_path: "src/lib.rs".to_string(),
                range: TextByteRange::new(0, 1),
            }],
        }),
    );
    assert!(matches!(
        validate_result(&result),
        Err(LanguageIntelligenceRejection::UnsafeRelativePath { .. })
    ));
}

#[test]
fn control_characters_in_hover_markdown_are_rejected() {
    let result = core_envelope(
        LanguageIntelligenceFeature::Hover,
        LanguageIntelligencePayload::Hover(HoverResult {
            range: None,
            markdown: "bad\x07bell".to_string(),
        }),
    );
    assert!(matches!(
        validate_result(&result),
        Err(LanguageIntelligenceRejection::ControlCharactersInField { .. })
    ));
}

#[test]
fn oversize_hover_markdown_is_rejected() {
    let result = core_envelope(
        LanguageIntelligenceFeature::Hover,
        LanguageIntelligencePayload::Hover(HoverResult {
            range: None,
            markdown: "x".repeat(LANGUAGE_INTELLIGENCE_MAX_HOVER_MARKDOWN_CHARS + 1),
        }),
    );
    assert!(matches!(
        validate_result(&result),
        Err(LanguageIntelligenceRejection::FieldTooLong { .. })
    ));
}

#[test]
fn inert_edit_preview_cannot_auto_apply_or_bypass_version_checks() {
    // The edit preview is inert data bound to a stale version; validation
    // accepts it as a preview but the coordinator stale-drops the version.
    // This test locks that the preview carries no executable/command field
    // beyond an optional separate command_id and a bounded inert edit list.
    let result = core_envelope(
        LanguageIntelligenceFeature::CodeAction,
        LanguageIntelligencePayload::CodeAction(CodeActionResult {
            actions: vec![CodeAction {
                range: TextByteRange::new(0, 3),
                title: "Rename".to_string(),
                command_id: None,
                edit: Some(EditPreview {
                    document_id: 7,
                    document_version: 999,
                    edits: vec![RangeEdit {
                        range: TextByteRange::new(0, 3),
                        replacement: "renamed".to_string(),
                    }],
                }),
            }],
        }),
    );
    validate_result(&result).unwrap();
}

#[test]
fn generic_types_carry_no_lsp_or_jsonrpc_fields_and_work_for_fake_analyzer() {
    // A non-LSP fake analyzer (e.g. a Rust regex-based provider) constructs the
    // same canonical byte-offset results without any LSP/URI/line-encoding.
    let result = core_envelope(
        LanguageIntelligenceFeature::Hover,
        LanguageIntelligencePayload::Hover(HoverResult {
            range: Some(TextByteRange::new(0, 4)),
            markdown: "fake analyzer: this is `name`".to_string(),
        }),
    );
    let archived = rkyv::to_bytes::<rkyv::rancor::Error>(&result).unwrap();
    // The serialized payload must contain no LSP/JSON-RPC/URI/UTF-16 markers.
    let as_text = String::from_utf8_lossy(&archived);
    for forbidden in [
        "file://",
        "Content-Length",
        "jsonrpc",
        "utf-16",
        "line",
        "character",
    ] {
        assert!(
            !as_text.contains(forbidden),
            "language-intelligence payload must not carry LSP field marker {forbidden:?}"
        );
    }
    validate_result(&result).unwrap();
}

// ── Task 7: provider registry / cancellable coordinator ─────────────────────

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use clay::packages::record::assemble_package_record;
use clay::server::language_intelligence::{
    LanguageIntelligenceCoordinator, LanguageIntelligenceCoordinatorError,
    LanguageIntelligenceDocumentWindow, LanguageIntelligenceProviderError,
    LanguageIntelligenceProviderMeta, LanguageIntelligenceProviderRegistry,
    LanguageIntelligenceProviderRegistryError,
};
use serde_json::json;

fn intelligence_package(name: &str, api_prefix: &str) -> clay::packages::record::PackageRecord {
    assemble_package_record(&json!({
        "name": name,
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": api_prefix,
            "entry": "./dist/index.js",
            "permissions": ["parse-document"],
            "modes": [api_prefix],
            "docs": "./docs/index.md",
            "contributions": {
                "languageIntelligenceProviders": [{
                    "id": format!("{api_prefix}.intelligence"),
                    "modes": [api_prefix],
                    "features": ["hover", "definition", "codeAction", "signatureHelp"],
                    "priority": 10,
                    "module": "./provider.js",
                    "exportName": "provideLanguageIntelligence",
                    "timeoutMs": 500
                }]
            }
        }
    }))
    .expect("language-intelligence package fixture validates")
}

fn request(
    feature: LanguageIntelligenceFeature,
    generation: u64,
    request_id: u64,
) -> LanguageIntelligenceRequest {
    LanguageIntelligenceRequest {
        request_id,
        client_id: 9,
        document_id: 7,
        document_version: 42,
        behavior_version: 3,
        cursor_byte_offset: 4,
        feature,
        provider_generation: generation,
    }
}

fn window_for(req: &LanguageIntelligenceRequest, mode: &str) -> LanguageIntelligenceDocumentWindow {
    LanguageIntelligenceDocumentWindow {
        document_id: req.document_id,
        document_version: req.document_version,
        behavior_version: req.behavior_version,
        byte_start: 0,
        byte_end: 11,
        text: "hello world".to_string(),
        active_mode: mode.to_string(),
    }
}

fn hover_result(req: &LanguageIntelligenceRequest, markdown: &str) -> LanguageIntelligenceResult {
    LanguageIntelligenceResult {
        request_id: req.request_id,
        client_id: req.client_id,
        document_id: req.document_id,
        document_version: req.document_version,
        behavior_version: req.behavior_version,
        provider_generation: req.provider_generation,
        feature: req.feature,
        status: LanguageIntelligenceStatus::Ok,
        payload: LanguageIntelligencePayload::Hover(HoverResult {
            range: Some(TextByteRange::new(0, 5)),
            markdown: markdown.to_string(),
        }),
        provenance: CompletionProvenance::builtin_core(),
    }
}

fn ok_provider(
    markdown: &'static str,
) -> impl Fn(
    LanguageIntelligenceRequest,
    LanguageIntelligenceDocumentWindow,
) -> std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<LanguageIntelligenceResult, LanguageIntelligenceProviderError>,
            > + Send,
    >,
> + Send
+ Sync
+ 'static {
    move |req, _window| {
        let result = hover_result(&req, markdown);
        Box::pin(async move { Ok(result) })
    }
}

#[test]
fn language_intelligence_contribution_requires_parse_document_and_features() {
    let package = intelligence_package("@org/intel", "intel");
    assert_eq!(
        package.contributions.language_intelligence_providers.len(),
        1
    );
    let descriptor = &package.contributions.language_intelligence_providers[0];
    assert_eq!(descriptor.id, "intel.intelligence");
    assert_eq!(descriptor.features.len(), 4);
    assert_eq!(descriptor.export_name, "provideLanguageIntelligence");

    let missing_permission = assemble_package_record(&json!({
        "name": "@org/intel",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "intel",
            "entry": "./dist/index.js",
            "permissions": [],
            "modes": ["intel"],
            "docs": "./docs/index.md",
            "contributions": {
                "languageIntelligenceProviders": [{
                    "id": "intel.intelligence",
                    "features": ["hover"],
                    "timeoutMs": 500
                }]
            }
        }
    }))
    .unwrap_err();
    assert!(format!("{missing_permission:?}").contains("parse-document"));
}

#[test]
fn registry_priority_mode_and_feature_selection_is_deterministic() {
    let mut registry = LanguageIntelligenceProviderRegistry::new();
    registry
        .register_builtin(
            LanguageIntelligenceProviderMeta::builtin_core(
                "low",
                vec![LanguageIntelligenceFeature::Hover],
                1,
                500,
                1,
            ),
            ok_provider("low"),
        )
        .unwrap();
    let mut high = LanguageIntelligenceProviderMeta::builtin_core(
        "high",
        vec![
            LanguageIntelligenceFeature::Hover,
            LanguageIntelligenceFeature::GoToDefinition,
        ],
        10,
        500,
        1,
    );
    high.modes = vec!["rust".to_string()];
    registry
        .register_builtin(high, ok_provider("high"))
        .unwrap();

    let ordered: Vec<_> = registry
        .providers_for_feature(LanguageIntelligenceFeature::Hover, "rust")
        .into_iter()
        .map(|meta| meta.id.as_str())
        .collect();
    assert_eq!(ordered, vec!["core.high", "core.low"]);

    let definition_only: Vec<_> = registry
        .providers_for_feature(LanguageIntelligenceFeature::GoToDefinition, "rust")
        .into_iter()
        .map(|meta| meta.id.as_str())
        .collect();
    assert_eq!(definition_only, vec!["core.high"]);

    let no_mode_match: Vec<_> = registry
        .providers_for_feature(LanguageIntelligenceFeature::GoToDefinition, "markdown")
        .into_iter()
        .map(|meta| meta.id.as_str())
        .collect();
    assert!(no_mode_match.is_empty());
}

#[test]
fn registry_rejects_duplicate_reserved_and_permissionless_providers() {
    let mut registry = LanguageIntelligenceProviderRegistry::new();
    registry
        .register_builtin(
            LanguageIntelligenceProviderMeta::builtin_core(
                "hover",
                vec![LanguageIntelligenceFeature::Hover],
                1,
                500,
                1,
            ),
            ok_provider("a"),
        )
        .unwrap();
    let duplicate = registry
        .register_builtin(
            LanguageIntelligenceProviderMeta::builtin_core(
                "hover",
                vec![LanguageIntelligenceFeature::Hover],
                1,
                500,
                1,
            ),
            ok_provider("b"),
        )
        .unwrap_err();
    assert!(matches!(
        duplicate,
        LanguageIntelligenceProviderRegistryError::ProviderAlreadyRegistered { .. }
    ));

    let package = intelligence_package("@org/intel", "intel");
    let mut reserved = LanguageIntelligenceProviderMeta {
        id: "clay.reserved".to_string(),
        provenance: CompletionProvenance {
            package_name: package.manifest.name.clone(),
            package_version: package.manifest.version.clone(),
            package_prefix: package.manifest.clay.api_prefix.clone(),
        },
        modes: Vec::new(),
        features: vec![LanguageIntelligenceFeature::Hover],
        priority: 1,
        timeout_ms: 500,
        generation: 1,
    };
    let err = registry
        .register_package(&package, reserved.clone(), ok_provider("x"))
        .unwrap_err();
    assert!(matches!(
        err,
        LanguageIntelligenceProviderRegistryError::ReservedClayNamespace { .. }
    ));

    let no_perm = assemble_package_record(&json!({
        "name": "@org/noperm",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "noperm",
            "entry": "./dist/index.js",
            "permissions": [],
            "modes": ["noperm"],
            "docs": "./docs/index.md",
            "contributions": {}
        }
    }))
    .unwrap();
    reserved.id = "noperm.intelligence".to_string();
    reserved.provenance.package_prefix = "noperm".to_string();
    reserved.provenance.package_name = "@org/noperm".to_string();
    let err = registry
        .register_package(&no_perm, reserved, ok_provider("x"))
        .unwrap_err();
    assert!(matches!(
        err,
        LanguageIntelligenceProviderRegistryError::MissingPermission { .. }
    ));
}

#[tokio::test]
async fn rust_and_package_fake_providers_publish_identical_validated_results() {
    let coordinator = LanguageIntelligenceCoordinator::new();
    coordinator
        .register_builtin(
            LanguageIntelligenceProviderMeta::builtin_core(
                "fake",
                vec![LanguageIntelligenceFeature::Hover],
                1,
                500,
                1,
            ),
            ok_provider("identical hover"),
        )
        .unwrap();

    let package = intelligence_package("@org/intel", "intel");
    let meta = LanguageIntelligenceProviderMeta {
        id: "intel.intelligence".to_string(),
        provenance: CompletionProvenance {
            package_name: package.manifest.name.clone(),
            package_version: package.manifest.version.clone(),
            package_prefix: package.manifest.clay.api_prefix.clone(),
        },
        modes: vec!["intel".to_string()],
        features: vec![LanguageIntelligenceFeature::Hover],
        priority: 5,
        timeout_ms: 500,
        generation: 1,
    };
    coordinator
        .register_package(&package, meta, ok_provider("identical hover"))
        .unwrap();

    let rust_req = request(LanguageIntelligenceFeature::Hover, 1, 1);
    let rust_result = coordinator
        .schedule(
            Some("core.fake"),
            rust_req.clone(),
            window_for(&rust_req, ""),
        )
        .unwrap()
        .await
        .expect("rust result");
    validate_result(&rust_result).unwrap();

    let js_shaped_req = request(LanguageIntelligenceFeature::Hover, 1, 2);
    let package_result = coordinator
        .schedule(
            Some("intel.intelligence"),
            js_shaped_req.clone(),
            window_for(&js_shaped_req, "intel"),
        )
        .unwrap()
        .await
        .expect("package result");
    validate_result(&package_result).unwrap();

    assert_eq!(
        rust_result.payload, package_result.payload,
        "Rust fake and resolver-validated package fake must produce identical payloads"
    );
    assert_eq!(rust_result.status, LanguageIntelligenceStatus::Ok);
    assert_eq!(package_result.status, LanguageIntelligenceStatus::Ok);
    assert_eq!(
        package_result.provenance.package_prefix, "intel",
        "package provenance must be preserved"
    );
}

#[tokio::test]
async fn newer_request_cancels_superseded_work_for_same_client_document_feature() {
    let coordinator = LanguageIntelligenceCoordinator::new();
    let started = Arc::new(AtomicU64::new(0));
    let started_clone = Arc::clone(&started);
    coordinator
        .register_builtin(
            LanguageIntelligenceProviderMeta::builtin_core(
                "slow",
                vec![LanguageIntelligenceFeature::Hover],
                1,
                5_000,
                1,
            ),
            move |req, _window| {
                started_clone.fetch_add(1, Ordering::SeqCst);
                let result = hover_result(&req, "slow");
                Box::pin(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    Ok(result)
                })
            },
        )
        .unwrap();

    let first = request(LanguageIntelligenceFeature::Hover, 1, 1);
    let second = request(LanguageIntelligenceFeature::Hover, 1, 2);
    let _superseded_rx = coordinator
        .schedule(Some("core.slow"), first.clone(), window_for(&first, ""))
        .unwrap();
    let published_rx = coordinator
        .schedule(Some("core.slow"), second.clone(), window_for(&second, ""))
        .unwrap();

    let published = published_rx.await.expect("newer result");
    assert_eq!(published.request_id, 2);
    let stats = coordinator.stats();
    assert!(stats.cancelled_superseded_tasks >= 1);
    assert_eq!(stats.published_results, 1);
}

#[tokio::test]
async fn provider_generation_bump_stale_drops_older_results() {
    let coordinator = LanguageIntelligenceCoordinator::new();
    coordinator
        .register_builtin(
            LanguageIntelligenceProviderMeta::builtin_core(
                "gen",
                vec![LanguageIntelligenceFeature::Hover],
                1,
                5_000,
                1,
            ),
            |req, _window| {
                let result = hover_result(&req, "old-gen");
                Box::pin(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    Ok(result)
                })
            },
        )
        .unwrap();

    let old = request(LanguageIntelligenceFeature::Hover, 1, 1);
    let _stale_rx = coordinator
        .schedule(Some("core.gen"), old.clone(), window_for(&old, ""))
        .unwrap();
    coordinator.bump_generation(old.document_id, 2);
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let stats = coordinator.stats();
    assert_eq!(stats.published_results, 0);
    assert!(stats.stale_results_rejected + stats.cancelled_superseded_tasks >= 1);
}

#[tokio::test]
async fn timeout_publishes_sanitized_status_without_provider_leakage() {
    let coordinator = LanguageIntelligenceCoordinator::new();
    coordinator
        .register_builtin(
            LanguageIntelligenceProviderMeta::builtin_core(
                "timeout",
                vec![LanguageIntelligenceFeature::Hover],
                1,
                20,
                1,
            ),
            |req, _window| {
                let result = hover_result(&req, "/secret/path stderr boom");
                Box::pin(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    Ok(result)
                })
            },
        )
        .unwrap();

    let req = request(LanguageIntelligenceFeature::Hover, 1, 11);
    let published = coordinator
        .schedule(Some("core.timeout"), req.clone(), window_for(&req, ""))
        .unwrap()
        .await
        .expect("timeout result");
    assert_eq!(published.status, LanguageIntelligenceStatus::Timeout);
    assert_eq!(published.request_id, 11);
    match &published.payload {
        LanguageIntelligencePayload::Hover(hover) => assert!(hover.markdown.is_empty()),
        other => panic!("expected empty hover payload, got {other:?}"),
    }
    let encoded = format!("{published:?}");
    assert!(!encoded.contains("/secret/path"));
    assert!(!encoded.contains("stderr"));
}

#[tokio::test]
async fn schedule_returns_immediately_and_provider_without_language_server_cannot_claim_process() {
    let coordinator = LanguageIntelligenceCoordinator::new();
    let package = intelligence_package("@org/intel", "intel");
    assert!(
        !package
            .manifest
            .clay
            .permissions
            .contains(&clay::packages::permissions::PackagePermission::LanguageServer),
        "inert analysis providers must not require language-server"
    );
    let meta = LanguageIntelligenceProviderMeta {
        id: "intel.intelligence".to_string(),
        provenance: CompletionProvenance {
            package_name: package.manifest.name.clone(),
            package_version: package.manifest.version.clone(),
            package_prefix: package.manifest.clay.api_prefix.clone(),
        },
        modes: vec!["intel".to_string()],
        features: vec![LanguageIntelligenceFeature::Hover],
        priority: 1,
        timeout_ms: 500,
        generation: 1,
    };
    coordinator
        .register_package(&package, meta, ok_provider("no process"))
        .unwrap();

    let req = request(LanguageIntelligenceFeature::Hover, 1, 3);
    let started = std::time::Instant::now();
    let published_rx = coordinator
        .schedule(
            Some("intel.intelligence"),
            req.clone(),
            window_for(&req, "intel"),
        )
        .unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_millis(50),
        "schedule must return without waiting on provider work"
    );
    let published = published_rx.await.unwrap();
    assert_eq!(published.status, LanguageIntelligenceStatus::Ok);
}

#[tokio::test]
async fn outstanding_request_limit_fails_closed() {
    let coordinator = LanguageIntelligenceCoordinator::new();
    coordinator
        .register_builtin(
            LanguageIntelligenceProviderMeta::builtin_core(
                "hold",
                vec![LanguageIntelligenceFeature::Hover],
                1,
                5_000,
                1,
            ),
            |req, _window| {
                let result = hover_result(&req, "hold");
                Box::pin(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    Ok(result)
                })
            },
        )
        .unwrap();

    for request_id in 1..=LANGUAGE_INTELLIGENCE_MAX_OUTSTANDING_REQUESTS as u64 {
        let mut req = request(LanguageIntelligenceFeature::Hover, 1, request_id);
        // Distinct clients so supersede does not cancel prior outstanding work.
        req.client_id = request_id;
        coordinator
            .schedule(Some("core.hold"), req.clone(), window_for(&req, ""))
            .unwrap();
    }
    let overflow = request(LanguageIntelligenceFeature::Hover, 1, 99);
    let err = coordinator
        .schedule(
            Some("core.hold"),
            overflow.clone(),
            window_for(&overflow, ""),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        LanguageIntelligenceCoordinatorError::OutstandingRequestLimit { .. }
    ));
}

#[test]
fn built_in_language_intelligence_commands_are_discoverable_and_feature_mapped() {
    use clay::client::language_intelligence_feature_for_command;
    use clay::protocol::{BehaviorManifest, LanguageIntelligenceFeature};
    use clay::server::command_execution::{builtin_server_command, builtin_server_command_ids};

    let manifest = BehaviorManifest::minimal_text_editing(1);
    for (command_id, feature) in [
        ("clay.language.hover", LanguageIntelligenceFeature::Hover),
        (
            "clay.language.goToDefinition",
            LanguageIntelligenceFeature::GoToDefinition,
        ),
        (
            "clay.language.codeActions",
            LanguageIntelligenceFeature::CodeAction,
        ),
        (
            "clay.language.signatureHelp",
            LanguageIntelligenceFeature::SignatureHelp,
        ),
    ] {
        assert!(
            manifest
                .commands
                .iter()
                .any(|command| command.command_id == command_id),
            "{command_id} must be discoverable in the default behavior manifest"
        );
        assert_eq!(
            language_intelligence_feature_for_command(command_id),
            Some(feature)
        );
        assert!(
            builtin_server_command_ids().contains(&command_id),
            "{command_id} must be listed for Control Center discovery"
        );
        assert!(builtin_server_command(command_id).is_some());
    }

    // Empty default key bindings: none of the four commands are bound by default.
    for command_id in [
        "clay.language.hover",
        "clay.language.goToDefinition",
        "clay.language.codeActions",
        "clay.language.signatureHelp",
    ] {
        assert!(
            !manifest
                .keymaps
                .iter()
                .any(|binding| binding.command_id == command_id),
            "{command_id} must ship with empty default key bindings"
        );
    }
}

#[test]
fn language_intelligence_feature_for_command_rejects_unknown_ids() {
    use clay::client::language_intelligence_feature_for_command;
    assert!(language_intelligence_feature_for_command("clay.language.rename").is_none());
    assert!(language_intelligence_feature_for_command("completion.trigger").is_none());
}

fn semantic_package(
    name: &str,
    prefix: &str,
    permissions: &[&str],
) -> clay::packages::record::PackageRecord {
    assemble_package_record(&json!({
        "name": name,
        "version": "1.0.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": prefix,
            "entry": "./dist/index.js",
            "permissions": permissions,
            "modes": [prefix],
            "docs": "./docs/index.md"
        }
    }))
    .expect("semantic package fixture validates")
}

fn semantic_provenance(name: &str, prefix: &str) -> DecorationProvenance {
    DecorationProvenance {
        package_name: name.to_string(),
        package_version: "1.0.0".to_string(),
        package_prefix: prefix.to_string(),
    }
}

#[test]
fn semantic_span_refines_syntax_while_syntax_chunk_remains_theme_resolved() {
    let syntax_package = semantic_package("@org/syntax", "syntaxpkg", &["render-decorations"]);
    let semantic_package =
        semantic_package("@org/semantic", "semanticpkg", &["render-decorations"]);

    let syntax = validate_decoration_publication(
        &syntax_package,
        3,
        DecorationSet {
            document_id: 7,
            document_version: 3,
            package_prefix: "syntaxpkg".to_string(),
            kind: DecorationKind::Syntax,
            viewport_byte_start: 0,
            viewport_byte_end: 12,
            spans: vec![DecorationSpan::from_vocabulary(
                0,
                8,
                DecorationKind::Syntax,
                TokenType::Variable,
                Modifiers::NONE,
                10,
                semantic_provenance("@org/syntax", "syntaxpkg"),
            )],
        },
    )
    .expect("syntax publication validates");

    let semantic = validate_decoration_publication(
        &semantic_package,
        3,
        DecorationSet {
            document_id: 7,
            document_version: 3,
            package_prefix: "semanticpkg".to_string(),
            kind: DecorationKind::Semantic,
            viewport_byte_start: 0,
            viewport_byte_end: 12,
            spans: vec![DecorationSpan::from_vocabulary(
                0,
                8,
                DecorationKind::Semantic,
                TokenType::Variable,
                Modifiers::READONLY | Modifiers::DECLARATION,
                10,
                semantic_provenance("@org/semantic", "semanticpkg"),
            )],
        },
    )
    .expect("semantic publication validates");

    let mut editor = EditorSurface::default();
    editor.load_snapshot(
        7,
        3,
        "readonly_x".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    );
    assert!(editor.apply_decoration_set(syntax));
    assert_eq!(editor.decoration_span_count(), 1);
    assert!(editor.apply_decoration_set(semantic));
    // Additive chunks: syntax remains retained beside semantic.
    assert_eq!(editor.decoration_span_count(), 2);

    let theme = StyleRegistry::default();
    let syntax_color = theme
        .style_for(DecorationKind::Syntax, TokenType::Variable, Modifiers::NONE)
        .color;
    let semantic_style = theme.style_for(
        DecorationKind::Semantic,
        TokenType::Variable,
        Modifiers::READONLY | Modifiers::DECLARATION,
    );
    assert_eq!(
        semantic_style.color, syntax_color,
        "semantic intelligence reuses the Syntax TokenType theme table"
    );
    // Readonly/Declaration are semantic specificity bits; Bold/Italic drive text attrs.
    assert!(!semantic_style.bold);
    assert!(!semantic_style.italic);

    let paint = editor.visible_decoration_paint_ranges_for_test();
    assert!(
        paint
            .iter()
            .any(|(range, color)| *range == (0..8) && *color == syntax_color),
        "composed paint must theme-resolve the refined range: {paint:?}"
    );
}

#[test]
fn semantic_publication_rejects_stale_invalid_forged_and_oversize_payloads() {
    let package = semantic_package("@org/semantic", "semanticpkg", &["render-decorations"]);
    let provenance = semantic_provenance("@org/semantic", "semanticpkg");
    let valid = DecorationSet {
        document_id: 7,
        document_version: 3,
        package_prefix: "semanticpkg".to_string(),
        kind: DecorationKind::Semantic,
        viewport_byte_start: 0,
        viewport_byte_end: 32,
        spans: vec![DecorationSpan::from_vocabulary(
            4,
            12,
            DecorationKind::Semantic,
            TokenType::Function,
            Modifiers::DECLARATION,
            20,
            provenance.clone(),
        )],
    };

    assert!(matches!(
        validate_decoration_publication(&package, 4, valid.clone()).unwrap_err(),
        DecorationValidationError::StaleDocumentVersion { .. }
    ));

    let mut invalid_range = valid.clone();
    invalid_range.spans[0].byte_end = invalid_range.spans[0].byte_start;
    assert!(matches!(
        validate_decoration_publication(&package, 3, invalid_range).unwrap_err(),
        DecorationValidationError::InvalidSpanRange { .. }
    ));

    let mut forged = valid.clone();
    forged.spans[0].provenance.package_prefix = "other".to_string();
    assert!(matches!(
        validate_decoration_publication(&package, 3, forged).unwrap_err(),
        DecorationValidationError::PackageProvenanceMismatch { .. }
    ));

    let mut oversize = valid;
    // Keep ranges valid inside the viewport while blowing the payload budget.
    oversize.spans = (0..400)
        .map(|index| {
            let start = (index % 28) as u64;
            DecorationSpan::from_vocabulary(
                start,
                start + 2,
                DecorationKind::Semantic,
                TokenType::Variable,
                Modifiers::NONE,
                index as u16,
                provenance.clone(),
            )
        })
        .collect();
    assert!(matches!(
        validate_decoration_publication(&package, 3, oversize).unwrap_err(),
        DecorationValidationError::PayloadBudgetExceeded { .. }
    ));
}

#[test]
fn language_server_permission_does_not_bypass_render_decorations() {
    let package = assemble_package_record(&json!({
        "name": "@org/lspbridge",
        "version": "1.0.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "lspbridge",
            "entry": "./dist/index.js",
            "loadEntry": "./dist/load.js",
            "capabilities": ["language-server"],
            "modes": [],
            "docs": "./docs/index.md",
            "contributions": {
                "languageServers": [{
                    "id": "lspbridge.server",
                    "executable": "/bin/true",
                    "args": ["--stdio"]
                }]
            }
        }
    }))
    .expect("language-server-only package validates");
    let set = DecorationSet {
        document_id: 7,
        document_version: 1,
        package_prefix: "lspbridge".to_string(),
        kind: DecorationKind::Semantic,
        viewport_byte_start: 0,
        viewport_byte_end: 8,
        spans: vec![DecorationSpan::from_vocabulary(
            0,
            4,
            DecorationKind::Semantic,
            TokenType::Class,
            Modifiers::NONE,
            1,
            semantic_provenance("@org/lspbridge", "lspbridge"),
        )],
    };
    assert!(matches!(
        validate_decoration_publication(&package, 1, set).unwrap_err(),
        DecorationValidationError::MissingPermission { .. }
    ));
}
