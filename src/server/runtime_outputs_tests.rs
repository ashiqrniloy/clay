use std::sync::Arc;

use tokio::sync::Mutex;

use super::{ActiveBehaviorManifest, StaticSduiState};
use crate::server::runtime_state::{apply_runtime_outputs, apply_runtime_outputs_without_sdui};
use crate::{
    protocol::{
        BehaviorManifest, DecorationSet, DocumentId, SduiActionIntent, SduiActionSource, SduiNodeId,
    },
    server::{js_runtime::ClayRuntimeEvaluation, sdui::default_document_tree},
};

fn valid_manifest() -> BehaviorManifest {
    let mut manifest = BehaviorManifest::minimal_text_editing(99);
    manifest.manifest_id = "test.manifest".to_string();
    manifest
}

fn empty_decoration_set(document_id: DocumentId) -> DecorationSet {
    DecorationSet {
        document_id,
        document_version: 1,
        package_prefix: "markdown".to_string(),
        kind: crate::protocol::DecorationKind::Syntax,
        viewport_byte_start: 0,
        viewport_byte_end: 0,
        spans: vec![],
        trace_id: None,
    }
}

fn harness(
    document_id: DocumentId,
) -> (
    Arc<Mutex<ActiveBehaviorManifest>>,
    Arc<Mutex<StaticSduiState>>,
) {
    (
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        Arc::new(Mutex::new(StaticSduiState::empty_for_document(document_id))),
    )
}

/// Behavior manifest and SDUI tree are applied to shared state in one
/// primitive, and no diagnostics are produced when both are valid.
#[tokio::test]
async fn apply_runtime_outputs_applies_behavior_and_sdui_to_shared_state() {
    let (behavior, sdui) = harness(1);
    let evaluation = ClayRuntimeEvaluation {
        op_records: vec![],
        published_sdui_tree: Some(default_document_tree(1, 1)),
        published_decoration_set: None,
        published_diagnostic_set: None,
        published_folding_set: None,
        parse_handlers: vec![],
        js_parse_handlers: vec![],
        behavior_manifest: Some(valid_manifest()),
        ui_contributions: Default::default(),
        syntax_grammars: vec![],
        syntax_engine_preferences: Default::default(),
        completion_providers: vec![],
        js_completion_providers: vec![],
        language_intelligence_providers: vec![],
        js_language_intelligence_providers: vec![],
        document_analyzers: vec![],
        active_theme: None,
        active_typography: None,
        active_design_system: None,
        active_icon_pack: None,
        configuration_diagnostics: Vec::new(),
    };

    let application = apply_runtime_outputs(&evaluation, 1, &behavior, &sdui).await;

    assert!(
        application.diagnostics().is_empty(),
        "no diagnostics for valid outputs"
    );
    assert!(
        application.behavior.is_some_and(|r| r.is_ok()),
        "behavior applied"
    );
    assert!(application.sdui.is_some_and(|r| r.is_ok()), "sdui applied");
    assert_eq!(
        behavior.lock().await.version(),
        2,
        "shared behavior advanced"
    );
}

#[tokio::test]
async fn open_time_runtime_sdui_output_does_not_replace_workspace_browser_state() {
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = Arc::new(Mutex::new(StaticSduiState::for_document(1, 1)));
    let evaluation = ClayRuntimeEvaluation {
        op_records: vec![],
        published_sdui_tree: Some(default_document_tree(2, 1)),
        published_decoration_set: Some(empty_decoration_set(1)),
        published_diagnostic_set: None,
        published_folding_set: None,
        parse_handlers: vec![],
        js_parse_handlers: vec![],
        behavior_manifest: Some(valid_manifest()),
        ui_contributions: Default::default(),
        syntax_grammars: vec![],
        syntax_engine_preferences: Default::default(),
        completion_providers: vec![],
        js_completion_providers: vec![],
        language_intelligence_providers: vec![],
        js_language_intelligence_providers: vec![],
        document_analyzers: vec![],
        active_theme: None,
        active_typography: None,
        active_design_system: None,
        active_icon_pack: None,
        configuration_diagnostics: Vec::new(),
    };

    let application = apply_runtime_outputs_without_sdui(&evaluation, &behavior).await;

    assert!(application.sdui.is_none(), "open-time SDUI is ignored");
    assert!(application.behavior.is_some_and(|result| result.is_ok()));
    assert!(application.decorations.is_some());
    sdui.lock()
        .await
        .validate_action(&SduiActionIntent::command(
            "workspace.refresh",
            SduiActionSource::Button {
                node_id: SduiNodeId(5),
            },
        ))
        .expect("original workspace browser action still validates");
}

/// An SDUI tree bound to a different document fails per-document
/// validation and surfaces the unified `sdui.invalid_tree` diagnostic
/// regardless of which flow called the primitive.
#[tokio::test]
async fn apply_runtime_outputs_reports_unified_diagnostic_for_invalid_sdui() {
    let (behavior, sdui) = harness(1);
    // Tree built for document 2, applied against document 1 -> binding
    // validation fails.
    let evaluation = ClayRuntimeEvaluation {
        op_records: vec![],
        published_sdui_tree: Some(default_document_tree(2, 1)),
        published_decoration_set: None,
        published_diagnostic_set: None,
        published_folding_set: None,
        parse_handlers: vec![],
        js_parse_handlers: vec![],
        behavior_manifest: None,
        ui_contributions: Default::default(),
        syntax_grammars: vec![],
        syntax_engine_preferences: Default::default(),
        completion_providers: vec![],
        js_completion_providers: vec![],
        language_intelligence_providers: vec![],
        js_language_intelligence_providers: vec![],
        document_analyzers: vec![],
        active_theme: None,
        active_typography: None,
        active_design_system: None,
        active_icon_pack: None,
        configuration_diagnostics: Vec::new(),
    };

    let application = apply_runtime_outputs(&evaluation, 1, &behavior, &sdui).await;

    assert!(
        matches!(application.sdui, Some(Err(()))),
        "sdui failed validation"
    );
    let diagnostics = application.diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "sdui.invalid_tree");
    assert_eq!(
        behavior.lock().await.version(),
        1,
        "behavior untouched when absent"
    );
}

/// Decoration sets are passed through for the caller to emit; the
/// config-eval boundary holds no decoration store, so they are not applied
/// to shared state here. This makes the previously-silent drop explicit.
#[tokio::test]
async fn apply_runtime_outputs_passes_decorations_through() {
    let (behavior, sdui) = harness(1);
    let set = empty_decoration_set(1);
    let evaluation = ClayRuntimeEvaluation {
        op_records: vec![],
        published_sdui_tree: None,
        published_decoration_set: Some(set.clone()),
        published_diagnostic_set: None,
        published_folding_set: None,
        parse_handlers: vec![],
        js_parse_handlers: vec![],
        behavior_manifest: None,
        ui_contributions: Default::default(),
        syntax_grammars: vec![],
        syntax_engine_preferences: Default::default(),
        completion_providers: vec![],
        js_completion_providers: vec![],
        language_intelligence_providers: vec![],
        js_language_intelligence_providers: vec![],
        document_analyzers: vec![],
        active_theme: None,
        active_typography: None,
        active_design_system: None,
        active_icon_pack: None,
        configuration_diagnostics: Vec::new(),
    };

    let application = apply_runtime_outputs(&evaluation, 1, &behavior, &sdui).await;

    assert_eq!(
        application.decorations,
        Some(set),
        "decorations passed through"
    );
    assert!(application.behavior.is_none(), "no manifest present");
    assert!(application.sdui.is_none(), "no tree present");
    assert!(application.diagnostics().is_empty());
}
