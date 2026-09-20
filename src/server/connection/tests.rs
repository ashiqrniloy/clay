use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc, time::SystemTime};

use crate::packages::commands::CommandRegistry;
use crate::protocol::ViewportRenderStatus;
use crate::protocol::{AgentServerMessage, KeyBindingContext, KeyCode};

use tokio::{
    io::duplex,
    sync::Mutex,
    time::{Duration, timeout},
};

use super::{
    RuntimeDiagnosticStore, handle_connection, route_connection_tab_state, session_bound_message,
};
// Moved family helpers (Plan 090 task 2) are glob re-exported in the
// connection module scope; the few names tests also import explicitly are
// imported from their family modules for unambiguous unqualified use.
use super::runtime::{
    execute_command_intent, language_intelligence_document_window,
    language_intelligence_document_window_for_behavior, sdui_command_request,
    static_package_completion_result,
};
use super::tabs::open_workspace_for_bound_tab;
use crate::protocol::ParseByteRange;
use crate::server::command_execution::{CommandExecutionRequest, CommandExecutionTarget};

fn workspace_state() -> Arc<Mutex<WorkspaceState>> {
    Arc::new(Mutex::new(WorkspaceState::new()))
}

fn sdui_state() -> Arc<Mutex<StaticSduiState>> {
    Arc::new(Mutex::new(StaticSduiState::for_document(1, 1)))
}

fn document_state() -> Arc<Mutex<DocumentState>> {
    Arc::new(Mutex::new(DocumentState::new(
        1,
        "".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )))
}

/// Document under test for window builders: they read the rope, not the id, so
/// the request's document id is what matters.
fn document_with_text(text: &str) -> DocumentState {
    DocumentState::new(
        1,
        text.to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )
}

fn empty_sdui_state() -> Arc<Mutex<StaticSduiState>> {
    Arc::new(Mutex::new(StaticSduiState::empty_for_document(1)))
}

fn runtime_diagnostics() -> Arc<Mutex<RuntimeDiagnosticStore>> {
    Arc::new(Mutex::new(RuntimeDiagnosticStore::default()))
}

fn active_theme_state() -> Arc<Mutex<Option<crate::protocol::ActiveTheme>>> {
    Arc::new(Mutex::new(None))
}

fn js_runtime() -> ClayJsRuntimeService {
    ClayJsRuntimeService::default()
}

#[test]
fn language_intelligence_window_uses_active_behavior_mode() {
    let mut manifest = BehaviorManifest::minimal_text_editing(3);
    manifest.manifest_id = "rust.rust".to_string();
    let behavior = ActiveBehaviorManifest::new(manifest).unwrap();
    let request = crate::protocol::LanguageIntelligenceRequest {
        request_id: 1,
        client_id: 2,
        document_id: 3,
        document_version: 4,
        behavior_version: 3,
        cursor_byte_offset: 1,
        feature: crate::protocol::LanguageIntelligenceFeature::Hover,
        provider_generation: 0,
    };

    let window = language_intelligence_document_window_for_behavior(
        &request,
        &document_with_text("fn main() {}"),
        behavior.manifest(),
    );

    assert_eq!(window.active_mode, "rust");
}

#[test]
fn language_intelligence_window_resolves_per_document_mode_layer() {
    // Phase 22.2: two documents in different modes; the window builder
    // must use each document's OWN layer's mode, not a connection-wide
    // latest.
    let mut state = ActiveBehaviorManifest::default();
    let mut markdown = BehaviorManifest::minimal_text_editing(1);
    markdown.manifest_id = "markdown.markdown".to_string();
    markdown.scope = crate::protocol::BehaviorScope::Document { document_id: 7 };
    let mut rust = BehaviorManifest::minimal_text_editing(1);
    rust.manifest_id = "rust.rust".to_string();
    rust.scope = crate::protocol::BehaviorScope::Document { document_id: 9 };
    state.publish_replacement(markdown).unwrap();
    state.publish_replacement(rust).unwrap();

    let request = |document_id| crate::protocol::LanguageIntelligenceRequest {
        request_id: 1,
        client_id: 2,
        document_id,
        document_version: 4,
        behavior_version: 3,
        cursor_byte_offset: 1,
        feature: crate::protocol::LanguageIntelligenceFeature::Hover,
        provider_generation: 0,
    };

    let markdown_window = language_intelligence_document_window_for_behavior(
        &request(7),
        &document_with_text("## Heading"),
        state.manifest_for(7),
    );
    assert_eq!(markdown_window.active_mode, "markdown");

    let rust_window = language_intelligence_document_window_for_behavior(
        &request(9),
        &document_with_text("fn main() {}"),
        state.manifest_for(9),
    );
    assert_eq!(rust_window.active_mode, "rust");
}

/// Plan 126 task 3: the static completion path reads only the replacement
/// range, so a 4 MiB document produces exactly the small-document result.
#[test]
fn static_completion_on_large_document_matches_small_document_results() {
    let provenance = crate::protocol::CompletionProvenance {
        package_name: "@clay/javascript".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "javascript".to_string(),
    };
    let provider = crate::server::completion::CompletionProviderMeta {
        id: "javascript.keywords".to_string(),
        provenance: provenance.clone(),
        priority: 0,
        exclusive: false,
        trigger_metadata: crate::server::completion::CompletionTriggerMetadata {
            trigger_characters: vec![".".to_string()],
        },
        word_boundary: crate::server::completion::WordBoundaryRule::default(),
        items: ["function", "for", "return"]
            .into_iter()
            .map(|item| crate::protocol::CompletionItem::new(item, item, provenance.clone()))
            .collect(),
        timeout_ms: 300,
        max_items: 32,
        generation: 0,
    };
    let request = crate::protocol::CompletionRequest {
        request_id: 1,
        client_id: 2,
        document_id: 3,
        document_version: 4,
        behavior_version: 5,
        cursor_byte_offset: 2,
        replacement_range: crate::protocol::CompletionReplacementRange::new(0, 2),
        trigger: crate::protocol::CompletionTrigger::Character(".".to_string()),
        provider_generation: 0,
        recent_completions: Vec::<String>::new().into_boxed_slice(),
    };
    // Same two-byte replacement range at the head of both documents.
    let small = document_with_text("fu");
    let large = document_with_text(&format!("fu{}", "x".repeat(4 * 1024 * 1024)));
    let result_of = |document: &DocumentState| {
        let replacement_text = document
            .text_range(
                request.replacement_range.byte_start,
                request.replacement_range.byte_end,
            )
            .expect("replacement range is inside the document");
        static_package_completion_result(
            &request,
            "javascript.javascript",
            &replacement_text,
            std::slice::from_ref(&provider),
        )
        .expect("static provider matches the replacement range")
    };

    let small_result = result_of(&small);
    let large_result = result_of(&large);

    assert_eq!(small_result.status, large_result.status);
    assert_eq!(small_result.provenance, large_result.provenance);
    assert_eq!(large_result.items, small_result.items);
    assert_eq!(
        large_result
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["function"]
    );
    assert_eq!(
        large_result.replacement_range,
        small_result.replacement_range
    );
}

/// Plan 126 task 3: the language-intelligence window is built from the rope,
/// stays inside its budget, and lands on scalar boundaries on a multi-MiB
/// multibyte document.
#[test]
fn language_intelligence_window_budget_honored() {
    use crate::perf::budgets::LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES;

    let text = "日本語 🦀 fn main() {}\n".repeat(4 * 1024 * 1024 / 28);
    let document = document_with_text(&text);
    let cursor_byte_offset = (text.len() / 2) as u64;
    let request = crate::protocol::LanguageIntelligenceRequest {
        request_id: 1,
        client_id: 2,
        document_id: 3,
        document_version: 4,
        behavior_version: 3,
        cursor_byte_offset,
        feature: crate::protocol::LanguageIntelligenceFeature::Hover,
        provider_generation: 0,
    };

    let window = language_intelligence_document_window(&request, &document, "rust");

    assert!(
        window.text.len() <= LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES + 3,
        "window of {} bytes exceeds the {} byte budget",
        window.text.len(),
        LANGUAGE_INTELLIGENCE_DOCUMENT_WINDOW_BUDGET_BYTES
    );
    assert_eq!(
        window.byte_end - window.byte_start,
        window.text.len() as u64
    );
    assert!(window.byte_start <= cursor_byte_offset);
    assert!(cursor_byte_offset <= window.byte_end);
    // Indexing panics unless both bounds are scalar boundaries; the equality
    // then proves the window is the document's own text, uncorrupted.
    assert_eq!(
        window.text,
        text[window.byte_start as usize..window.byte_end as usize]
    );
}

#[test]
fn static_package_completion_filters_active_provider_items_by_prefix() {
    let provenance = crate::protocol::CompletionProvenance {
        package_name: "@clay/javascript".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "javascript".to_string(),
    };
    let provider = crate::server::completion::CompletionProviderMeta {
        id: "javascript.keywords".to_string(),
        provenance: provenance.clone(),
        priority: 0,
        exclusive: false,
        trigger_metadata: crate::server::completion::CompletionTriggerMetadata {
            trigger_characters: vec![".".to_string()],
        },
        word_boundary: crate::server::completion::WordBoundaryRule::default(),
        items: ["function", "for", "return"]
            .into_iter()
            .map(|item| crate::protocol::CompletionItem::new(item, item, provenance.clone()))
            .collect(),
        timeout_ms: 300,
        max_items: 32,
        generation: 0,
    };
    let request = crate::protocol::CompletionRequest {
        request_id: 1,
        client_id: 2,
        document_id: 3,
        document_version: 4,
        behavior_version: 5,
        cursor_byte_offset: 2,
        replacement_range: crate::protocol::CompletionReplacementRange::new(0, 2),
        trigger: crate::protocol::CompletionTrigger::Character(".".to_string()),
        provider_generation: 0,
        recent_completions: Vec::<String>::new().into_boxed_slice(),
    };

    let result =
        static_package_completion_result(&request, "javascript.javascript", "fu", &[provider])
            .unwrap();

    assert_eq!(
        result
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        vec!["function"]
    );
    assert_eq!(result.provenance, provenance);
}

#[test]
fn static_package_completion_equal_priority_merge_uses_shared_score() {
    let provenance = crate::protocol::CompletionProvenance {
        package_name: "@clay/rust".to_string(),
        package_version: "0.1.0".to_string(),
        package_prefix: "rust".to_string(),
    };
    let provider = |id: &str, item: crate::protocol::CompletionItem| {
        crate::server::completion::CompletionProviderMeta {
            id: id.to_string(),
            provenance: provenance.clone(),
            priority: 0,
            exclusive: false,
            trigger_metadata: crate::server::completion::CompletionTriggerMetadata {
                trigger_characters: vec![".".to_string()],
            },
            word_boundary: crate::server::completion::WordBoundaryRule::default(),
            items: vec![item],
            timeout_ms: 300,
            max_items: 32,
            generation: 0,
        }
    };
    let keyword = crate::protocol::CompletionItem::new("fn", "fn", provenance.clone());
    let snippet = crate::protocol::CompletionItem::new(
        "fn",
        "fn ${1:name}(${2:args}) {\n\t$0\n}",
        provenance.clone(),
    )
    .with_snippet();
    let providers = [
        provider("rust.keywords", keyword),
        provider("rust.snippets", snippet),
    ];
    let request = crate::protocol::CompletionRequest {
        request_id: 1,
        client_id: 2,
        document_id: 3,
        document_version: 4,
        behavior_version: 5,
        cursor_byte_offset: 2,
        replacement_range: crate::protocol::CompletionReplacementRange::new(0, 2),
        trigger: crate::protocol::CompletionTrigger::Character(".".to_string()),
        provider_generation: 0,
        recent_completions: vec!["fn ${1:name}(${2:args}) {\n\t$0\n}".to_string()]
            .into_boxed_slice(),
    };

    let result = static_package_completion_result(&request, "rust.rust", "fn", &providers).unwrap();

    assert_eq!(result.items.len(), 2);
    assert_eq!(
        result.items[0].text_format,
        crate::protocol::CompletionItemTextFormat::Snippet
    );
    assert_eq!(
        result.items[1].text_format,
        crate::protocol::CompletionItemTextFormat::PlainText
    );
    assert!(result.validate().is_ok());
}

fn runtime_generation() -> super::RuntimeGenerationStore {
    runtime_generation_from(js_runtime())
}

fn runtime_generation_from(runtime: ClayJsRuntimeService) -> super::RuntimeGenerationStore {
    super::RuntimeGenerationStore {
        current: Arc::new(Mutex::new(super::super::RuntimeGeneration {
            id: 1,
            service: runtime,
            evaluation: None,
            diagnostics: Vec::new(),
        })),
        typography: super::super::ActiveTypographyState::default(),
        runtime_state: super::super::ActiveRuntimeStateFanout::default(),
        behavior_grace: super::super::behavior::BehaviorGraceState::new(),
    }
}

fn parse_coordinator() -> ParseCoordinator {
    ParseCoordinator::default()
}

fn language_intelligence_coordinator() -> LanguageIntelligenceCoordinator {
    LanguageIntelligenceCoordinator::new()
}

async fn load_markdown_runtime(
    runtime: &ClayJsRuntimeService,
    coordinator: &ParseCoordinator,
    behavior: &Arc<Mutex<ActiveBehaviorManifest>>,
    sdui: &Arc<Mutex<StaticSduiState>>,
) {
    let evaluation = runtime
        .evaluate_controlled_module(
            r#"import { loadPackage } from "clay:packages";
import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
await loadPackage("@clay/markdown");
const classification = serverClassifyDocument({ documentId: 1, path: "README.md" });
serverActivateClassifiedMode(classification, { path: "README.md" });"#,
        )
        .await
        .expect("Markdown package load should evaluate");
    runtime
        .register_parse_handlers(coordinator, 1, &evaluation)
        .expect("Markdown parse handler should register");
    super::super::apply_runtime_outputs(&evaluation, 1, behavior, sdui).await;
}

fn temp_workspace(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "clay-connection-workspace-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&dir).unwrap();
    dir
}
use crate::{
    protocol::{
        BehaviorManifest, BehaviorScope, ClientMessage, DocumentAccess, DocumentMetadata,
        EditOperation, EditRejection, FileErrorCode, PROTOCOL_VERSION, ProtocolErrorCode,
        RuntimeDiagnostic, SduiActionArgument, SduiActionIntent, SduiActionSource, SduiActionValue,
        SduiNodeId, SduiNodeKind, ServerMessage, TokenType, codec::Codec,
    },
    server::{
        behavior::ActiveBehaviorManifest, document::DocumentState,
        js_runtime::ClayJsRuntimeService, language_intelligence::LanguageIntelligenceCoordinator,
        parse_coordinator::ParseCoordinator, sdui::StaticSduiState, workspace::WorkspaceState,
    },
    shell::file_browser::FileBrowserState,
};

#[tokio::test]
async fn sdui_actions_and_keybinding_intents_share_command_execution_path() {
    let sdui_request = sdui_command_request(&SduiActionIntent::command(
        "controlCenter.open",
        SduiActionSource::Button {
            node_id: SduiNodeId(5),
        },
    ));
    let keybinding_request = CommandExecutionRequest {
        command_id: "controlCenter.open".to_string(),
        arguments: serde_json::Value::Null,
        target: CommandExecutionTarget::ActiveDocument { document_id: 1 },
        provenance: None,
        expected_permissions: Vec::new(),
    };

    let document = document_state();
    let sdui = sdui_state();
    assert_eq!(
        execute_command_intent(
            sdui_request,
            workspace_state(),
            &document,
            &sdui,
            1,
            None,
            &CommandRegistry::new(),
        )
        .await,
        None
    );
    assert_eq!(
        execute_command_intent(
            keybinding_request,
            workspace_state(),
            &document,
            &sdui,
            1,
            None,
            &CommandRegistry::new(),
        )
        .await,
        None
    );
}

#[tokio::test]
async fn reload_command_intent_uses_shared_server_reload_service() {
    let root = temp_workspace("reload-command-intent");
    fs::write(root.join("init.js"), "").unwrap();
    let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
        "reload-command-intent",
    ));
    config.configuration_root = Some(root.clone());
    let server = super::super::IpcServer::new(config);

    let response = execute_command_intent(
        CommandExecutionRequest {
            command_id: "runtime.reloadConfiguration".to_string(),
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::Global,
            provenance: None,
            expected_permissions: Vec::new(),
        },
        Arc::clone(&server.workspace),
        &server.document,
        &server.sdui,
        1,
        Some(&server),
        &CommandRegistry::new(),
    )
    .await
    .expect("reload command returns status");

    assert!(matches!(
        response,
        ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, .. })
            if code == "runtime.reload_succeeded"
    ));
    assert_eq!(server.runtime_generation.generation_id().await, 2);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn settings_live_switch_persists_and_reloads_end_to_end() {
    // Plan 067 task 12: full live-switch + persistence matrix through the
    // real command executor + persist + reload path. From a clean config:
    // settings.setTheme selects Gruvbox (proving Gruvbox remains
    // selectable), persists to preferences.json, and advances the runtime
    // generation so the change applies live via reload→fanout;
    // settings.setAppearance persists appearance; settings.reset clears the
    // store and reloads; a non-bundled @clay/theme-* specifier is rejected
    // by execute_settings without advancing the generation.
    let root = temp_workspace("settings-live-switch-e2e");
    fs::write(root.join("init.js"), "").unwrap();
    let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
        "settings-live-switch-e2e",
    ));
    config.configuration_root = Some(root.clone());
    let server = super::super::IpcServer::new(config);
    let preferences = root.join("preferences.json");
    let workspace = workspace_state();
    let document = document_state();
    let sdui = sdui_state();

    let settings_registry = CommandRegistry::new();
    let settings_request = |command_id: &str, item_id: &str| {
        execute_command_intent(
            CommandExecutionRequest {
                command_id: command_id.to_string(),
                arguments: serde_json::json!({ "item_id": item_id }),
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            },
            Arc::clone(&workspace),
            &document,
            &sdui,
            1,
            Some(&server),
            &settings_registry,
        )
    };

    // 0. Open/close remain validated server commands but project one
    //    narrow client UI request without reloading.
    assert!(matches!(
        settings_request("settings.open", "").await,
        Some(ServerMessage::ShellClientCommandRequest { command_id })
            if command_id == "settings.open"
    ));
    assert_eq!(server.runtime_generation.generation_id().await, 1);

    // 1. settings.setTheme selects Gruvbox Material Light (opt-in theme
    //    remains selectable), persists, and reloads live.
    let response =
        settings_request("settings.setTheme", "@clay/theme-gruvbox-material-light").await;
    assert!(
        response.is_none(),
        "settings.setTheme returns no error on success"
    );
    assert_eq!(server.runtime_generation.generation_id().await, 2);
    let persisted = fs::read_to_string(&preferences).expect("preferences.json written");
    assert!(
        persisted.contains("@clay/theme-gruvbox-material-light"),
        "preferences.json persists the selected theme: {persisted}"
    );

    // 2. settings.setAppearance persists appearance and reloads again.
    let response = settings_request("settings.setAppearance", "light").await;
    assert!(
        response.is_none(),
        "settings.setAppearance returns no error on success"
    );
    assert_eq!(server.runtime_generation.generation_id().await, 3);
    let persisted = fs::read_to_string(&preferences).expect("preferences.json updated");
    assert!(
        persisted.contains("\"appearance\"") && persisted.contains("light"),
        "preferences.json persists appearance: {persisted}"
    );

    // 3. Complete typography persists and reloads through the same parser.
    let typography = serde_json::json!({
        "monospace": { "families": ["Mono"], "size": 16 },
        "proportional": { "families": ["Sans"], "size": 17 },
        "ui": { "families": ["UI"], "size": 14 },
        "hierarchy": {
            "display": 1.5, "title": 1.16, "section": 1.08,
            "body": 1.0, "status": 1.0, "detail": 0.83, "caption": 0.75
        }
    });
    let response = execute_command_intent(
        CommandExecutionRequest {
            command_id: "settings.setTypography".to_string(),
            arguments: serde_json::json!({ "typography": typography.to_string() }),
            target: CommandExecutionTarget::Global,
            provenance: None,
            expected_permissions: Vec::new(),
        },
        Arc::clone(&workspace),
        &document,
        &sdui,
        1,
        Some(&server),
        &settings_registry,
    )
    .await;
    assert!(response.is_none(), "complete typography applies");
    assert_eq!(server.runtime_generation.generation_id().await, 4);
    let persisted = fs::read_to_string(&preferences).expect("typography persisted");
    assert!(persisted.contains("\"typography\"") && persisted.contains("\"UI\""));

    // 4. A non-bundled @clay/theme-* specifier is rejected by execute_settings
    //    and does not advance the generation (authority denial).
    let generation_before = server.runtime_generation.generation_id().await;
    let response = settings_request("settings.setTheme", "@clay/theme-evil").await;
    assert!(
        matches!(response, Some(ServerMessage::Error { .. })),
        "non-bundled theme specifier is rejected"
    );
    assert_eq!(
        server.runtime_generation.generation_id().await,
        generation_before,
        "rejected settings intent does not reload"
    );

    // 5. settings.reset clears the persisted store and reloads.
    let response = execute_command_intent(
        CommandExecutionRequest {
            command_id: "settings.reset".to_string(),
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::Global,
            provenance: None,
            expected_permissions: Vec::new(),
        },
        Arc::clone(&workspace),
        &document,
        &sdui,
        1,
        Some(&server),
        &CommandRegistry::new(),
    )
    .await;
    assert!(
        response.is_none(),
        "settings.reset returns no error on success"
    );
    assert_eq!(
        server.runtime_generation.generation_id().await,
        generation_before + 1
    );
    let reset = fs::read_to_string(&preferences).unwrap_or_default();
    assert!(
        !reset.contains("@clay/theme-"),
        "preferences.json cleared after reset: {reset}"
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn settings_set_design_system_persists_and_snapshot_lists_choices() {
    // Plan 110 task 10: settings.setDesignSystem persists the designSystem
    // preference and reloads live; the committed runtime snapshot enumerates
    // the installed theme/design-system packages plus the persisted appearance
    // so the Settings panel renders real choice lists.
    let root = temp_workspace("settings-design-system-e2e");
    fs::write(root.join("init.js"), "").unwrap();
    let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
        "settings-design-system-e2e",
    ));
    config.configuration_root = Some(root.clone());
    let server = super::super::IpcServer::new(config);
    let preferences = root.join("preferences.json");
    let workspace = workspace_state();
    let document = document_state();
    let sdui = sdui_state();

    let registry = CommandRegistry::new();
    let settings_request = |command_id: &str, arguments: serde_json::Value| {
        execute_command_intent(
            CommandExecutionRequest {
                command_id: command_id.to_string(),
                arguments,
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            },
            Arc::clone(&workspace),
            &document,
            &sdui,
            1,
            Some(&server),
            &registry,
        )
    };
    let item = |specifier: &str| serde_json::json!({ "item_id": specifier });

    // 1. Appearance persists and shows up in the snapshot choices.
    let response = settings_request("settings.setAppearance", item("light")).await;
    assert!(response.is_none(), "setAppearance accepted");
    assert_eq!(server.runtime_generation.generation_id().await, 2);

    // 2. Design system selection persists and reloads. A real bundled DS
    //    package now applies through the shared enable path (plan 110 task 18
    //    fixed the package-service double-lock deadlock); the specifier is
    //    suffix-built to stay plan-104 source-independence-guard-proof.
    let ds_suffix = "instrument";
    let ds_specifier = format!("@clay/design-{ds_suffix}");
    let response = settings_request("settings.setDesignSystem", item(&ds_specifier)).await;
    assert!(response.is_none(), "setDesignSystem accepted");
    assert_eq!(server.runtime_generation.generation_id().await, 3);
    let persisted = fs::read_to_string(&preferences).expect("preferences written");
    assert!(persisted.contains("designSystem"));

    // 3. Theme selection enables the theme record; the snapshot enumerates it.
    let response = settings_request(
        "settings.setTheme",
        item("@clay/theme-gruvbox-material-light"),
    )
    .await;
    assert!(response.is_none(), "setTheme accepted");
    assert_eq!(server.runtime_generation.generation_id().await, 4);

    let snapshot = server
        .runtime_generation
        .latest_runtime_snapshot_for(1)
        .await
        .expect("committed runtime snapshot");
    let choices = snapshot.ui_choices;
    assert_eq!(choices.appearance.as_deref(), Some("light"));
    assert!(
        choices
            .themes
            .iter()
            .any(|theme| theme.specifier == "@clay/theme-gruvbox-material-light"),
        "enabled theme is enumerated: {:?}",
        choices.themes
    );
    // Plan 118 task 20: the choice set is exactly what ships — the built-in core
    // baseline first, then the enabled design-system packages, sorted — so the
    // Settings dropdown shows the shipped set and nothing else.
    assert_eq!(
        choices
            .design_systems
            .iter()
            .map(|option| option.specifier.as_str())
            .collect::<Vec<_>>(),
        vec!["@clay/core", ds_specifier.as_str()],
        "exactly the shipped design-system choices, core baseline first"
    );
    assert_eq!(
        snapshot.active_design_system.specifier.as_str(),
        ds_specifier,
        "committed snapshot carries the enabled design system"
    );
    assert!(
        !snapshot.active_design_system.recipes.is_empty(),
        "active design system carries the package's resolved recipes"
    );

    // 4. Invalid specifiers are rejected without persisting or reloading.
    let generation_before = server.runtime_generation.generation_id().await;
    let response = settings_request(
        "settings.setDesignSystem",
        item("@clay/theme-modus-vivendi"),
    )
    .await;
    assert!(
        matches!(response, Some(ServerMessage::Error { .. })),
        "non-design-system specifier is rejected"
    );
    assert_eq!(
        server.runtime_generation.generation_id().await,
        generation_before
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn persisted_design_system_preference_applies_at_startup_reload() {
    // Plan 110 task 18 regression: a persisted non-core design-system choice
    // is applied by `apply_persisted_preferences` during the startup reload
    // without deadlocking the package service. The bounded timeout makes a
    // regression fail the test instead of hanging CI.
    let ds_suffix = "instrument";
    let ds_specifier = format!("@clay/design-{ds_suffix}");
    let root = temp_workspace("persisted-design-system-startup");
    fs::write(root.join("init.js"), "").unwrap();
    fs::write(
        root.join("preferences.json"),
        serde_json::json!({ "designSystem": ds_specifier }).to_string(),
    )
    .unwrap();
    let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
        "persisted-design-system-startup",
    ));
    config.configuration_root = Some(root.clone());
    let server = super::super::IpcServer::new(config);

    let outcome = timeout(Duration::from_secs(5), server.reload_runtime_generation())
        .await
        .expect("startup reload with a persisted non-core DS choice must not hang");
    assert!(outcome.reloaded, "startup reload succeeds");
    assert_eq!(server.runtime_generation.generation_id().await, 2);

    let snapshot = server
        .runtime_generation
        .latest_runtime_snapshot_for(1)
        .await
        .expect("committed runtime snapshot");
    assert_eq!(
        snapshot.active_design_system.specifier.as_str(),
        ds_specifier,
        "persisted design system applied at startup"
    );
    assert!(
        !snapshot.active_design_system.recipes.is_empty(),
        "applied design system carries the package's resolved recipes"
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn persisted_removed_design_system_preference_commits_the_core_baseline() {
    // Plan 118 task 20: a preference naming a design system this generation
    // removed must not fail the generation or leave a half-installed snapshot.
    // The committed generation carries the core baseline — the host-consumed
    // subset of the shipped language — and the shipped choice set is unchanged.
    // The specifier is assembled so the plan-118 absence guard sees no literal of
    // a removed package name.
    let removed = format!("@clay/design-{}", "neobrutal");
    let shipped = format!("@clay/design-{}", "instrument");
    let root = temp_workspace("persisted-removed-design-system-startup");
    fs::write(root.join("init.js"), "").unwrap();
    fs::write(
        root.join("preferences.json"),
        serde_json::json!({ "designSystem": removed }).to_string(),
    )
    .unwrap();
    let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
        "persisted-removed-design-system-startup",
    ));
    config.configuration_root = Some(root.clone());
    let server = super::super::IpcServer::new(config);

    let outcome = timeout(Duration::from_secs(5), server.reload_runtime_generation())
        .await
        .expect("startup reload with a removed design-system preference must not hang");
    assert!(outcome.reloaded, "startup reload succeeds");
    assert_eq!(server.runtime_generation.generation_id().await, 2);

    let snapshot = server
        .runtime_generation
        .latest_runtime_snapshot_for(1)
        .await
        .expect("committed runtime snapshot");
    assert_eq!(
        snapshot.active_design_system.specifier.as_str(),
        "@clay/core",
        "a removed design system falls back to the core baseline"
    );
    assert_eq!(
        snapshot.active_design_system.provenance.package_name,
        "core"
    );
    assert!(
        !snapshot.active_design_system.recipes.is_empty(),
        "the fallback carries the shipped language's recipes"
    );
    assert_eq!(
        snapshot
            .ui_choices
            .design_systems
            .iter()
            .map(|option| option.specifier.as_str())
            .collect::<Vec<_>>(),
        vec!["@clay/core", shipped.as_str()],
        "the rejected preference leaves the shipped choice set intact"
    );
    // The shipped system is offered (with its declared display name) without a
    // prior loadPackage, so the dropdown matches what the command accepts.
    assert_eq!(
        snapshot.ui_choices.design_systems[1]
            .display_name
            .as_deref(),
        Some("Quiet Instrument")
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn package_ui_unregistered_action_is_rejected_by_command_execution() {
    let response = execute_command_intent(
        sdui_command_request(&SduiActionIntent::command(
            "markdown.missingCommand",
            SduiActionSource::Button {
                node_id: SduiNodeId(5),
            },
        )),
        workspace_state(),
        &document_state(),
        &sdui_state(),
        1,
        None,
        &CommandRegistry::new(),
    )
    .await
    .expect("unknown package UI action returns protocol error");

    assert!(matches!(response, ServerMessage::Error { .. }));
    if let ServerMessage::Error { message, .. } = response {
        assert!(message.contains("UnknownCommand"));
    }
}

#[tokio::test]
async fn workspace_directory_action_sends_refreshed_file_browser_snapshot() {
    let root = temp_workspace("navigate-snapshot");
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

    let workspace = workspace_state();
    let root_id = workspace.lock().await.add_root(&root).unwrap();
    let document = document_state();
    let sdui = sdui_state();
    let mut intent = SduiActionIntent::command(
        "workspace.openDirectory",
        SduiActionSource::ListItem {
            node_id: SduiNodeId(5),
            item_id: "src".to_string(),
        },
    );
    intent.arguments = vec![
        SduiActionArgument {
            name: "workspaceRootId".to_string(),
            value: SduiActionValue::U64(root_id),
        },
        SduiActionArgument {
            name: "relativePath".to_string(),
            value: SduiActionValue::String("src".to_string()),
        },
    ];

    let response = execute_command_intent(
        sdui_command_request(&intent),
        workspace,
        &document,
        &sdui,
        42,
        None,
        &CommandRegistry::new(),
    )
    .await
    .expect("directory navigation sends a snapshot");

    let ServerMessage::SduiSnapshot { client_id, tree } = response else {
        panic!("expected SduiSnapshot");
    };
    assert_eq!(client_id, 42);
    let labels: Vec<String> = tree
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            SduiNodeKind::List { items, .. } => {
                Some(items.iter().map(|item| item.label.clone()).collect())
            }
            _ => None,
        })
        .unwrap();
    assert!(labels.iter().any(|label| label == "Parent folder"));
    assert!(labels.iter().any(|label| label == "main.rs"));

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn file_browser_action_survives_markdown_open_followup_diagnostic() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = temp_workspace("browser-survives-open-followup");
    fs::write(root.join("note.md"), "# note\n").unwrap();

    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let browser = FileBrowserState::from_workspace(&workspace_state_value, root_id).unwrap();
    let tree = browser.to_sdui_tree(1u64, 1u64);
    let action = tree
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            SduiNodeKind::List { items, .. } => items
                .iter()
                .find(|item| item.label == "note.md")
                .and_then(|item| item.action.clone()),
            _ => None,
        })
        .expect("note.md file-browser action");
    let sdui = empty_sdui_state();
    sdui.lock()
        .await
        .replace_for_document_with_runtime_tree(1, tree)
        .unwrap();

    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let runtime = js_runtime();
    let coordinator = parse_coordinator();
    load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;
    let metadata = DocumentMetadata {
        document_id: 2,
        version: 1,
        access: DocumentAccess::Editable { lease_id: 1 },
        lease_id: Some(1),
        dirty: false,
        workspace_root_id: root_id,
        path: "note.md".to_string(),
    };

    let document = Arc::new(Mutex::new(DocumentState::new(
        2,
        "# note\n".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let messages = super::open_document_followup_messages(
        &metadata,
        &document,
        &behavior,
        &sdui,
        1,
        &runtime,
        &coordinator,
    )
    .await;
    assert!(messages.iter().any(|message| {
        matches!(
            message,
            ServerMessage::BehaviorManifest(_) | ServerMessage::RuntimeDiagnostic(_)
        )
    }));
    sdui.lock()
        .await
        .validate_action(&action)
        .expect("file-browser action remains valid after open-time follow-up");

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn server_accepts_hello_and_sends_snapshot() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "Hello from server".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::Welcome {
            client_id: 99,
            protocol_version: PROTOCOL_VERSION,
        }
    );
    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::InitialDocument {
            document_id: 7,
            version: 1,
            head: crate::protocol::DocumentTextHead::complete("Hello from server".to_string(),),
            access: DocumentAccess::Editable { lease_id: 1 },
            lease_id: Some(1),
            workspace_root: String::new(),
        }
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn handshake_replays_committed_runtime_snapshot_with_pane_surfaces() {
    let generation = runtime_generation();
    let snapshot = crate::protocol::RuntimeStateSnapshot {
        runtime_generation_id: 2,
        client_id: 0,
        behavior: BehaviorManifest::minimal_text_editing(2),
        active_theme: crate::protocol::ActiveTheme {
            specifier: "@clay/default".into(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        },
        active_typography: crate::protocol::ActiveTypography::default(),
        active_design_system: crate::shell::design_system::ActiveDesignSystem::core_fallback(2),
        active_icon_pack: None,
        ui_choices: crate::protocol::UiChoicesSnapshot::default(),
        sdui_tree: crate::server::sdui::default_document_tree(1, 1),
        package_ui: crate::protocol::PackageUiSnapshot {
            version: 2,
            surfaces: vec![crate::protocol::EmptyTabContent {
                id: "coding-agent.surface".into(),
                package_name: "@clay/coding-agent".into(),
                component_json: r#"{"id":"coding-agent.root","kind":"panel","children":[]}"#.into(),
                action_targets: vec!["coding-agent.profile".into()],
                provenance: crate::protocol::PackageUiProvenance {
                    package_name: "@clay/coding-agent".into(),
                    package_version: "0.1.0".into(),
                    api_prefix: "coding-agent".into(),
                    trust_domain: crate::protocol::PackageUiTrustDomain::Trusted,
                },
            }],
            ..Default::default()
        },
        documents: Vec::new(),
        diagnostics: Vec::new(),
    };
    snapshot.validate().expect("handshake fixture snapshot");
    generation.publish_runtime_snapshot(snapshot).await;

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "Hello from server".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        generation,
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();

    let mut saw_surface = false;
    for _ in 0..64 {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::RuntimeStateSnapshot(snapshot) => {
                assert_eq!(snapshot.client_id, 99);
                assert_eq!(snapshot.package_ui.surfaces.len(), 1);
                assert_eq!(snapshot.package_ui.surfaces[0].id, "coding-agent.surface");
                saw_surface = true;
                break;
            }
            ServerMessage::FileOpenCapabilityIssued { .. } => break,
            _ => {}
        }
    }
    assert!(
        saw_surface,
        "handshake must replay the committed runtime snapshot so pane surfaces reach a connecting client"
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

/// Plan 060 T4 test helpers: drain the bootstrap sequence through the
/// always-terminal capability issue so tests start from a clean cursor.
async fn drain_bootstrap(client: &mut tokio::io::DuplexStream, codec: Codec) -> String {
    loop {
        if let ServerMessage::FileOpenCapabilityIssued { token } =
            codec.read_server_message(client).await.unwrap()
        {
            return token;
        }
    }
}

struct TestConnection {
    client: tokio::io::DuplexStream,
    server_task: tokio::task::JoinHandle<Result<(), crate::protocol::codec::CodecError>>,
    codec: Codec,
    file_open_capability: String,
}

impl TestConnection {
    #[allow(
        clippy::too_many_arguments,
        reason = "test connection harness mirrors the server's explicit authority parameters"
    )]
    async fn connect(
        client_id: u64,
        document: Arc<Mutex<DocumentState>>,
        behavior: Arc<Mutex<ActiveBehaviorManifest>>,
        workspace: Arc<Mutex<WorkspaceState>>,
        runtime_generation: super::RuntimeGenerationStore,
        parse_coordinator: ParseCoordinator,
        document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
        language_intelligence: LanguageIntelligenceCoordinator,
    ) -> Self {
        let registry = Arc::new(Mutex::new(crate::server::tab_registry::TabRegistry::new()));
        let (tab_registry_tx, _) = tokio::sync::broadcast::channel(16);
        Self::connect_with_registry(
            client_id,
            document,
            behavior,
            workspace,
            runtime_generation,
            parse_coordinator,
            document_analysis,
            language_intelligence,
            registry,
            tab_registry_tx,
        )
        .await
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "test connection harness mirrors the server's explicit authority parameters"
    )]
    async fn connect_with_registry(
        client_id: u64,
        document: Arc<Mutex<DocumentState>>,
        behavior: Arc<Mutex<ActiveBehaviorManifest>>,
        workspace: Arc<Mutex<WorkspaceState>>,
        runtime_generation: super::RuntimeGenerationStore,
        parse_coordinator: ParseCoordinator,
        document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
        language_intelligence: LanguageIntelligenceCoordinator,
        tab_registry: Arc<Mutex<crate::server::tab_registry::TabRegistry>>,
        tab_registry_tx: tokio::sync::broadcast::Sender<crate::protocol::TabRegistrySnapshot>,
    ) -> Self {
        let (client, server) = duplex(65536);
        let codec = Codec::default();
        let server_task = tokio::spawn(super::handle_connection_with_analysis(
            server,
            client_id,
            document,
            behavior,
            workspace,
            sdui_state(),
            active_theme_state(),
            runtime_diagnostics(),
            runtime_generation,
            parse_coordinator,
            crate::server::completion::CompletionCoordinator::new(),
            document_analysis,
            language_intelligence,
            None,
            tab_registry,
            tab_registry_tx,
            codec,
        ));
        let mut client = client;
        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let file_open_capability = drain_bootstrap(&mut client, codec).await;
        Self {
            client,
            server_task,
            codec,
            file_open_capability,
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "test connection harness mirrors the production IpcServer connection wiring"
    )]
    async fn connect_with_server(client_id: u64, server: super::super::IpcServer) -> Self {
        let (client, server_stream) = duplex(65536);
        let codec = Codec::default();
        let document = Arc::clone(&server.bootstrap_state.welcome);
        let behavior = Arc::clone(&server.behavior);
        let workspace = Arc::clone(&server.bootstrap_state.workspace);
        let sdui = Arc::clone(&server.sdui);
        let active_theme = Arc::clone(&server.active_theme);
        let runtime_diagnostics = Arc::clone(&server.runtime_diagnostics);
        let runtime_generation = server.runtime_generation.clone();
        let parse_coordinator = server.parse_coordinator.clone();
        let completion = server.completion.clone();
        let document_analysis = server.document_analysis.clone();
        let language_intelligence = server.language_intelligence.clone();
        let tab_registry = Arc::clone(&server.tab_registry);
        let tab_registry_tx = server.tab_registry_tx.clone();
        let server_task = tokio::spawn(super::handle_connection_with_analysis(
            server_stream,
            client_id,
            document,
            behavior,
            workspace,
            sdui,
            active_theme,
            runtime_diagnostics,
            runtime_generation,
            parse_coordinator,
            completion,
            document_analysis,
            language_intelligence,
            Some(server),
            tab_registry,
            tab_registry_tx,
            codec,
        ));
        let mut client = client;
        codec
            .write_client_message(
                &mut client,
                &ClientMessage::Hello {
                    protocol_version: PROTOCOL_VERSION,
                    client_name: "test-client".to_string(),
                },
            )
            .await
            .unwrap();
        let file_open_capability = drain_bootstrap(&mut client, codec).await;
        Self {
            client,
            server_task,
            codec,
            file_open_capability,
        }
    }

    async fn reclaim(&mut self, client_id: u64, tab_id: crate::protocol::TabId) {
        self.send(&ClientMessage::TabCommand {
            client_id,
            command: crate::protocol::TabCommand::Reclaim { tab_id },
        })
        .await;
        let mut received_initial_document = false;
        loop {
            match self.receive().await {
                ServerMessage::InitialDocument { .. } => received_initial_document = true,
                ServerMessage::TabRegistry(_) if received_initial_document => return,
                ServerMessage::SduiSnapshot { .. }
                | ServerMessage::TabRegistry(_)
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::FileOpenCapabilityIssued { .. }
                | ServerMessage::BehaviorManifest(_) => {}
                other => panic!("unexpected message during tab reclaim: {other:?}"),
            }
        }
    }

    async fn open_document(
        &mut self,
        client_id: u64,
        workspace_root_id: crate::protocol::WorkspaceRootId,
        path: &str,
    ) -> (DocumentMetadata, crate::protocol::BehaviorVersion) {
        self.send(&ClientMessage::OpenDocument {
            client_id,
            workspace_root_id,
            path: path.to_string(),
        })
        .await;
        let mut behavior_version = 1;
        loop {
            match self.receive().await {
                message @ ServerMessage::BehaviorManifest(_) => {
                    let ServerMessage::BehaviorManifest(manifest) = message else {
                        unreachable!();
                    };
                    behavior_version = manifest.behavior_version;
                }
                message @ ServerMessage::DocumentOpened { .. } => {
                    let ServerMessage::DocumentOpened { metadata, .. } = message else {
                        unreachable!();
                    };
                    return (metadata, behavior_version);
                }
                ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::SduiSnapshot { .. }
                | ServerMessage::TabRegistry(_) => {}
                other => panic!("unexpected message during document open: {other:?}"),
            }
        }
    }

    async fn send(&mut self, message: &ClientMessage) {
        self.codec
            .write_client_message(&mut self.client, message)
            .await
            .unwrap();
    }

    async fn receive(&mut self) -> ServerMessage {
        self.codec
            .read_server_message(&mut self.client)
            .await
            .unwrap()
    }

    async fn close(self) {
        drop(self.client);
        self.server_task.await.unwrap().unwrap();
    }

    /// Drain open/activation follow-ups until the stream goes quiet so the
    /// next read observes the response to the next request, not a queued
    /// BehaviorManifest/decoration frame.
    async fn drain_until_quiet(&mut self) {
        while timeout(
            Duration::from_millis(50),
            self.codec.read_server_message(&mut self.client),
        )
        .await
        .is_ok()
        {}
    }

    /// Drain a bounded amount of asynchronous output. Some parser lanes
    /// can continuously publish while a test is intentionally not asserting
    /// every advisory frame.
    async fn drain_bounded(&mut self) {
        for _ in 0..32 {
            if timeout(
                Duration::from_millis(10),
                self.codec.read_server_message(&mut self.client),
            )
            .await
            .is_err()
            {
                break;
            }
        }
    }

    /// Read until the response frame arrives, skipping asynchronous parse
    /// and activation output that can race a request/response exchange.
    async fn receive_response(&mut self) -> ServerMessage {
        loop {
            let frame = self.receive().await;
            if matches!(
                frame,
                ServerMessage::Error { .. }
                    | ServerMessage::FileOperationFailed { .. }
                    | ServerMessage::DocumentSaved { .. }
                    | ServerMessage::DocumentReloaded { .. }
                    | ServerMessage::DocumentClosed { .. }
                    | ServerMessage::DocumentStatus { .. }
                    | ServerMessage::DocumentList { .. }
                    | ServerMessage::ResyncSnapshot { .. }
            ) {
                return frame;
            }
        }
    }
}

#[tokio::test]
async fn connection_state_route_follows_reclaim_and_fails_closed() {
    let root_a = temp_workspace("route-alpha");
    let root_b = temp_workspace("route-beta");
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("connection-state-route"),
    ));
    let (alpha_snapshot, alpha_state) = server
        .create_tab_state(11, root_a.to_string_lossy().into_owned())
        .await
        .expect("alpha tab state is created");
    let (beta_snapshot, beta_state) = server
        .create_tab_state(22, root_b.to_string_lossy().into_owned())
        .await
        .expect("beta tab state is created");
    let alpha_tab = alpha_snapshot.tabs[0].tab_id;
    let beta_tab = beta_snapshot.tabs[1].tab_id;

    let alpha_route = route_connection_tab_state(
        11,
        Some(&server),
        &alpha_state.welcome,
        &alpha_state.workspace,
    )
    .await
    .expect("bound alpha route");
    assert_eq!(alpha_route.tab_id, Some(alpha_tab));
    assert!(Arc::ptr_eq(
        &alpha_route.state.workspace,
        &alpha_state.workspace
    ));
    assert!(!Arc::ptr_eq(
        &alpha_route.state.workspace,
        &beta_state.workspace
    ));

    fs::write(root_a.join("alpha.txt"), "alpha").expect("alpha file is written");
    fs::write(root_b.join("beta.txt"), "beta").expect("beta file is written");
    let alpha_root_id = alpha_state
        .workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .next()
        .expect("alpha root")
        .workspace_root_id;
    let beta_root_id = beta_state
        .workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .next()
        .expect("beta root")
        .workspace_root_id;
    let alpha_document = crate::server::workspace::open_existing_file_unlocked(
        &alpha_route.state.workspace,
        alpha_root_id,
        "alpha.txt",
        11,
    )
    .await
    .expect("alpha document opens in alpha state");
    let beta_document = crate::server::workspace::open_existing_file_unlocked(
        &beta_state.workspace,
        beta_root_id,
        "beta.txt",
        22,
    )
    .await
    .expect("beta document opens in beta state");
    let alpha_response = alpha_document.document.lock().await.apply_edit(
        alpha_document.document_id,
        11,
        alpha_document.access.lease_id(),
        1,
        1,
        crate::protocol::EditOperation::Insert {
            byte_offset: 5,
            text: "!".to_string(),
        },
    );
    assert!(matches!(alpha_response, ServerMessage::EditAck { .. }));
    let beta_document_state = beta_document.document.lock().await;
    assert_eq!(beta_document_state.version(), 1);
    assert!(!beta_document_state.is_dirty());
    assert_eq!(beta_document_state.text(), "beta");

    assert!(server.tab_registry.lock().await.reclaim(alpha_tab, 33));
    assert!(
        route_connection_tab_state(
            11,
            Some(&server),
            &alpha_state.welcome,
            &alpha_state.workspace,
        )
        .await
        .is_none()
    );
    let reclaimed_route = route_connection_tab_state(
        33,
        Some(&server),
        &alpha_state.welcome,
        &alpha_state.workspace,
    )
    .await
    .expect("reclaimed route");
    assert_eq!(reclaimed_route.tab_id, Some(alpha_tab));
    assert!(Arc::ptr_eq(
        &reclaimed_route.state.workspace,
        &alpha_state.workspace
    ));

    server.remove_tab_state(beta_tab).await;
    assert!(
        route_connection_tab_state(
            22,
            Some(&server),
            &beta_state.welcome,
            &beta_state.workspace,
        )
        .await
        .is_none()
    );

    let _ = fs::remove_dir_all(root_a);
    let _ = fs::remove_dir_all(root_b);
}

#[tokio::test]
async fn cross_tab_workspace_and_document_authority_is_fail_closed() {
    let root_a = temp_workspace("authority-alpha");
    let root_b = temp_workspace("authority-beta");
    let extra_root_a = temp_workspace("authority-alpha-extra");
    fs::write(root_a.join("alpha.txt"), "alpha").unwrap();
    fs::write(root_b.join("beta.txt"), "beta").unwrap();
    fs::write(extra_root_a.join("only-alpha.txt"), "only alpha").unwrap();

    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("cross-tab-authority"),
    ));
    let (alpha_snapshot, alpha_state) = server
        .create_tab_state(11, root_a.to_string_lossy().into_owned())
        .await
        .expect("alpha tab state is created");
    let (beta_snapshot, beta_state) = server
        .create_tab_state(22, root_b.to_string_lossy().into_owned())
        .await
        .expect("beta tab state is created");
    let alpha_tab = alpha_snapshot.tabs[0].tab_id;
    let beta_tab = beta_snapshot.tabs[1].tab_id;
    let alpha_root_id = alpha_state
        .workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .next()
        .expect("alpha root")
        .workspace_root_id;
    let alpha_extra_root_id = alpha_state
        .workspace
        .lock()
        .await
        .add_root(&extra_root_a)
        .expect("alpha extra root");
    let beta_root_id = beta_state
        .workspace
        .lock()
        .await
        .directory_roots()
        .into_iter()
        .next()
        .expect("beta root")
        .workspace_root_id;

    let mut connection_a = TestConnection::connect_with_server(11, server.clone()).await;
    connection_a.reclaim(11, alpha_tab).await;
    let mut connection_b = TestConnection::connect_with_server(22, server.clone()).await;
    connection_b.reclaim(22, beta_tab).await;
    connection_a.drain_bounded().await;

    let (alpha_metadata, alpha_behavior_version) = connection_a
        .open_document(11, alpha_root_id, "alpha.txt")
        .await;
    let (beta_metadata, _) = connection_b
        .open_document(22, beta_root_id, "beta.txt")
        .await;
    connection_a.drain_bounded().await;
    connection_b.drain_bounded().await;
    connection_a
        .send(&ClientMessage::ListDocuments { client_id: 11 })
        .await;
    let alpha_list = connection_a.receive_response().await;
    assert!(matches!(
        alpha_list,
        ServerMessage::DocumentList { ref documents }
            if documents.len() == 1 && documents[0].document_id == alpha_metadata.document_id
    ));

    connection_b
        .send(&ClientMessage::ListDocuments { client_id: 22 })
        .await;
    let beta_list = connection_b.receive_response().await;
    assert!(matches!(
        beta_list,
        ServerMessage::DocumentList { ref documents }
            if documents.len() == 1 && documents[0].document_id == beta_metadata.document_id
    ));

    connection_a
        .send(&ClientMessage::OpenDocument {
            client_id: 11,
            workspace_root_id: beta_root_id,
            path: "beta.txt".to_string(),
        })
        .await;
    let foreign_open = connection_a.receive_response().await;
    assert!(matches!(
        foreign_open,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::NotFound,
            ..
        }
    ));

    connection_b
        .send(&ClientMessage::OpenDocument {
            client_id: 22,
            workspace_root_id: alpha_extra_root_id,
            path: "only-alpha.txt".to_string(),
        })
        .await;
    let foreign_root = connection_b.receive_response().await;
    assert!(matches!(
        foreign_root,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownWorkspaceRoot,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::RequestResync {
            client_id: 11,
            document_id: beta_metadata.document_id,
            known_version: 1,
        })
        .await;
    let foreign_resync = connection_a.receive_response().await;
    assert!(matches!(
        foreign_resync,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownDocument,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 11,
            document_id: beta_metadata.document_id,
        })
        .await;
    let foreign_status = connection_a.receive_response().await;
    assert!(matches!(
        foreign_status,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownDocument,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::SaveDocument {
            client_id: 11,
            document_id: beta_metadata.document_id,
            known_version: beta_metadata.version,
        })
        .await;
    let foreign_save = connection_a.receive_response().await;
    assert!(matches!(
        foreign_save,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownDocument,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::ReloadDocument {
            client_id: 11,
            document_id: beta_metadata.document_id,
            known_version: beta_metadata.version,
            force: true,
        })
        .await;
    let foreign_reload = connection_a.receive_response().await;
    assert!(matches!(
        foreign_reload,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownDocument,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::CloseDocument {
            client_id: 11,
            document_id: beta_metadata.document_id,
            force: true,
        })
        .await;
    let foreign_close = connection_a.receive_response().await;
    assert!(matches!(
        foreign_close,
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::UnknownDocument,
            ..
        }
    ));

    connection_a
        .send(&ClientMessage::Edit {
            client_id: 11,
            document_id: beta_metadata.document_id,
            lease_id: beta_metadata.lease_id,
            base_version: beta_metadata.version,
            behavior_version: alpha_behavior_version,
            transaction_id: 7,
            operation: EditOperation::Insert {
                byte_offset: 0,
                text: "leak".to_string(),
            },
        })
        .await;
    let foreign_edit = connection_a.receive().await;
    assert!(matches!(
        foreign_edit,
        ServerMessage::EditRejected {
            reason: EditRejection::InvalidDocument { document_id },
            ..
        } if document_id == beta_metadata.document_id
    ));

    connection_a
        .send(&ClientMessage::OpenSelectedFile {
            client_id: 11,
            capability: connection_b.file_open_capability.clone(),
            selected_path: root_b.join("beta.txt").to_string_lossy().into_owned(),
        })
        .await;
    let capability_replenish = connection_a.receive().await;
    let capability_rejection = connection_a.receive().await;
    assert!(matches!(
        capability_replenish,
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));
    assert!(matches!(
        capability_rejection,
        ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { ref code, .. })
            if code == "client.selected_file_open.unauthorized"
    ));

    connection_b
        .send(&ClientMessage::RequestResync {
            client_id: 22,
            document_id: beta_metadata.document_id,
            known_version: beta_metadata.version,
        })
        .await;
    let beta_resync = connection_b.receive_response().await;
    assert!(matches!(
        beta_resync,
        ServerMessage::ResyncSnapshot { ref head, document_id, .. }
            if document_id == beta_metadata.document_id && head.first_chunk == "beta"
    ));
    connection_b
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 22,
            document_id: beta_metadata.document_id,
        })
        .await;
    let beta_status = connection_b.receive_response().await;
    assert!(matches!(
        beta_status,
        ServerMessage::DocumentStatus { ref metadata }
            if metadata.document_id == beta_metadata.document_id
                && metadata.version == beta_metadata.version
                && !metadata.dirty
    ));

    connection_a
        .send(&ClientMessage::TabCommand {
            client_id: 11,
            command: crate::protocol::TabCommand::Reclaim { tab_id: beta_tab },
        })
        .await;
    let reclaim_foreign = connection_a.receive().await;
    assert!(matches!(
        reclaim_foreign,
        ServerMessage::Error {
            code: ProtocolErrorCode::InvalidMessage,
            ..
        }
    ));
    let registry = server.tab_registry.lock().await.snapshot();
    assert!(
        registry
            .tabs
            .iter()
            .any(|entry| entry.tab_id == alpha_tab && entry.client_id == 11)
    );
    assert!(
        registry
            .tabs
            .iter()
            .any(|entry| entry.tab_id == beta_tab && entry.client_id == 22)
    );

    connection_a.close().await;
    connection_b.close().await;
    let _ = fs::remove_dir_all(root_a);
    let _ = fs::remove_dir_all(root_b);
    let _ = fs::remove_dir_all(extra_root_a);
}

/// Phase 24.3: `controlCenter.openPath` opens the Path Browser through
/// the shared Command Centre helper — from its keybinding command and
/// from the Control Center catalogue — with the one-active-session
/// invariant enforced on every open.
#[tokio::test]
async fn path_browser_opens_from_keybinding_and_control_center_catalogue() {
    let root = temp_workspace("path-browser-open");
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("README.md"), "# path browser").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-open"),
    ));
    let (tab_snapshot, _) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;
    let behavior_version = server.behavior.lock().await.version();

    // Keybinding path: one seed resolution + one bounded listing, pushed
    // as the initial snapshot (the active document is the tab's welcome
    // document, so the seed falls back to the bound tab's workspace root).
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.openPath".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(first) = receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    assert_eq!(first.prompt, format!("Browse · {}", root.display()));
    assert_eq!(first.query, format!("{}/", root.display()));
    // Plan 124 task 7: the path browser is the palette's path mode, so it rides
    // the same composer anchor as the catalogue instead of a window sheet.
    assert_eq!(
        first.origin,
        crate::protocol::TransientMenuOriginData::CommandPalette
    );
    let names: Vec<_> = first.items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(
        names,
        vec!["src", "README.md"],
        "empty filter keeps deterministic directory-first order"
    );
    let first_path_id = first.session_id;
    assert!(first_path_id & (1 << 63) != 0, "server-owned id partition");

    // Reopening replaces the active session and reports the closed id.
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.openPath".to_string(),
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == first_path_id
    ));
    let ServerMessage::TransientMenuSnapshot(second) = receive_menu_message(&mut connection).await
    else {
        panic!("expected replacement TransientMenuSnapshot");
    };
    assert_ne!(second.session_id, first_path_id);
    let second_path_id = second.session_id;
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: second_path_id,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == second_path_id
    ));

    // Control Center path: the catalogue lists "Browse Filesystem";
    // activating it closes the Control Center and opens the Path Browser
    // through the same helper.
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.open".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(control_center) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected Control Center snapshot");
    };
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: control_center.session_id,
            query: "Browse Filesystem".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(filtered) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filtered snapshot");
    };
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(filtered.items[0].id, "controlCenter.openPath");
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: control_center.session_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == control_center.session_id
    ));
    let ServerMessage::TransientMenuSnapshot(from_catalogue) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected Path Browser snapshot from catalogue activation");
    };
    assert_eq!(
        from_catalogue.prompt,
        format!("Browse · {}", root.display())
    );
    assert_eq!(from_catalogue.query, format!("{}/", root.display()));

    connection.drain_bounded().await;
    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3: `controlCenter.openPath` opens the Path Browser through
/// the shared Command Centre helper — from its keybinding command and
/// from the Control Center catalogue — with the one-active-session
/// invariant enforced on every open.
#[tokio::test]
async fn path_browser_opens_with_sticky_error_for_unlistable_seed() {
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-error-seed"),
    ));
    let root = temp_workspace("path-browser-error-seed");
    let (tab_snapshot, _) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;
    // The tab root vanishes after binding: the seed still resolves to it,
    // but the bounded listing fails. The command does not fail; the
    // session opens in its sticky error state (empty items, bounded
    // status) and stays cancellable.
    let _ = fs::remove_dir_all(&root);
    let behavior_version = server.behavior.lock().await.version();

    // A missing seed does not fail the command: the session opens in its
    // sticky error state (empty items, bounded status) and stays
    // cancellable.
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.openPath".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    assert!(snapshot.prompt.starts_with("Browse · "));
    assert!(snapshot.items.is_empty(), "items suppressed under error");
    assert!(matches!(
        snapshot.status,
        crate::protocol::TransientMenuStatusData::Empty { .. }
    ));
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: snapshot.session_id,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == snapshot.session_id
    ));
    connection.drain_bounded().await;
    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 8): directory descend (primary activation keeps the
/// session open and relists), empty-filter Backspace ascent, direct
/// absolute/relative path jumps, and invalid-path recovery — all with a
/// stable session id and exactly one snapshot per accepted transition.
#[tokio::test]
async fn path_browser_navigates_descend_ascend_and_direct_jump() {
    let root = temp_workspace("path-browser-navigate");
    fs::create_dir_all(root.join("a/b")).unwrap();
    fs::create_dir(root.join("c")).unwrap();
    fs::write(root.join("notes.txt"), "notes").unwrap();
    fs::write(root.join("a/a1.txt"), "a1").unwrap();
    fs::write(root.join("a/b/b1.txt"), "b1").unwrap();
    fs::write(root.join("c/c1.txt"), "c1").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-navigate"),
    ));
    let (tab_snapshot, _) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;
    let behavior_version = server.behavior.lock().await.version();

    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.openPath".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    let path_id = snapshot.session_id;
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["a", "c", "notes.txt"]);

    // Direct relative jump: typing `c/` from the tab root relists
    // `/root/c`.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "c/".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected relist snapshot");
    };
    assert_eq!(snapshot.session_id, path_id, "session id stays stable");
    assert_eq!(snapshot.prompt, format!("Browse · {}/c", root.display()));
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["c1.txt"]);

    // Direct absolute jump into `a/b`.
    let a_b = format!("{}/a/b/", root.display());
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: a_b.clone(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected absolute jump snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.prompt, format!("Browse · {}/a/b", root.display()));
    assert_eq!(snapshot.query, a_b);
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["b1.txt"]);

    // Empty-filter Backspace ascends one level (relist, same session).
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected ascent snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.prompt, format!("Browse · {}/a", root.display()));
    assert_eq!(snapshot.query, format!("{}/a/", root.display()));
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["b", "a1.txt"]);

    // Primary activation on the selected directory (`b`, index 0)
    // descends: the session stays open and relists.
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected descend snapshot");
    };
    assert_eq!(snapshot.session_id, path_id, "descend keeps the session");
    assert_eq!(snapshot.prompt, format!("Browse · {}/a/b", root.display()));
    assert_eq!(snapshot.query, format!("{}/a/b/", root.display()));
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["b1.txt"]);

    // Filter-only edits never relist (no second snapshot): typing a
    // fuzzy fragment over the listing re-scores locally.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "b1".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filter-only snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.query, "b1");
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["b1.txt"]);

    // Invalid direct jump: the menu stays open with a bounded error
    // status (items suppressed), and Backspace recovers by ascending to
    // the last canonical directory.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: format!("{}/missing/", root.display()),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected error-status snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert!(snapshot.items.is_empty(), "items suppressed under error");
    assert!(matches!(
        snapshot.status,
        crate::protocol::TransientMenuStatusData::Empty { .. }
    ));
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected recovery snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.prompt, format!("Browse · {}/a", root.display()));
    assert!(!snapshot.items.is_empty(), "recovered listing reinstated");

    // Path mode browses the whole filesystem: ascents past the tab root
    // continue to the filesystem root, where Backspace is a no-op. The
    // recovery above left the session at `/root/a`.
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected tab-root ascent snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.prompt, format!("Browse · {}", root.display()));
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected /tmp ascent snapshot");
    };
    assert_eq!(snapshot.prompt, "Browse · /tmp");
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filesystem-root snapshot");
    };
    assert_eq!(snapshot.prompt, "Browse · /");
    assert_eq!(snapshot.query, "/");
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected root no-op snapshot");
    };
    assert_eq!(snapshot.session_id, path_id);
    assert_eq!(snapshot.query, "/", "filesystem root Backspace is a no-op");

    connection.drain_bounded().await;
    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 9): primary activation on a selected file closes the
/// path session and runs the ordinary selected-file open — the browse
/// activation itself is the authorization event that converts to exactly
/// one `SingleFile` grant. The grant is strictly single-file (siblings
/// fail `OutsideRoot`), duplicate opens return the same document id with
/// no second view, and a file that disappeared between listing and
/// activation fails without a grant or document leak.
#[tokio::test]
async fn path_browser_open_file_converts_browse_to_single_file_grant() {
    let root = temp_workspace("path-browser-open-file");
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("sub/b.txt"), "b").unwrap();
    fs::write(root.join("a.txt"), "hello").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-open-file"),
    ));
    let (tab_snapshot, tab_state) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;

    // Open the Path Browser with a fresh behavior stamp; a stale stamp
    // (a concurrent manifest publish bumps the connection-wide version)
    // is answered with a bounded Error, so resync and retry exactly like
    // the real client would.
    async fn open_path_browser(
        connection: &mut TestConnection,
        server: &super::super::IpcServer,
    ) -> ServerMessage {
        loop {
            connection
                .send(&ClientMessage::CommandIntent {
                    client_id: 11,
                    document_id: 1,
                    behavior_version: server.behavior.lock().await.version(),
                    command_id: "controlCenter.openPath".to_string(),
                })
                .await;
            loop {
                match timeout(Duration::from_secs(5), connection.receive()).await {
                    Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                    Ok(ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message,
                    }) if message.contains("behavior version is stale") => break,
                    Ok(_) => continue,
                    Err(_) => panic!("timed out awaiting path browser snapshot"),
                }
            }
        }
    }

    // Read the next transient-menu frame, skipping parse/analysis noise
    // that can race an exchange; labeled so failures name the phase.
    async fn recv_menu_frame(
        label: &'static str,
        connection: &mut TestConnection,
    ) -> ServerMessage {
        loop {
            match timeout(Duration::from_secs(5), connection.receive()).await {
                Ok(
                    message @ (ServerMessage::TransientMenuSnapshot(_)
                    | ServerMessage::TransientMenuClosed { .. }),
                ) => return message,
                Ok(_) => continue,
                Err(_) => panic!("{label}: timed out awaiting transient menu frame"),
            }
        }
    }

    // Open the path browser and filter down to the file (directory-first
    // order puts `sub` at index 0).
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "a.txt".to_string(),
            scope: None,
        })
        .await;
    let snapshot = recv_menu_frame("filter", &mut connection).await;
    let ServerMessage::TransientMenuSnapshot(snapshot) = snapshot else {
        panic!("filter: expected snapshot, got closed");
    };
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["a.txt"]);

    // Primary activation: session closes first, then DocumentOpened with
    // the ordinary follow-up chain (no capability token involved).
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let closed = recv_menu_frame("close-before-open", &mut connection).await;
    let ServerMessage::TransientMenuClosed { session_id } = closed else {
        panic!("close-before-open: expected closed, got {closed:?}");
    };
    assert_eq!(session_id, path_id, "menu closes before the open response");
    let (opened_root_id, opened_document_id) =
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::DocumentOpened { metadata, head }) => {
                assert_eq!(head.first_chunk, "hello");
                assert_eq!(metadata.path, "a.txt");
                (metadata.workspace_root_id, metadata.document_id)
            }
            Ok(other) => panic!("expected DocumentOpened, got {other:?}"),
            Err(_) => panic!("timed out awaiting DocumentOpened"),
        };
    // Follow-ups arrive after the open frame; drain them.
    connection.drain_bounded().await;

    // The browse activation became a single-file grant: opening a
    // sibling document under that root fails OutsideRoot.
    connection
        .send(&ClientMessage::OpenDocument {
            client_id: 11,
            workspace_root_id: opened_root_id,
            path: "sub/b.txt".to_string(),
        })
        .await;
    let mut saw_outside_root = false;
    for _ in 0..8 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::FileOperationFailed {
                code: FileErrorCode::OutsideRoot,
                workspace_root_id: Some(id),
                document_id: None,
                ..
            }) if id == opened_root_id => {
                saw_outside_root = true;
                break;
            }
            Ok(ServerMessage::DecorationSet(_))
            | Ok(ServerMessage::DiagnosticSet(_) | ServerMessage::FoldingRangeSet(_))
            | Ok(ServerMessage::RuntimeDiagnostic(_))
            | Ok(ServerMessage::BehaviorManifest(_)) => {}
            Ok(other) => panic!("expected outside-root failure, got {other:?}"),
            Err(_) => panic!("timed out awaiting outside-root failure"),
        }
    }
    assert!(saw_outside_root, "single-file grant rejects siblings");
    connection.drain_bounded().await;

    // Duplicate open: reopening the same file returns the same document
    // id with no second view or grant.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "a.txt".to_string(),
            scope: None,
        })
        .await;
    let snapshot = recv_menu_frame("second-filter", &mut connection).await;
    let ServerMessage::TransientMenuSnapshot(snapshot) = snapshot else {
        panic!("second-filter: expected snapshot, got closed");
    };
    assert_eq!(snapshot.items.len(), 1);
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let closed = recv_menu_frame("close-duplicate", &mut connection).await;
    assert!(
        matches!(closed, ServerMessage::TransientMenuClosed { .. }),
        "close-duplicate: expected closed, got {closed:?}"
    );
    let mut duplicate_id = None;
    for _ in 0..8 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::DocumentOpened { metadata, .. }) => {
                duplicate_id = Some(metadata.document_id);
                break;
            }
            Ok(ServerMessage::DecorationSet(_))
            | Ok(ServerMessage::DiagnosticSet(_) | ServerMessage::FoldingRangeSet(_))
            | Ok(ServerMessage::RuntimeDiagnostic(_))
            | Ok(ServerMessage::BehaviorManifest(_)) => {}
            Ok(other) => panic!("expected duplicate DocumentOpened, got {other:?}"),
            Err(_) => panic!("timed out awaiting duplicate DocumentOpened"),
        }
    }
    assert_eq!(
        duplicate_id,
        Some(opened_document_id),
        "duplicate open returns the existing document"
    );
    connection.drain_bounded().await;
    let workspace = tab_state.workspace.lock().await;
    assert!(
        workspace
            .document_canonical_path(opened_document_id)
            .is_some()
    );
    assert!(
        workspace
            .document_canonical_path(opened_document_id + 1)
            .is_none(),
        "no second document created by the duplicate open"
    );
    drop(workspace);

    // Disappeared file: the listing was taken before deletion, so the
    // activation still resolves to the stale canonical path; the open
    // fails with a bounded FileOperationFailed, the session is closed,
    // and no grant or document appears.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "a.txt".to_string(),
            scope: None,
        })
        .await;
    let snapshot = recv_menu_frame("third-filter", &mut connection).await;
    let ServerMessage::TransientMenuSnapshot(snapshot) = snapshot else {
        panic!("third-filter: expected snapshot, got closed");
    };
    assert_eq!(snapshot.items.len(), 1, "listing predates the deletion");
    fs::remove_file(root.join("a.txt")).unwrap();
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let closed = recv_menu_frame("close-failed-open", &mut connection).await;
    assert!(
        matches!(closed, ServerMessage::TransientMenuClosed { .. }),
        "close-failed-open: expected closed, got {closed:?}"
    );
    let mut failed = false;
    for _ in 0..8 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::FileOperationFailed { .. }) => {
                failed = true;
                break;
            }
            Ok(ServerMessage::DecorationSet(_))
            | Ok(ServerMessage::DiagnosticSet(_) | ServerMessage::FoldingRangeSet(_))
            | Ok(ServerMessage::RuntimeDiagnostic(_))
            | Ok(ServerMessage::BehaviorManifest(_)) => {}
            Ok(other) => panic!("expected FileOperationFailed, got {other:?}"),
            Err(_) => panic!("timed out awaiting FileOperationFailed"),
        }
    }
    assert!(failed, "disappeared file fails without an open");
    connection.drain_bounded().await;
    let workspace = tab_state.workspace.lock().await;
    assert!(
        workspace
            .document_canonical_path(opened_document_id)
            .is_some()
    );
    assert!(
        workspace
            .document_canonical_path(opened_document_id + 1)
            .is_none(),
        "failed open allocates no document"
    );
    drop(workspace);

    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 10): secondary activation (Alt+Enter) on a selected
/// directory closes the path session and opens that directory as the
/// current tab's workspace — the browse activation itself is the
/// authorization event that converts ephemeral browse authority into a
/// `Directory` root grant. The bound tab's registry row rebinds to the
/// canonical directory root and the file browser refreshes to the new
/// root; a foreign tab's row is untouched; reopening the same directory
/// deduplicates to the same root id; secondary activation on a file
/// rejects without mutation.
#[tokio::test]
async fn path_browser_workspace_open_rebinds_only_bound_tab() {
    let root = temp_workspace("path-browser-open-workspace");
    fs::create_dir(root.join("alpha")).unwrap();
    fs::create_dir(root.join("alpha/inner")).unwrap();
    fs::write(root.join("alpha/file.txt"), "hi").unwrap();
    fs::write(root.join("beta.txt"), "b").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let alpha = fs::canonicalize(root.join("alpha")).unwrap();
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-open-workspace"),
    ));
    let (tab_snapshot, _tab_state) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    // The workspace pane is visible by default in fresh tab state, so the
    // rebind refresh carries the file-browser listing.
    // A second tab owned by a foreign client must stay untouched by the
    // bound tab's workspace open.
    let (foreign_snapshot, _) = server
        .create_tab_state(7, root.to_string_lossy().into_owned())
        .await
        .expect("foreign tab state is created");
    let foreign_tab_id = foreign_snapshot
        .tabs
        .iter()
        .find(|tab| tab.client_id == 7)
        .expect("foreign tab present")
        .tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;

    async fn open_path_browser(
        connection: &mut TestConnection,
        server: &super::super::IpcServer,
    ) -> ServerMessage {
        loop {
            connection
                .send(&ClientMessage::CommandIntent {
                    client_id: 11,
                    document_id: 1,
                    behavior_version: server.behavior.lock().await.version(),
                    command_id: "controlCenter.openPath".to_string(),
                })
                .await;
            loop {
                match timeout(Duration::from_secs(5), connection.receive()).await {
                    Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                    Ok(ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message,
                    }) if message.contains("behavior version is stale") => break,
                    Ok(_) => continue,
                    Err(_) => panic!("timed out awaiting path browser snapshot"),
                }
            }
        }
    }

    // Seed listing: directory-first order puts `alpha` at index 0.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    let names: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(names, vec!["alpha", "beta.txt"]);

    // Alt+Enter on the selected directory: the session closes first,
    // then the bound tab rebinds to the canonical directory root and the
    // file browser refreshes to the new root's listing.
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Secondary,
        })
        .await;
    let mut closed_id = None;
    let mut registry_snapshot = None;
    let mut saw_browser_refresh = false;
    for _ in 0..8 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TransientMenuClosed { session_id }) => {
                assert_eq!(session_id, path_id, "workspace open closes the session");
                closed_id = Some(session_id);
            }
            Ok(ServerMessage::TabRegistry(snapshot)) => registry_snapshot = Some(snapshot),
            Ok(ServerMessage::SduiSnapshot { tree, .. }) => {
                // The refresh is the file-browser tree for the new root.
                // Other snapshots (late reclaim follow-ups with the
                // hidden editor-only tree) are noise; keep scanning.
                // Directory labels carry a trailing separator ("inner/").
                let labels: Vec<String> = tree
                    .nodes
                    .iter()
                    .find_map(|node| match &node.kind {
                        SduiNodeKind::List { items, .. } => {
                            Some(items.iter().map(|item| item.label.clone()).collect())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();
                if labels.iter().any(|label| label == "inner/")
                    && labels.iter().any(|label| label == "file.txt")
                {
                    saw_browser_refresh = true;
                }
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting workspace rebind frames"),
        }
        if closed_id.is_some() && registry_snapshot.is_some() && saw_browser_refresh {
            break;
        }
    }
    assert_eq!(closed_id, Some(path_id));
    assert!(
        saw_browser_refresh,
        "file browser refresh for the new root never arrived"
    );
    let registry_snapshot = registry_snapshot.expect("TabRegistry snapshot after workspace open");
    let bound = registry_snapshot
        .tabs
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .expect("bound tab present");
    assert_eq!(bound.workspace_root, alpha.to_string_lossy().as_ref());
    let foreign = registry_snapshot
        .tabs
        .iter()
        .find(|tab| tab.tab_id == foreign_tab_id)
        .expect("foreign tab present");
    assert_eq!(
        foreign.workspace_root,
        root.to_string_lossy().as_ref(),
        "other tabs' roots are untouched"
    );
    let bound_root_id = registry_snapshot
        .tabs
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .expect("bound tab present")
        .workspace_root_id;
    connection.drain_bounded().await;

    // Reopening the same directory deduplicates to the same root id: the
    // tab's seed is now the rebound root, so ascend back to the original
    // root and activate `alpha` again.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    assert_eq!(
        snapshot.prompt,
        format!("Browse · {}", alpha.display()),
        "seed follows the rebound tab workspace root"
    );
    let path_id = snapshot.session_id;
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected ascent snapshot");
    };
    assert_eq!(snapshot.prompt, format!("Browse · {}", root.display()));
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Secondary,
        })
        .await;
    loop {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TabRegistry(snapshot)) => {
                let bound = snapshot
                    .tabs
                    .iter()
                    .find(|tab| tab.tab_id == tab_id)
                    .expect("bound tab present");
                assert_eq!(
                    bound.workspace_root_id, bound_root_id,
                    "same canonical directory deduplicates to the same root id"
                );
                break;
            }
            Ok(ServerMessage::TransientMenuClosed { .. }) => {}
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting deduplicated TabRegistry snapshot"),
        }
    }
    connection.drain_bounded().await;

    // Secondary activation on a file is not a workspace open: the
    // session closes and the bounded diagnostic names the rejection.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "file.txt".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filter snapshot");
    };
    assert_eq!(snapshot.items.len(), 1);
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Secondary,
        })
        .await;
    let mut saw_rejection = false;
    for _ in 0..4 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TransientMenuClosed { .. }) => {}
            Ok(ServerMessage::Error { message, .. }) => {
                assert!(
                    message.contains("no activation"),
                    "file has no secondary activation: {message}"
                );
                saw_rejection = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting file-activation rejection"),
        }
    }
    assert!(saw_rejection);

    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 10): secondary activation on a directory that
/// vanished between listing and activation rejects with the bounded
/// file-operation failure and leaves the tab's workspace root unchanged.
#[tokio::test]
async fn path_browser_workspace_open_rejects_vanished_directory() {
    let root = temp_workspace("path-browser-vanished-workspace");
    fs::create_dir(root.join("alpha")).unwrap();
    fs::write(root.join("beta.txt"), "b").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-vanished-workspace"),
    ));
    let (tab_snapshot, _) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;

    async fn open_path_browser(
        connection: &mut TestConnection,
        server: &super::super::IpcServer,
    ) -> ServerMessage {
        loop {
            connection
                .send(&ClientMessage::CommandIntent {
                    client_id: 11,
                    document_id: 1,
                    behavior_version: server.behavior.lock().await.version(),
                    command_id: "controlCenter.openPath".to_string(),
                })
                .await;
            loop {
                match timeout(Duration::from_secs(5), connection.receive()).await {
                    Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                    Ok(ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message,
                    }) if message.contains("behavior version is stale") => break,
                    Ok(_) => continue,
                    Err(_) => panic!("timed out awaiting path browser snapshot"),
                }
            }
        }
    }

    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    // The directory disappears after the listing installed.
    fs::remove_dir_all(root.join("alpha")).unwrap();
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Secondary,
        })
        .await;
    let mut saw_failure = false;
    for _ in 0..4 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TransientMenuClosed { session_id }) => {
                assert_eq!(session_id, path_id);
            }
            Ok(ServerMessage::FileOperationFailed {
                code: FileErrorCode::NotFound,
                workspace_root_id: None,
                document_id: None,
                ..
            }) => {
                saw_failure = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting vanished-directory failure"),
        }
    }
    assert!(saw_failure, "vanished directory fails with NotFound");

    // The tab's workspace root did not change: a fresh Path Browser still
    // seeds from the original root.
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    assert_eq!(
        snapshot.prompt,
        format!("Browse · {}", root.display()),
        "failed workspace open leaves the tab root unchanged"
    );

    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 10): the shared bound-tab workspace-open helper
/// rejects a connection with no bound tab before touching the workspace
/// or registry.
#[tokio::test]
async fn open_workspace_helper_requires_bound_tab() {
    let workspace = workspace_state();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let sdui = sdui_state();
    let (registry, tab_registry_tx) = two_tab_registry();
    let messages = open_workspace_for_bound_tab(
        &workspace,
        &document,
        &sdui,
        &registry,
        &tab_registry_tx,
        None,
        99,
        None,
        PathBuf::from("/tmp/unused"),
    )
    .await;
    assert_eq!(messages.len(), 1);
    match &messages[0] {
        ServerMessage::Error { message, .. } => {
            assert!(message.contains("requires a bound tab"));
        }
        other => panic!("expected bound-tab rejection, got {other:?}"),
    }
}

/// Phase 24.3 (task 11): browsing alone — open, filter, descend, ascend,
/// direct jump, cancel — never allocates a root grant or opens a
/// document. Activation is the single grant conversion point.
#[tokio::test]
async fn path_browser_navigation_only_creates_no_grants() {
    let root = temp_workspace("path-browser-navigation-only");
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
    fs::write(root.join("README.md"), "r").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-navigation-only"),
    ));
    let (tab_snapshot, tab_state) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let workspace = Arc::clone(&tab_state.workspace);
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;
    assert_eq!(workspace.lock().await.directory_roots().len(), 1);

    async fn open_path_browser(
        connection: &mut TestConnection,
        server: &super::super::IpcServer,
    ) -> ServerMessage {
        loop {
            connection
                .send(&ClientMessage::CommandIntent {
                    client_id: 11,
                    document_id: 1,
                    behavior_version: server.behavior.lock().await.version(),
                    command_id: "controlCenter.openPath".to_string(),
                })
                .await;
            loop {
                match timeout(Duration::from_secs(5), connection.receive()).await {
                    Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                    Ok(ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message,
                    }) if message.contains("behavior version is stale") => break,
                    Ok(_) => continue,
                    Err(_) => panic!("timed out awaiting path browser snapshot"),
                }
            }
        }
    }

    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;

    // Filter-only edit: no filesystem work, no grant.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: "RE".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(filtered) =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("filter snapshot")
    else {
        panic!("expected snapshot after filter edit");
    };
    assert_eq!(
        filtered.session_id, path_id,
        "session id stable across edits"
    );
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(filtered.items[0].label, "README.md");

    // Descend into src (primary on the directory after clearing the
    // filter): the session stays open and relists.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: String::new(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(_) =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("cleared filter snapshot")
    else {
        panic!("expected snapshot after clearing the filter");
    };
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(descended) =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("descend snapshot")
    else {
        panic!("expected snapshot after descend");
    };
    assert_eq!(
        descended.session_id, path_id,
        "session id stable across descend"
    );
    assert_eq!(
        descended.prompt,
        format!("Browse · {}/src", root.display()),
        "descend relists the canonical target"
    );
    assert_eq!(descended.items.len(), 1);
    assert_eq!(descended.items[0].label, "main.rs");

    // Ascend (Backspace on the empty filter) back to the tab root.
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(ascended) =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("ascend snapshot")
    else {
        panic!("expected snapshot after ascend");
    };
    assert_eq!(
        ascended.prompt,
        format!("Browse · {}", root.display()),
        "empty-filter Backspace ascends to the parent"
    );

    // Direct jump to a typed absolute directory.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: path_id,
            query: format!("{}/src/", root.display()),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(jumped) =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("direct jump snapshot")
    else {
        panic!("expected snapshot after direct jump");
    };
    assert_eq!(
        jumped.prompt,
        format!("Browse · {}/src", root.display()),
        "direct path edit jumps to the typed directory"
    );

    // Cancel: the session closes, still no grant or document.
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuClosed { session_id } =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("cancel frame")
    else {
        panic!("expected menu close");
    };
    assert_eq!(session_id, path_id);

    // Nothing but menu frames flowed, and the workspace gained no root.
    for _ in 0..16 {
        if timeout(
            Duration::from_millis(10),
            connection.codec.read_server_message(&mut connection.client),
        )
        .await
        .is_err()
        {
            break;
        }
    }
    assert_eq!(
        workspace.lock().await.directory_roots().len(),
        1,
        "browse navigation alone creates no root grants"
    );
    connection.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 11): a session id from one connection is opaque to
/// every other connection — cross-client activation fails closed with the
/// bounded `menu.unknown_session` diagnostic and never disturbs the
/// owning session.
#[tokio::test]
async fn path_browser_cross_client_activation_denied() {
    let root = temp_workspace("path-browser-cross-client");
    fs::write(root.join("a.txt"), "a").unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-cross-client"),
    ));
    let (tab_snapshot, _) = server
        .create_tab_state(11, root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let (foreign_snapshot, _) = server
        .create_tab_state(22, root.to_string_lossy().into_owned())
        .await
        .expect("foreign tab state is created");
    let foreign_tab_id = foreign_snapshot
        .tabs
        .iter()
        .find(|tab| tab.client_id == 22)
        .expect("foreign tab present")
        .tab_id;
    let mut connection_a = TestConnection::connect_with_server(11, server.clone()).await;
    connection_a.reclaim(11, tab_id).await;
    connection_a.drain_bounded().await;
    let mut connection_b = TestConnection::connect_with_server(22, server.clone()).await;
    connection_b.reclaim(22, foreign_tab_id).await;
    connection_b.drain_bounded().await;

    async fn open_path_browser(
        connection: &mut TestConnection,
        server: &super::super::IpcServer,
    ) -> ServerMessage {
        loop {
            connection
                .send(&ClientMessage::CommandIntent {
                    client_id: 11,
                    document_id: 1,
                    behavior_version: server.behavior.lock().await.version(),
                    command_id: "controlCenter.openPath".to_string(),
                })
                .await;
            loop {
                match timeout(Duration::from_secs(5), connection.receive()).await {
                    Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                    Ok(ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message,
                    }) if message.contains("behavior version is stale") => break,
                    Ok(_) => continue,
                    Err(_) => panic!("timed out awaiting path browser snapshot"),
                }
            }
        }
    }

    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection_a, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;

    // Client B cannot drive A's session: the id is per-connection opaque.
    connection_b
        .send(&ClientMessage::MenuActivate {
            client_id: 22,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    let mut saw_denial = false;
    for _ in 0..4 {
        match timeout(Duration::from_secs(2), connection_b.receive()).await {
            Ok(ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, message, .. }))
                if code == "menu.unknown_session" =>
            {
                assert!(message.contains(&path_id.to_string()));
                saw_denial = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting cross-client denial"),
        }
    }
    assert!(saw_denial, "foreign session id fails closed");

    // A's session is untouched: it still cancels with the expected frame.
    connection_a
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let ServerMessage::TransientMenuClosed { session_id } =
        timeout(Duration::from_secs(5), connection_a.receive())
            .await
            .expect("owner cancel frame")
    else {
        panic!("expected owner menu close");
    };
    assert_eq!(
        session_id, path_id,
        "owning connection still holds the session"
    );

    connection_a.close().await;
    connection_b.close().await;
    let _ = fs::remove_dir_all(&root);
}

/// Phase 24.3 (task 11): the per-connection session store survives tab
/// rebinds and drops cleanly on disconnect.
#[tokio::test]
async fn path_browser_survives_tab_switch_and_disconnect() {
    let root_a = temp_workspace("path-browser-tab-switch-a");
    fs::write(root_a.join("a.txt"), "a").unwrap();
    let root_b = temp_workspace("path-browser-tab-switch-b");
    fs::write(root_b.join("b.txt"), "b").unwrap();
    let root_a = fs::canonicalize(&root_a).unwrap();
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("path-browser-tab-switch"),
    ));
    let (tab_a_snapshot, _) = server
        .create_tab_state(11, root_a.to_string_lossy().into_owned())
        .await
        .expect("tab a is created");
    let (tab_b_snapshot, _) = server
        .create_tab_state(11, root_b.to_string_lossy().into_owned())
        .await
        .expect("tab b is created");
    let tab_a = tab_a_snapshot.tabs[0].tab_id;
    let tab_b = tab_b_snapshot.tabs[1].tab_id;
    assert_ne!(tab_a, tab_b);

    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_a).await;
    connection.drain_bounded().await;

    async fn open_path_browser(
        connection: &mut TestConnection,
        server: &super::super::IpcServer,
    ) -> ServerMessage {
        loop {
            connection
                .send(&ClientMessage::CommandIntent {
                    client_id: 11,
                    document_id: 1,
                    behavior_version: server.behavior.lock().await.version(),
                    command_id: "controlCenter.openPath".to_string(),
                })
                .await;
            loop {
                match timeout(Duration::from_secs(5), connection.receive()).await {
                    Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                    Ok(ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message,
                    }) if message.contains("behavior version is stale") => break,
                    Ok(_) => continue,
                    Err(_) => panic!("timed out awaiting path browser snapshot"),
                }
            }
        }
    }

    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;

    // Activating the second tab dismisses the session Escape-free (focus
    // loss) with the ordinary close frame; the session is then gone.
    connection
        .send(&ClientMessage::TabCommand {
            client_id: 11,
            command: crate::protocol::TabCommand::Activate { tab_id: tab_b },
        })
        .await;
    let ServerMessage::TransientMenuClosed { session_id } =
        timeout(Duration::from_secs(5), connection.receive())
            .await
            .expect("tab switch close frame")
    else {
        panic!("expected menu close on tab switch");
    };
    assert_eq!(session_id, path_id, "tab switch dismisses the session");
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let mut saw_unknown = false;
    for _ in 0..4 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, .. }))
                if code == "menu.unknown_session" =>
            {
                saw_unknown = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting unknown-session diagnostic"),
        }
    }
    assert!(saw_unknown, "dismissed session is gone");

    // Disconnect sweeps the store; `close` fails the test if the
    // connection task panicked or leaked a session.
    connection.close().await;
    let _ = fs::remove_dir_all(&root_a);
    let _ = fs::remove_dir_all(&root_b);
}

/// Phase 24.3 (task 11): after a runtime reload bumps the generation
/// stamp, an open session fails closed with the bounded stale-generation
/// diagnostic instead of executing against the old generation — and the
/// session is then gone, so cancel reports `menu.unknown_session`.
#[tokio::test]
async fn path_browser_activation_after_runtime_reload_fails_closed() {
    let config_root = temp_workspace("path-browser-reload-config");
    fs::write(config_root.join("init.js"), "").unwrap();
    let workspace_root = temp_workspace("path-browser-reload-workspace");
    fs::write(workspace_root.join("a.txt"), "a").unwrap();
    let workspace_root = fs::canonicalize(&workspace_root).unwrap();
    let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
        "path-browser-reload",
    ));
    config.configuration_root = Some(config_root.clone());
    let server = super::super::IpcServer::new(config);
    let (tab_snapshot, _) = server
        .create_tab_state(11, workspace_root.to_string_lossy().into_owned())
        .await
        .expect("tab state is created");
    let tab_id = tab_snapshot.tabs[0].tab_id;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;

    async fn open_path_browser(
        connection: &mut TestConnection,
        server: &super::super::IpcServer,
    ) -> ServerMessage {
        loop {
            connection
                .send(&ClientMessage::CommandIntent {
                    client_id: 11,
                    document_id: 1,
                    behavior_version: server.behavior.lock().await.version(),
                    command_id: "controlCenter.openPath".to_string(),
                })
                .await;
            loop {
                match timeout(Duration::from_secs(5), connection.receive()).await {
                    Ok(message @ ServerMessage::TransientMenuSnapshot(_)) => return message,
                    Ok(ServerMessage::Error {
                        code: ProtocolErrorCode::InvalidMessage,
                        message,
                    }) if message.contains("behavior version is stale") => break,
                    Ok(_) => continue,
                    Err(_) => panic!("timed out awaiting path browser snapshot"),
                }
            }
        }
    }

    let ServerMessage::TransientMenuSnapshot(snapshot) =
        open_path_browser(&mut connection, &server).await
    else {
        panic!("expected path browser snapshot");
    };
    let path_id = snapshot.session_id;
    assert_eq!(server.runtime_generation.generation_id().await, 1);

    let outcome = server.reload_runtime_generation().await;
    assert!(
        outcome.reloaded,
        "reload succeeds with the empty init.js config"
    );
    assert_eq!(server.runtime_generation.generation_id().await, 2);

    // Activation with the old stamp fails closed: bounded diagnostic, no
    // execution, and the session is consumed like any rejected activation.
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: path_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    // Reload publishes the new generation snapshot; the loop closes the
    // active session first (the catalogue is generation-bound), exactly
    // like a tab switch dismisses on focus loss.
    let mut saw_closed = false;
    for _ in 0..8 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TransientMenuClosed { session_id }) => {
                assert_eq!(session_id, path_id);
                saw_closed = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting reload dismissal"),
        }
    }
    assert!(saw_closed, "runtime reload dismisses the active session");

    // The session is gone: cancel reports the bounded unknown-session
    // diagnostic rather than a spurious close.
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: path_id,
        })
        .await;
    let mut saw_unknown = false;
    for _ in 0..4 {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic { code, .. }))
                if code == "menu.unknown_session" =>
            {
                saw_unknown = true;
                break;
            }
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting unknown-session diagnostic"),
        }
    }
    assert!(saw_unknown, "cancelled session reports unknown_session");

    connection.close().await;
    let _ = fs::remove_dir_all(&config_root);
    let _ = fs::remove_dir_all(&workspace_root);
}

/// Phase 24.3 (task 11): the package/generic command lane cannot open
/// the Path Browser. Only the connection's `CommandIntent` special case
/// and the Control Centre catalogue's `MenuActivate` special case reach
/// the session store; the shared executor (the lane package callbacks
/// run through) yields nothing on the wire for the built-in id.
#[tokio::test]
async fn package_command_lane_cannot_open_path_browser() {
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("package-lane-path-browser"),
    ));
    let response = execute_command_intent(
        CommandExecutionRequest {
            command_id: "controlCenter.openPath".to_string(),
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::ActiveDocument { document_id: 1 },
            provenance: None,
            expected_permissions: Vec::new(),
        },
        Arc::clone(&server.workspace),
        &server.document,
        &server.sdui,
        1,
        Some(&server),
        &CommandRegistry::new(),
    )
    .await;
    assert!(
        response.is_none(),
        "generic execution of controlCenter.openPath must yield no message"
    );
}

/// Phase 24.1: menu intents naming sessions this connection does not hold
/// are dropped with a bounded `menu.unknown_session` diagnostic — never an
/// error or disconnect. Sessions are per-tab (per-connection), so the
/// connection must be tab-bound first.
#[tokio::test]
async fn menu_intents_for_unknown_sessions_produce_bounded_diagnostics() {
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("menu-unknown-session"),
    ));
    let (snapshot, _state) = server
        .create_tab_state(
            11,
            temp_workspace("menu-unknown")
                .to_string_lossy()
                .into_owned(),
        )
        .await
        .expect("tab state is created");
    let tab_id = snapshot.tabs[0].tab_id;

    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, tab_id).await;
    connection.drain_bounded().await;

    for message in [
        ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: 1 << 63 | 7,
            query: "reload".to_string(),
            scope: None,
        },
        ClientMessage::MenuSelectionMove {
            client_id: 11,
            session_id: 1 << 63 | 7,
            delta: 1,
        },
        ClientMessage::MenuActivate {
            client_id: 11,
            session_id: 1 << 63 | 7,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        },
        ClientMessage::MenuCancel {
            client_id: 11,
            session_id: 1 << 63 | 7,
        },
    ] {
        connection.send(&message).await;
        let response = connection.receive().await;
        assert!(
            matches!(
                response,
                ServerMessage::RuntimeDiagnostic(ref diagnostic)
                    if diagnostic.code == "menu.unknown_session"
            ),
            "unexpected response: {response:?}"
        );
    }

    // The connection is still alive and functional (never a disconnect).
    connection
        .send(&ClientMessage::ListDocuments { client_id: 11 })
        .await;
    assert!(matches!(
        connection.receive_response().await,
        ServerMessage::DocumentList { .. }
    ));
    connection.close().await;
}

/// Read until a transient-menu frame arrives, skipping parse/activation
/// noise that can race a menu exchange.
async fn receive_menu_message(connection: &mut TestConnection) -> ServerMessage {
    loop {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(
                message @ (ServerMessage::TransientMenuSnapshot(_)
                | ServerMessage::TransientMenuClosed { .. }),
            ) => return message,
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting transient menu frame"),
        }
    }
}

#[tokio::test]
async fn control_center_opens_filters_activates_and_cancels() {
    // Plan 086 task 7: whole-workflow bound. A hang here means pending
    // session cleanup, not a slow machine (measured ~0.03s); the timeout
    // names the failure instead of waiting indefinitely.
    timeout(
        Duration::from_secs(5),
        control_center_opens_filters_activates_and_cancels_scenario(),
    )
    .await
    .expect(
        "control_center_opens_filters_activates_and_cancels exceeded its 5s whole-workflow bound; \
         look for pending session or reply-receiver cleanup",
    );
}

async fn control_center_opens_filters_activates_and_cancels_scenario() {
    let root = temp_workspace("control-center");
    // Hermetic configuration root (Phase 24.5, task 8): without an
    // explicit root this test fell back to the real ~/.clay and
    // hung whenever that directory contains an init.js (reload evaluates
    // the live user config). The sentinel typography proves the hermetic
    // root — not ambient ~/.clay — is the generation source: an
    // ambient fallback would load the default 20px monospace, not 21px.
    let config_root = temp_workspace("control-center-config");
    fs::write(
        config_root.join("init.js"),
        "import { setTypography } from \"clay:theme\"; setTypography({ monospace: { families: [\"MartianMono Nerd Font\", \"monospace\"], size: 21 }, proportional: { families: [\"Noto Sans\", \"sans-serif\"], size: 17 }, ui: { families: [\"system-ui\"], size: 13 } });",
    )
    .unwrap();
    let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
        "control-center-open",
    ));
    config.workspace_roots.push(root.clone());
    config.configuration_root = Some(config_root);
    let server = super::super::IpcServer::new(config);
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    let open = |behavior_version| ClientMessage::CommandIntent {
        client_id: 11,
        document_id: 1,
        behavior_version,
        command_id: "controlCenter.open".to_string(),
    };
    let mut behavior_version = server.behavior.lock().await.version();

    // Opening replaces any active session: the first open delivers a
    // snapshot, a second open closes the old session and returns a
    // distinct new session id.
    connection.send(&open(behavior_version)).await;
    let ServerMessage::TransientMenuSnapshot(first_snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected first TransientMenuSnapshot");
    };
    let first_session_id = first_snapshot.session_id;
    connection.send(&open(behavior_version)).await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id }
            if session_id == first_session_id
    ));
    let ServerMessage::TransientMenuSnapshot(second_snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected replacement TransientMenuSnapshot");
    };
    let second_session_id = second_snapshot.session_id;
    assert_ne!(first_session_id, second_session_id);
    // Plan 124 task 7: the catalogue session is the composer's `/` palette — the
    // client anchors it to the lane's composer box (bottom anchor), names it
    // "Commands", and opens it unfiltered (the field below is the query).
    assert_eq!(
        second_snapshot.origin,
        crate::protocol::TransientMenuOriginData::CommandPalette,
        "the palette must declare the bottom/composer anchor, not a window sheet"
    );
    assert_eq!(second_snapshot.prompt, "Commands");
    assert!(!second_snapshot.items.is_empty());
    let opened_item_count = second_snapshot.items.len();

    // A stale selection move against the replaced session is a bounded
    // diagnostic, never an error or disconnect.
    connection
        .send(&ClientMessage::MenuSelectionMove {
            client_id: 11,
            session_id: first_session_id,
            delta: 1,
        })
        .await;
    assert!(matches!(
        connection.receive().await,
        ServerMessage::RuntimeDiagnostic(ref diagnostic)
            if diagnostic.code == "menu.unknown_session"
    ));

    // Query filtering narrows the live session's items.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: second_session_id,
            query: "reload".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(filtered) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filtered TransientMenuSnapshot");
    };
    assert!(
        !filtered.items.is_empty() && filtered.items.iter().all(|item| item.id.contains("reload")),
        "filtered items must all match the query: {:?}",
        filtered
            .items
            .iter()
            .map(|item| &item.id)
            .collect::<Vec<_>>()
    );
    // The palette rides one session across keystrokes: the filter update keeps
    // the same id and the same anchor, and clearing it restores the whole
    // open-time catalogue (the session holds it — no query rebuilds it).
    assert_eq!(filtered.session_id, second_session_id);
    assert_eq!(
        filtered.origin,
        crate::protocol::TransientMenuOriginData::CommandPalette
    );
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: second_session_id,
            query: String::new(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(restored) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected restored TransientMenuSnapshot");
    };
    assert_eq!(restored.session_id, second_session_id);
    assert_eq!(restored.items.len(), opened_item_count);

    // Plan 124: the palette's rows carry the server's own scope vocabulary and
    // their chords, so the sheet draws chips from data instead of guessing.
    let toggle_lane = restored
        .items
        .iter()
        .find(|item| item.id == "shell.toggleAgentLane")
        .expect("the lane toggle is a palette row");
    assert_eq!(toggle_lane.scope.as_deref(), Some("shell"));
    assert_eq!(toggle_lane.bindings, ["Ctrl+X Ctrl+P"]);

    // The scope chip rides the same update as the query: the *session* filters
    // by it, so the client never hides a row the server still selects over.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: second_session_id,
            query: String::new(),
            scope: Some("files".to_string()),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(scoped) = receive_menu_message(&mut connection).await
    else {
        panic!("expected scoped TransientMenuSnapshot");
    };
    assert_eq!(scoped.session_id, second_session_id);
    assert_eq!(
        scoped
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["controlCenter.openPath"],
        "the Files chip shows the palette's own path mode and nothing else"
    );
    assert_eq!(scoped.selected_index, 0);

    // The vocabulary is closed: an unknown word is not a scope (the chip falls
    // back to All), and the session survives it.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: second_session_id,
            query: String::new(),
            scope: Some("not-a-scope".to_string()),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(unscoped) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected unscoped TransientMenuSnapshot");
    };
    assert_eq!(unscoped.session_id, second_session_id);
    assert_eq!(unscoped.items.len(), opened_item_count);

    // Leave the palette filtered for the activation below.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id: second_session_id,
            query: "reload".to_string(),
            scope: None,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuSnapshot(_)
    ));

    // Activating the selected item closes the menu and executes the
    // server command; the reload fanout (diagnostic + snapshot) arrives
    // asynchronously after the close.
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id: second_session_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id }
            if session_id == second_session_id
    ));
    let mut saw_reload_diagnostic = false;
    let mut saw_runtime_snapshot = false;
    for _ in 0..64 {
        match connection.receive().await {
            ServerMessage::RuntimeDiagnostic(ref diagnostic)
                if diagnostic.code == "runtime.reload_succeeded" =>
            {
                saw_reload_diagnostic = true;
            }
            ServerMessage::RuntimeStateSnapshot(snapshot) => {
                saw_runtime_snapshot = true;
                behavior_version = snapshot.behavior.behavior_version;
            }
            ServerMessage::BehaviorManifest(manifest) => {
                behavior_version = manifest.behavior_version;
            }
            _ => {}
        }
        if saw_reload_diagnostic && saw_runtime_snapshot {
            break;
        }
    }
    assert!(
        saw_reload_diagnostic && saw_runtime_snapshot,
        "reload fanout must deliver the diagnostic and snapshot"
    );
    // The hermetic root (not ambient ~/.clay) was the reload
    // source: the sentinel typography from its init.js is now live.
    assert_eq!(
        server
            .runtime_generation
            .active_typography()
            .await
            .monospace
            .size,
        21.0,
        "reloaded generation must come from the hermetic config root"
    );

    // Reopening after the generation replacement yields a fresh session
    // id (the old generation's session is gone).
    connection.send(&open(behavior_version)).await;
    let ServerMessage::TransientMenuSnapshot(third_snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected reopened TransientMenuSnapshot");
    };
    assert_ne!(third_snapshot.session_id, second_session_id);

    // Escape (MenuCancel) closes the active session with a close frame.
    connection
        .send(&ClientMessage::MenuCancel {
            client_id: 11,
            session_id: third_snapshot.session_id,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id }
            if session_id == third_snapshot.session_id
    ));

    // The connection is still alive and functional.
    connection
        .send(&ClientMessage::ListDocuments { client_id: 11 })
        .await;
    assert!(matches!(
        connection.receive_response().await,
        ServerMessage::DocumentList { .. }
    ));
    connection.drain_bounded().await;
    connection.close().await;
}

#[tokio::test]
async fn control_center_shell_activation_sends_shell_command_request() {
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("control-center-shell"),
    ));
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    let behavior_version = server.behavior.lock().await.version();
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.open".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    let session_id = snapshot.session_id;
    assert!(
        snapshot
            .items
            .iter()
            .any(|item| item.id == "shell.clientSplitPaneVertical"),
        "shell.client* entries must appear in the Control Center listing"
    );

    // Fuzzy query narrows to exactly the shell entry.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id,
            query: "clientSplitPaneVertical".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(filtered) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filtered TransientMenuSnapshot");
    };
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(filtered.items[0].id, "shell.clientSplitPaneVertical");

    // Activation closes the menu, then ships the narrow shell-command
    // request frame the client re-parses deny-by-default.
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id,
            kind: crate::protocol::TransientMenuActivationData::Primary,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == session_id
    ));
    assert_eq!(
        connection.receive().await,
        ServerMessage::ShellClientCommandRequest {
            command_id: "shell.clientSplitPaneVertical".to_string(),
        }
    );
    connection.drain_bounded().await;
    connection.close().await;
}

#[tokio::test]
async fn menu_backspace_deletes_one_char_and_secondary_activation_matches_primary() {
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("menu-backspace-secondary"),
    ));
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    let behavior_version = server.behavior.lock().await.version();
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.open".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    let session_id = snapshot.session_id;

    // Backspace on an empty query is a bounded no-op snapshot.
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(after_empty_backspace) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    assert_eq!(after_empty_backspace.query, "");

    // Backspace deletes exactly one query character (Control Center
    // semantics; path mode overrides with ascend in task 8).
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id,
            query: "clientSplitPaneVertical".to_string(),
            scope: None,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(filtered) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected filtered TransientMenuSnapshot");
    };
    assert_eq!(filtered.items.len(), 1);
    connection
        .send(&ClientMessage::MenuBackspace {
            client_id: 11,
            session_id,
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(after_backspace) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    assert_eq!(after_backspace.query, "clientSplitPaneVertica");

    // Restore the exact query, then activate with the secondary kind:
    // the Control Center executes the same selection as primary.
    connection
        .send(&ClientMessage::MenuQueryUpdate {
            client_id: 11,
            session_id,
            query: "clientSplitPaneVertical".to_string(),
            scope: None,
        })
        .await;
    let _ = receive_menu_message(&mut connection).await;
    connection
        .send(&ClientMessage::MenuActivate {
            client_id: 11,
            session_id,
            kind: crate::protocol::TransientMenuActivationData::Secondary,
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == session_id
    ));
    assert_eq!(
        connection.receive().await,
        ServerMessage::ShellClientCommandRequest {
            command_id: "shell.clientSplitPaneVertical".to_string(),
        }
    );
    connection.drain_bounded().await;
    connection.close().await;
}

#[tokio::test]
async fn runtime_generation_replacement_cancels_open_control_center() {
    // Plan 086 task 7: whole-workflow bound. A hang here means the
    // replacement left a pending session or reply receiver; the timeout
    // names that instead of waiting indefinitely.
    timeout(
        Duration::from_secs(5),
        runtime_generation_replacement_cancels_open_control_center_scenario(),
    )
    .await
    .expect(
        "runtime_generation_replacement_cancels_open_control_center exceeded its 5s whole-workflow bound; \
         look for pending session or reply-receiver cleanup",
    );
}

async fn runtime_generation_replacement_cancels_open_control_center_scenario() {
    // Hermetic configuration root (Phase 24.5, task 8): same real-config
    // fallback hazard as control_center_opens_filters_activates_and_cancels.
    // Sentinel typography proves the hermetic root is the generation
    // source (ambient ~/.clay must never load).
    let config_root = temp_workspace("control-center-generation-config");
    fs::write(
        config_root.join("init.js"),
        "import { setTypography } from \"clay:theme\"; setTypography({ monospace: { families: [\"MartianMono Nerd Font\", \"monospace\"], size: 21 }, proportional: { families: [\"Noto Sans\", \"sans-serif\"], size: 17 }, ui: { families: [\"system-ui\"], size: 13 } });",
    )
    .unwrap();
    let mut config = super::super::ServerConfig::new(crate::ipc::IpcEndpoint::from_argument(
        "control-center-generation",
    ));
    config.configuration_root = Some(config_root);
    let server = super::super::IpcServer::new(config);
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    let mut behavior_version = server.behavior.lock().await.version();
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.open".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    let session_id = snapshot.session_id;

    // A direct reload while the menu is open replaces the runtime
    // generation; the broadcast cancels the open menu session before
    // replaying the replacement state.
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "runtime.reloadConfiguration".to_string(),
        })
        .await;
    let mut saw_reload_diagnostic = false;
    let mut saw_menu_close = false;
    let mut saw_runtime_snapshot = false;
    for _ in 0..64 {
        match connection.receive().await {
            ServerMessage::RuntimeDiagnostic(ref diagnostic)
                if diagnostic.code == "runtime.reload_succeeded" =>
            {
                saw_reload_diagnostic = true;
            }
            ServerMessage::TransientMenuClosed { session_id: closed } if closed == session_id => {
                saw_menu_close = true;
            }
            ServerMessage::RuntimeStateSnapshot(snapshot) => {
                saw_runtime_snapshot = true;
                behavior_version = snapshot.behavior.behavior_version;
            }
            ServerMessage::BehaviorManifest(manifest) => {
                behavior_version = manifest.behavior_version;
            }
            _ => {}
        }
        if saw_reload_diagnostic && saw_menu_close && saw_runtime_snapshot {
            break;
        }
    }
    assert!(
        saw_reload_diagnostic && saw_menu_close && saw_runtime_snapshot,
        "generation replacement must close the open menu and replay state"
    );
    // The hermetic root (not ambient ~/.clay) was the reload
    // source: the sentinel typography from its init.js is now live.
    assert_eq!(
        server
            .runtime_generation
            .active_typography()
            .await
            .monospace
            .size,
        21.0,
        "replaced generation must come from the hermetic config root"
    );

    // The reopened menu is stamped with the replacement generation and
    // gets a distinct session id.
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.open".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(reopened) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected reopened TransientMenuSnapshot");
    };
    assert_ne!(reopened.session_id, session_id);

    // No pending session survives the replacement: a stale selection
    // against the cancelled session id is a bounded diagnostic, never a
    // reply from a live session or a hang.
    connection
        .send(&ClientMessage::MenuSelectionMove {
            client_id: 11,
            session_id,
            delta: 1,
        })
        .await;
    assert!(matches!(
        connection.receive().await,
        ServerMessage::RuntimeDiagnostic(ref diagnostic)
            if diagnostic.code == "menu.unknown_session"
    ));
    connection.drain_bounded().await;
    connection.close().await;
}

#[tokio::test]
async fn tab_switch_cancels_the_active_server_menu_session() {
    let root_a = temp_workspace("menu-tab-alpha");
    let root_b = temp_workspace("menu-tab-beta");
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("menu-tab-switch"),
    ));
    let (first_snapshot, _) = server
        .create_tab_state(11, root_a.to_string_lossy().into_owned())
        .await
        .expect("first tab state is created");
    let first_tab = first_snapshot.tabs[0].tab_id;
    let (second_snapshot, _) = server
        .create_tab_state(11, root_b.to_string_lossy().into_owned())
        .await
        .expect("second tab state is created");
    let second_tab = second_snapshot.tabs[0].tab_id;

    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.reclaim(11, first_tab).await;
    connection.drain_bounded().await;
    let behavior_version = server.behavior.lock().await.version();
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "controlCenter.open".to_string(),
        })
        .await;
    let ServerMessage::TransientMenuSnapshot(snapshot) =
        receive_menu_message(&mut connection).await
    else {
        panic!("expected TransientMenuSnapshot");
    };
    let session_id = snapshot.session_id;

    // Switching to the second tab dismisses the open menu (Escape-free
    // dismissal on focus loss) with an explicit close frame.
    connection
        .send(&ClientMessage::TabCommand {
            client_id: 11,
            command: crate::protocol::TabCommand::Activate { tab_id: second_tab },
        })
        .await;
    assert!(matches!(
        receive_menu_message(&mut connection).await,
        ServerMessage::TransientMenuClosed { session_id: closed }
            if closed == session_id
    ));
    connection.drain_bounded().await;
    connection.close().await;
}

#[tokio::test]
async fn agent_lane_toggle_projects_a_shell_client_request() {
    // Plan 124: `shell.toggleAgentLane` is declared ClientUi in the default
    // manifest, so its Global ServerFirst `Ctrl+X Ctrl+P` intent must come
    // back as the narrow shell-client request the shell executes (the lane's
    // visibility is client-local per-tab layout state) — not a wire error
    // from the server command executor, and no runtime generation bump.
    let server = super::super::IpcServer::new(super::super::ServerConfig::new(
        crate::ipc::IpcEndpoint::from_argument("agent-lane-toggle"),
    ));
    let generation_before = server.runtime_generation.generation_id().await;
    let mut connection = TestConnection::connect_with_server(11, server.clone()).await;
    connection.drain_bounded().await;
    let behavior_version = server.behavior.lock().await.version();
    connection
        .send(&ClientMessage::CommandIntent {
            client_id: 11,
            document_id: 1,
            behavior_version,
            command_id: "shell.toggleAgentLane".to_string(),
        })
        .await;
    assert_eq!(
        connection.receive().await,
        ServerMessage::ShellClientCommandRequest {
            command_id: "shell.toggleAgentLane".to_string(),
        }
    );
    assert_eq!(
        server.runtime_generation.generation_id().await,
        generation_before,
        "a client-local toggle must not advance the runtime generation"
    );
    connection.close().await;
}

#[tokio::test]
async fn package_command_dispatchs_through_shared_dispatcher_with_live_registry() {
    // A validated package command resolves through the live aggregated
    // registry passed by the menu-activation path (not the empty registry
    // SDUI/CommandIntent use): the dispatcher returns `None` (Accepted —
    // the JS side effect runs in the package runtime, no wire message).
    let mut registry = CommandRegistry::new();
    registry.insert_test_command(crate::packages::commands::RegisteredCommand {
        command_id: "markdown.togglePreview".to_string(),
        display_name: "Toggle Markdown Preview".to_string(),
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        api_prefix: "markdown".to_string(),
        routing_policy: crate::protocol::RoutingPolicy::ServerFirst,
        key_bindings: Vec::new(),
        permissions: vec![crate::packages::permissions::PackagePermission::ParseDocument],
        custom_properties: BTreeMap::new(),
    });
    let response = execute_command_intent(
        CommandExecutionRequest {
            command_id: "markdown.togglePreview".to_string(),
            arguments: serde_json::Value::Null,
            target: CommandExecutionTarget::ActiveDocument { document_id: 1 },
            provenance: None,
            expected_permissions: Vec::new(),
        },
        workspace_state(),
        &document_state(),
        &sdui_state(),
        1,
        None,
        &registry,
    )
    .await;
    assert_eq!(response, None, "validated package commands accept silently");
}

/// Read until a `TabRegistry` snapshot arrives (skipping unrelated
/// frames that can race the tab-command exchange).
async fn receive_tab_registry_snapshot(
    connection: &mut TestConnection,
) -> crate::protocol::TabRegistrySnapshot {
    loop {
        match timeout(Duration::from_secs(2), connection.receive()).await {
            Ok(ServerMessage::TabRegistry(snapshot)) => return snapshot,
            Ok(_) => continue,
            Err(_) => panic!("timed out awaiting TabRegistry snapshot"),
        }
    }
}

/// A shared registry seeded with two tabs: tab 1 bound to client 99 (the
/// test connection's identity) and tab 2 bound to a foreign client (7).
fn two_tab_registry() -> (
    Arc<Mutex<crate::server::tab_registry::TabRegistry>>,
    tokio::sync::broadcast::Sender<crate::protocol::TabRegistrySnapshot>,
) {
    let mut registry = crate::server::tab_registry::TabRegistry::new();
    registry.create_tab(99, 1, "/workspaces/alpha".to_string());
    registry.create_tab(7, 2, "/workspaces/beta".to_string());
    let registry = Arc::new(Mutex::new(registry));
    let (tab_registry_tx, _) = tokio::sync::broadcast::channel(16);
    (registry, tab_registry_tx)
}

/// Phase 22.7 (task 3): a rejected `Close` (foreign tab) pushes the
/// reconciling snapshot and the sender's connection keeps serving.
#[tokio::test]
async fn rejected_close_keeps_connection_serving() {
    let root = temp_workspace("rejected-close");
    let mut workspace_state_value = WorkspaceState::new();
    workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let runtime_generation = runtime_generation();
    let parse_coordinator = parse_coordinator();
    let document_analysis =
        crate::server::document_analysis::DocumentAnalysisCoordinator::default();
    let (registry, tab_registry_tx) = two_tab_registry();
    let mut connection = TestConnection::connect_with_registry(
        99,
        document,
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
        Arc::clone(&registry),
        tab_registry_tx,
    )
    .await;

    // A (client 99) tries to close B's tab (tab 2): rejected.
    connection
        .send(&ClientMessage::TabCommand {
            client_id: 99,
            command: crate::protocol::TabCommand::Close { tab_id: 2 },
        })
        .await;
    let snapshot = receive_tab_registry_snapshot(&mut connection).await;
    // Registry unchanged: both tabs still bound, tab 2 still owned by 7.
    assert_eq!(snapshot.tabs.len(), 2);
    assert!(
        snapshot
            .tabs
            .iter()
            .any(|entry| entry.tab_id == 2 && entry.client_id == 7)
    );
    assert!(
        snapshot
            .tabs
            .iter()
            .any(|entry| entry.tab_id == 1 && entry.client_id == 99)
    );

    // A's next command still processes: activate its own tab succeeds and
    // pushes another snapshot (the connection never ended).
    connection
        .send(&ClientMessage::TabCommand {
            client_id: 99,
            command: crate::protocol::TabCommand::Activate { tab_id: 1 },
        })
        .await;
    let snapshot = receive_tab_registry_snapshot(&mut connection).await;
    assert_eq!(snapshot.active, Some(1));

    assert!(registry.lock().await.snapshot().tabs.len() == 2);
    connection.close().await;
}

/// Phase 22.7 (task 3): an accepted `Close` (own tab) still ends the
/// connection (EOF on the client stream) and removes the tab.
#[tokio::test]
async fn accepted_close_still_ends_connection() {
    let root = temp_workspace("accepted-close");
    let mut workspace_state_value = WorkspaceState::new();
    workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let runtime_generation = runtime_generation();
    let parse_coordinator = parse_coordinator();
    let document_analysis =
        crate::server::document_analysis::DocumentAnalysisCoordinator::default();
    let (registry, tab_registry_tx) = two_tab_registry();
    let mut connection = TestConnection::connect_with_registry(
        99,
        document,
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
        Arc::clone(&registry),
        tab_registry_tx,
    )
    .await;

    // A closes its own tab (tab 1): accepted, the connection ends.
    connection
        .send(&ClientMessage::TabCommand {
            client_id: 99,
            command: crate::protocol::TabCommand::Close { tab_id: 1 },
        })
        .await;
    // The server task resolves and the client stream reaches EOF.
    timeout(
        Duration::from_secs(2),
        connection.codec.read_server_message(&mut connection.client),
    )
    .await
    .expect("EOF expected within the timeout")
    .expect_err("accepted close must end the connection (EOF)");
    // The tab is gone from the shared registry; the foreign tab remains.
    let snapshot = registry.lock().await.snapshot();
    assert_eq!(snapshot.tabs.len(), 1);
    assert!(
        snapshot
            .tabs
            .iter()
            .any(|entry| entry.tab_id == 2 && entry.client_id == 7)
    );
}

/// Plan 060 T4 (P0-2): one pre-dispatch boundary rejects every legacy
/// message whose `client_id` does not match the handshake-assigned
/// connection identity. Table covers every post-Hello family.
#[tokio::test]
async fn forged_client_identity_is_rejected_for_every_message_family() {
    let root = temp_workspace("forged-identity");
    fs::write(root.join("note.md"), "# secret\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let runtime_generation = runtime_generation();
    let parse_coordinator = parse_coordinator();
    let document_analysis =
        crate::server::document_analysis::DocumentAnalysisCoordinator::default();
    let mut connection = TestConnection::connect(
        99,
        document,
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
    )
    .await;

    let forged: Vec<(&str, ClientMessage)> = vec![
        (
            "Edit",
            ClientMessage::Edit {
                document_id: 7,
                client_id: 1,
                lease_id: Some(1),
                base_version: 1,
                behavior_version: 1,
                transaction_id: 1,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "x".to_string(),
                },
            },
        ),
        (
            "EditorIntent",
            ClientMessage::EditorIntent {
                document_id: 7,
                client_id: 1,
                lease_id: Some(1),
                base_version: 1,
                behavior_version: 1,
                transaction_id: 2,
                intent: crate::protocol::EditorIntent::InsertText {
                    byte_offset: 0,
                    text: "x".to_string(),
                },
            },
        ),
        (
            "RequestResync",
            ClientMessage::RequestResync {
                document_id: 7,
                client_id: 1,
                known_version: 0,
            },
        ),
        (
            "ViewportRenderRequest",
            ClientMessage::ViewportRenderRequest {
                client_id: 1,
                document_id: 7,
                document_version: 1,
                request_id: 1,
                byte_start: 0,
                byte_end: 1,
                trace_id: None,
            },
        ),
        (
            "OpenDocument",
            ClientMessage::OpenDocument {
                client_id: 1,
                workspace_root_id: root_id,
                path: "note.md".to_string(),
            },
        ),
        (
            "OpenSelectedFile",
            ClientMessage::OpenSelectedFile {
                client_id: 1,
                capability: "forged".to_string(),
                selected_path: root.join("note.md").to_string_lossy().into_owned(),
            },
        ),
        (
            "AddSelectedWorkspaceRoot",
            ClientMessage::AddSelectedWorkspaceRoot {
                client_id: 1,
                capability: "forged".to_string(),
                selected_path: root.to_string_lossy().into_owned(),
            },
        ),
        (
            "SaveDocument",
            ClientMessage::SaveDocument {
                client_id: 1,
                document_id: 7,
                known_version: 1,
            },
        ),
        (
            "ReloadDocument",
            ClientMessage::ReloadDocument {
                client_id: 1,
                document_id: 7,
                known_version: 1,
                force: true,
            },
        ),
        (
            "GetDocumentStatus",
            ClientMessage::GetDocumentStatus {
                client_id: 1,
                document_id: 7,
            },
        ),
        (
            "ListDocuments",
            ClientMessage::ListDocuments { client_id: 1 },
        ),
        (
            "SduiAction",
            ClientMessage::SduiAction {
                client_id: 1,
                ui_version: 1,
                intent: SduiActionIntent::command(
                    "controlCenter.open",
                    SduiActionSource::Button {
                        node_id: SduiNodeId(1),
                    },
                ),
            },
        ),
        (
            "CommandIntent",
            ClientMessage::CommandIntent {
                client_id: 1,
                document_id: 7,
                behavior_version: 1,
                command_id: "controlCenter.open".to_string(),
            },
        ),
        (
            "CompletionRequest",
            ClientMessage::CompletionRequest {
                request: crate::protocol::CompletionRequest {
                    request_id: 1,
                    client_id: 1,
                    document_id: 7,
                    document_version: 1,
                    behavior_version: 1,
                    cursor_byte_offset: 0,
                    replacement_range: crate::protocol::CompletionReplacementRange::new(0, 0),
                    trigger: crate::protocol::CompletionTrigger::Manual,
                    provider_generation: 1,
                    recent_completions: Vec::<String>::new().into_boxed_slice(),
                },
            },
        ),
        (
            "LanguageIntelligenceRequest",
            ClientMessage::LanguageIntelligenceRequest {
                request: crate::protocol::LanguageIntelligenceRequest {
                    request_id: 1,
                    client_id: 1,
                    document_id: 7,
                    document_version: 1,
                    behavior_version: 1,
                    cursor_byte_offset: 0,
                    feature: crate::protocol::LanguageIntelligenceFeature::Hover,
                    provider_generation: 1,
                },
            },
        ),
        (
            "RuntimeGenerationInstalled",
            ClientMessage::RuntimeGenerationInstalled {
                client_id: 1,
                runtime_generation_id: 1,
            },
        ),
    ];

    for (family, message) in forged {
        connection.send(&message).await;
        let response = connection.receive().await;
        assert!(
            matches!(
                response,
                ServerMessage::Error {
                    code: ProtocolErrorCode::InvalidMessage,
                    ..
                }
            ),
            "forged {family} must be rejected at the identity boundary, got {response:?}"
        );
    }

    // The forged OpenDocument must have had no effect: the connection's own
    // document list stays empty, and the connection survives rejections.
    connection
        .send(&ClientMessage::ListDocuments { client_id: 99 })
        .await;
    let response = connection.receive().await;
    assert!(
        matches!(response, ServerMessage::DocumentList { ref documents } if documents.is_empty()),
        "forged open must not register documents, got {response:?}"
    );

    connection.close().await;
    let _ = fs::remove_dir_all(root);
}

/// Plan 060 T4 (P0-3): two connections share the server coordinators; a
/// parse update for a document opened by one connection never reaches the
/// other connection's stream.
#[tokio::test]
async fn two_client_parse_updates_are_isolated_to_the_subscribed_connection() {
    let root = temp_workspace("parse-isolation");
    fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let runtime_generation = runtime_generation();
    let parse_coordinator = parse_coordinator();
    let document_analysis =
        crate::server::document_analysis::DocumentAnalysisCoordinator::default();

    let mut connection_a = TestConnection::connect(
        99,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation.clone(),
        parse_coordinator.clone(),
        document_analysis.clone(),
        language_intelligence_coordinator(),
    )
    .await;
    let mut connection_b = TestConnection::connect(
        100,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
    )
    .await;

    // A opens and edits the document; only A may see its parse output.
    connection_a
        .send(&ClientMessage::OpenDocument {
            client_id: 99,
            workspace_root_id: root_id,
            path: "main.rs".to_string(),
        })
        .await;
    let behavior_version = loop {
        match connection_a.receive().await {
            ServerMessage::BehaviorManifest(manifest) => break manifest.behavior_version,
            ServerMessage::DocumentOpened { .. }
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::SduiSnapshot { .. } => {}
            other => panic!("unexpected message during open: {other:?}"),
        }
    };
    connection_a.drain_until_quiet().await;
    connection_a
        .send(&ClientMessage::Edit {
            document_id: 1,
            client_id: 99,
            lease_id: Some(1),
            base_version: 1,
            behavior_version,
            transaction_id: 900,
            operation: EditOperation::Insert {
                byte_offset: 13,
                text: "// owned by A\n".to_string(),
            },
        })
        .await;
    loop {
        match connection_a.receive().await {
            ServerMessage::DecorationSet(set)
                if set.document_id == 1 && set.document_version == 2 =>
            {
                break;
            }
            ServerMessage::EditAck { .. }
            | ServerMessage::DecorationSet(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_) => {}
            other => panic!("unexpected message awaiting decorations: {other:?}"),
        }
    }

    // B never subscribed to document 1: its stream stays silent.
    let leaked = timeout(
        Duration::from_millis(150),
        connection_b
            .codec
            .read_server_message(&mut connection_b.client),
    )
    .await;
    assert!(
        leaked.is_err(),
        "unsubscribed connection must receive no parse output, got {leaked:?}"
    );

    connection_a.close().await;
    connection_b.close().await;
    let _ = fs::remove_dir_all(root);
}

/// Plan 060 T4 (P0-2): save requires the editable lease and validates
/// `known_version`; status and list fail closed for documents the
/// connection never opened.
#[tokio::test]
async fn save_reload_status_list_enforce_connection_owned_access() {
    let root = temp_workspace("save-access");
    fs::write(root.join("note.md"), "hello\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let runtime_generation = runtime_generation();
    let parse_coordinator = parse_coordinator();
    let document_analysis =
        crate::server::document_analysis::DocumentAnalysisCoordinator::default();

    let mut connection_a = TestConnection::connect(
        99,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation.clone(),
        parse_coordinator.clone(),
        document_analysis.clone(),
        language_intelligence_coordinator(),
    )
    .await;
    let mut connection_b = TestConnection::connect(
        100,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
    )
    .await;

    // A opens the document (editable lease).
    connection_a
        .send(&ClientMessage::OpenDocument {
            client_id: 99,
            workspace_root_id: root_id,
            path: "note.md".to_string(),
        })
        .await;
    loop {
        match connection_a.receive().await {
            ServerMessage::DocumentOpened { .. } => break,
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::SduiSnapshot { .. } => {}
            other => panic!("unexpected message during A open: {other:?}"),
        }
    }
    connection_a.drain_until_quiet().await;

    // B has never opened document 1: resync, status, and list fail closed.
    connection_b
        .send(&ClientMessage::RequestResync {
            document_id: 1,
            client_id: 100,
            known_version: 0,
        })
        .await;
    let resync = connection_b.receive_response().await;
    assert!(
        matches!(
            resync,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ),
        "resync for an unopened document must not leak text, got {resync:?}"
    );
    connection_b
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 100,
            document_id: 1,
        })
        .await;
    let status = connection_b.receive_response().await;
    assert!(
        matches!(
            status,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ),
        "status for an unopened document must fail closed, got {status:?}"
    );
    connection_b
        .send(&ClientMessage::ListDocuments { client_id: 100 })
        .await;
    let list = connection_b.receive_response().await;
    assert!(
        matches!(list, ServerMessage::DocumentList { ref documents } if documents.is_empty()),
        "list must not leak another connection's documents, got {list:?}"
    );

    // B opens the same document: read-only access. Save fails closed.
    connection_b
        .send(&ClientMessage::OpenDocument {
            client_id: 100,
            workspace_root_id: root_id,
            path: "note.md".to_string(),
        })
        .await;
    loop {
        match connection_b.receive().await {
            ServerMessage::DocumentOpened { metadata, .. } => {
                assert_eq!(metadata.access, DocumentAccess::ReadOnly);
                break;
            }
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::SduiSnapshot { .. } => {}
            other => panic!("unexpected message during B open: {other:?}"),
        }
    }
    connection_b.drain_until_quiet().await;
    connection_b
        .send(&ClientMessage::SaveDocument {
            client_id: 100,
            document_id: 1,
            known_version: 1,
        })
        .await;
    let read_only_save = connection_b.receive_response().await;
    assert!(
        matches!(
            read_only_save,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::AccessDenied,
                ..
            }
        ),
        "read-only save must fail closed, got {read_only_save:?}"
    );

    // A saves with a future version claim: stale check fails closed.
    connection_a
        .send(&ClientMessage::SaveDocument {
            client_id: 99,
            document_id: 1,
            known_version: 99,
        })
        .await;
    let stale_save = connection_a.receive_response().await;
    assert!(
        matches!(
            stale_save,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::StaleFileMetadata,
                ..
            }
        ),
        "future-version save must fail closed, got {stale_save:?}"
    );

    // A saves at the current version: succeeds and clears dirty state.
    connection_a
        .send(&ClientMessage::SaveDocument {
            client_id: 99,
            document_id: 1,
            known_version: 1,
        })
        .await;
    let saved = connection_a.receive_response().await;
    assert!(
        matches!(
            saved,
            ServerMessage::DocumentSaved {
                document_id: 1,
                version: 1,
                dirty: false,
            }
        ),
        "lease-holder save at the current version must succeed, got {saved:?}"
    );

    connection_a.close().await;
    connection_b.close().await;
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn protocol_v27_client_is_rejected_by_v28_server() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document_state(),
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: 26,
                client_name: "v27-client".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::Error {
            code: ProtocolErrorCode::UnsupportedProtocolVersion,
            message: "unsupported protocol version".to_string(),
        }
    );
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn live_typography_update_reaches_connection_once() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let runtime_generation = runtime_generation();
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document_state(),
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation.clone(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    loop {
        if matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ) {
            break;
        }
    }

    let mut typography = crate::protocol::ActiveTypography::default();
    typography.monospace.size = 16.0;
    runtime_generation
        .replace_typography(typography)
        .await
        .unwrap();

    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::ActiveTypography(typography)
            if typography.revision == 1 && typography.monospace.size == 16.0
    ));
    assert!(
        timeout(
            Duration::from_millis(20),
            codec.read_server_message(&mut client),
        )
        .await
        .is_err(),
        "one replacement emits one live update"
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn server_sends_minimal_behavior_manifest() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::default()));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();

    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::BehaviorManifest(Box::new(BehaviorManifest::minimal_text_editing(1)))
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn server_does_not_send_default_workspace_sdui_snapshot_after_bootstrap() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "Hello from server".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        empty_sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();

    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    // Post-handshake file-open capability is always issued once.
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));
    let next = timeout(
        Duration::from_millis(25),
        codec.read_server_message(&mut client),
    )
    .await;
    assert!(next.is_err(), "unexpected default SDUI message: {next:?}");

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn client_receives_js_generated_sdui_snapshot() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        1,
        "Hello from runtime SDUI".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = sdui_state();
    {
        let runtime_tree = crate::protocol::SduiTree {
            ui_version: 1,
            root_id: crate::protocol::SduiNodeId(1),
            nodes: vec![
                crate::protocol::SduiNode::new(
                    crate::protocol::SduiNodeId(1),
                    SduiNodeKind::Flex {
                        direction: crate::protocol::SduiFlexDirection::Row,
                        children: vec![
                            crate::protocol::SduiNodeId(2),
                            crate::protocol::SduiNodeId(3),
                        ],
                    },
                ),
                crate::protocol::SduiNode::new(
                    crate::protocol::SduiNodeId(2),
                    SduiNodeKind::Panel {
                        title: "Runtime".to_string(),
                        children: Vec::new(),
                    },
                ),
                crate::protocol::SduiNode::new(
                    crate::protocol::SduiNodeId(3),
                    SduiNodeKind::EditorView {
                        binding: crate::protocol::SduiEditorBinding {
                            document_id: 1,
                            expected_version: Some(1),
                        },
                    },
                ),
            ],
        };
        sdui.lock()
            .await
            .replace_with_runtime_tree(runtime_tree)
            .unwrap();
    }
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        Arc::clone(&sdui),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();

    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::SduiSnapshot { tree, .. } => {
            assert!(tree.nodes.iter().any(|node| matches!(
                &node.kind,
                SduiNodeKind::Panel { title, .. } if title == "Runtime"
            )));
        }
        message => panic!("expected runtime SduiSnapshot, got {message:?}"),
    }

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn control_center_lists_and_activates_loaded_package_commands() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        1,
        "Hello from package commands".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = sdui_state();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();
    load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        Arc::clone(&behavior),
        workspace_state(),
        Arc::clone(&sdui),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation_from(runtime),
        coordinator,
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();

    // Handshake: a runtime with loaded packages ships extra frames, so
    // read until the file-open capability, capturing the markdown mode
    // layer manifest on the way.
    let mut markdown_manifest = None;
    loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::BehaviorManifest(manifest) => {
                if manifest.manifest_id == "markdown.markdown" {
                    markdown_manifest = Some(*manifest);
                }
            }
            ServerMessage::FileOpenCapabilityIssued { .. } => break,
            _ => {}
        }
    }
    let markdown_manifest = markdown_manifest.expect("markdown mode layer must be published");
    // The default Control Center binding survives mode activation: the
    // layer carries the Global `Ctrl+X Ctrl+O` chord (plan 124 moved it off
    // the P stroke) from the shared default commands/keymaps.
    assert!(markdown_manifest.keymaps.iter().any(|rule| {
        rule.command_id == "controlCenter.open"
            && rule.context == KeyBindingContext::Global
            && rule.sequence.len() == 2
            && rule.sequence[0].key == KeyCode::Character("x".to_string())
            && rule.sequence[0].modifiers.control
            && rule.sequence[1].key == KeyCode::Character("o".to_string())
            && rule.sequence[1].modifiers.control
    }));

    let behavior_version = behavior.lock().await.version();
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::CommandIntent {
                client_id: 99,
                document_id: 1,
                behavior_version,
                command_id: "controlCenter.open".to_string(),
            },
        )
        .await
        .unwrap();
    let snapshot = loop {
        if let ServerMessage::TransientMenuSnapshot(snapshot) =
            codec.read_server_message(&mut client).await.unwrap()
        {
            break snapshot;
        }
    };
    let session_id = snapshot.session_id;
    let toggle_preview = snapshot
        .items
        .iter()
        .find(|item| item.id == "markdown.togglePreview")
        .expect("markdown.togglePreview must be listed");
    assert!(
        snapshot
            .items
            .iter()
            .any(|item| item.id == "markdown.toggleComment"),
        "markdown.toggleComment must be listed"
    );
    // Plan 124: the effective binding is the row's `bindings` field (the
    // palette's chips); the detail line carries routing and provenance.
    assert!(
        toggle_preview
            .bindings
            .iter()
            .any(|binding| binding.contains("Ctrl+Shift+M")),
        "bindings must carry the effective chord: {:?}",
        toggle_preview.bindings
    );
    let detail = toggle_preview.detail.as_deref().unwrap_or_default();
    assert!(
        detail.contains("@clay/markdown@0.1.0"),
        "detail must carry package provenance: {detail}"
    );

    // Query narrows to exactly the preview command.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::MenuQueryUpdate {
                client_id: 99,
                session_id,
                query: "togglePreview".to_string(),
                scope: None,
            },
        )
        .await
        .unwrap();
    let filtered = loop {
        if let ServerMessage::TransientMenuSnapshot(snapshot) =
            codec.read_server_message(&mut client).await.unwrap()
        {
            break snapshot;
        }
    };
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(filtered.items[0].id, "markdown.togglePreview");

    // Activation closes the menu and validates through the live
    // aggregated registry; the JS side effect runs in the package
    // runtime, so no wire frame follows the close.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::MenuActivate {
                client_id: 99,
                session_id,
                kind: crate::protocol::TransientMenuActivationData::Primary,
            },
        )
        .await
        .unwrap();
    loop {
        if let message @ ServerMessage::TransientMenuClosed { .. } =
            codec.read_server_message(&mut client).await.unwrap()
        {
            assert_eq!(message, ServerMessage::TransientMenuClosed { session_id });
            break;
        }
    }
    assert!(
        timeout(
            Duration::from_millis(25),
            codec.read_server_message(&mut client)
        )
        .await
        .is_err(),
        "no frame after validated package activation"
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn control_center_opens_even_when_the_client_version_lags_the_manifest() {
    // Regression (plan 117 follow-up): the mode-layer publish bumps the
    // behavior version after the client bootstrapped; a lagging client's
    // `Ctrl+X Ctrl+O` intent used to die on the stale-version gate with a
    // silent wire error — the Command Centre never opened. Server-owned
    // catalogue commands re-resolve everything at open time, so they skip
    // the gate; manifest-coupled commands keep it.
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        1,
        "stale version chord".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = sdui_state();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();
    load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        Arc::clone(&behavior),
        workspace_state(),
        Arc::clone(&sdui),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation_from(runtime),
        coordinator,
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let mut stale_version = 0;
    loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::BehaviorManifest(manifest) => {
                stale_version = manifest.behavior_version.saturating_sub(1);
            }
            ServerMessage::FileOpenCapabilityIssued { .. } => break,
            _ => {}
        }
    }
    assert!(stale_version >= 1, "mode layer publish bumped the version");

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::CommandIntent {
                client_id: 99,
                document_id: 1,
                behavior_version: stale_version,
                command_id: "controlCenter.open".to_string(),
            },
        )
        .await
        .unwrap();
    let opened = loop {
        if let ServerMessage::TransientMenuSnapshot(snapshot) =
            codec.read_server_message(&mut client).await.unwrap()
        {
            break snapshot;
        }
    };
    assert!(
        !opened.items.is_empty(),
        "the control centre opens despite the lagging client version"
    );

    // The gate survives for manifest-coupled commands: a stale version on a
    // generic server command is still rejected instead of executing.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::CommandIntent {
                client_id: 99,
                document_id: 1,
                behavior_version: stale_version,
                command_id: "workspace.refresh".to_string(),
            },
        )
        .await
        .unwrap();
    let rejected = loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::Error { message, .. } => break message,
            ServerMessage::TransientMenuClosed { .. } | ServerMessage::TransientMenuSnapshot(_) => {
                continue;
            }
            _ => continue,
        }
    };
    assert_eq!(rejected, "command intent behavior version is stale");

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn server_sends_runtime_diagnostics_after_bootstrap() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::default()));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let diagnostics = Arc::new(Mutex::new(RuntimeDiagnosticStore::default()));
    diagnostics.lock().await.push(RuntimeDiagnostic::error(
        "runtime.invalid_import",
        "Only clay:* facades and relative local configuration modules are allowed.",
    ));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        Arc::clone(&diagnostics),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();

    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic::error(
            "runtime.invalid_import",
            "Only clay:* facades and relative local configuration modules are allowed.",
        ))
    );
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    let live_diagnostic =
        RuntimeDiagnostic::warning("runtime.live_update", "configuration reload failed");
    diagnostics.lock().await.publish(live_diagnostic.clone());
    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::RuntimeDiagnostic(live_diagnostic)
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn server_acknowledges_insert_edit() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "Hi".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Edit {
                document_id: 7,
                client_id: 99,
                lease_id: Some(1),
                base_version: 1,
                behavior_version: 1,
                transaction_id: 123,
                operation: EditOperation::Insert {
                    byte_offset: 2,
                    text: " Clay".to_string(),
                },
            },
        )
        .await
        .unwrap();

    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::EditAck {
            document_id: 7,
            confirmed_version: 2,
            transaction_id: 123,
        }
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn server_rejects_edit_with_stale_behavior_version_without_mutating_document() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "Hi".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        Arc::clone(&document),
        behavior,
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Edit {
                document_id: 7,
                client_id: 99,
                lease_id: Some(1),
                base_version: 1,
                behavior_version: 0,
                transaction_id: 123,
                operation: EditOperation::Insert {
                    byte_offset: 2,
                    text: " stale".to_string(),
                },
            },
        )
        .await
        .unwrap();

    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::EditRejected {
            document_id: 7,
            transaction_id: 123,
            reason: EditRejection::InvalidBehaviorVersion {
                behavior_version: 0,
                server_behavior_version: 1,
            },
        }
    );

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Edit {
                document_id: 7,
                client_id: 99,
                lease_id: Some(1),
                base_version: 1,
                behavior_version: 1,
                transaction_id: 124,
                operation: EditOperation::Insert {
                    byte_offset: 2,
                    text: " ok".to_string(),
                },
            },
        )
        .await
        .unwrap();

    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::EditAck {
            document_id: 7,
            confirmed_version: 2,
            transaction_id: 124,
        }
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn server_sends_resync_snapshot_after_request() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "server 🦀".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::RequestResync {
                document_id: 7,
                client_id: 99,
                known_version: 0,
            },
        )
        .await
        .unwrap();

    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::ResyncSnapshot {
            document_id: 7,
            version: 1,
            head: crate::protocol::DocumentTextHead::complete("server 🦀".to_string()),
            access: DocumentAccess::Editable { lease_id: 1 },
            lease_id: Some(1),
        }
    );

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn stale_chunk_request_after_edit_rejects_then_resync_completes() {
    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "server 🦀".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Edit {
                document_id: 7,
                client_id: 99,
                lease_id: Some(1),
                base_version: 1,
                behavior_version: 1,
                transaction_id: 124,
                operation: EditOperation::Insert {
                    byte_offset: 2,
                    text: " ok".to_string(),
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::EditAck {
            document_id: 7,
            confirmed_version: 2,
            transaction_id: 124,
        }
    );

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::DocumentChunkRequest {
                client_id: 99,
                document_id: 7,
                document_version: 1,
                offset: 0,
                max_bytes: 16,
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::DocumentChunkRejected {
            document_id: 7,
            document_version: 1,
            offset: 0,
            reason: crate::protocol::DocumentChunkRejection::StaleVersion { current_version: 2 },
        }
    ));

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::RequestResync {
                document_id: 7,
                client_id: 99,
                known_version: 1,
            },
        )
        .await
        .unwrap();
    let ServerMessage::ResyncSnapshot {
        document_id,
        version,
        head,
        ..
    } = codec.read_server_message(&mut client).await.unwrap()
    else {
        panic!("expected resync snapshot");
    };
    assert_eq!(document_id, 7);
    assert_eq!(version, 2);
    assert_eq!(head.first_chunk, "se okrver 🦀");
    assert_eq!(head.total_bytes, head.first_chunk.len() as u64);

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn connection_open_document_sends_snapshot_and_manifest_without_full_document_on_edit_ack() {
    let root = temp_workspace("open-dispatch");
    let file = root.join("main.rs");
    fs::write(&file, "fn main() {}\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        Arc::clone(&workspace),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "main.rs".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::DocumentOpened {
            metadata: DocumentMetadata {
                document_id: 1,
                version: 1,
                access: DocumentAccess::Editable { lease_id: 1 },
                lease_id: Some(1),
                dirty: false,
                workspace_root_id: root_id,
                path: "main.rs".to_string(),
            },
            head: crate::protocol::DocumentTextHead::complete("fn main() {}\n".to_string()),
        }
    );
    let behavior_version = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::BehaviorManifest(manifest) => {
            assert_eq!(manifest.manifest_id, "rust.rust");
            assert_eq!(
                manifest.scope,
                crate::protocol::BehaviorScope::Document { document_id: 1 }
            );
            assert_eq!(manifest.editor_rules.tab.spaces_per_tab, 4);
            assert_eq!(
                manifest
                    .editor_rules
                    .autocomplete_triggers
                    .iter()
                    .map(|trigger| trigger.trigger.as_str())
                    .collect::<Vec<_>>(),
                vec![".", ":"]
            );
            manifest.behavior_version
        }
        other => panic!("expected Rust behavior manifest after open, got {other:?}"),
    };

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Edit {
                document_id: 1,
                client_id: 99,
                lease_id: Some(1),
                base_version: 1,
                behavior_version,
                transaction_id: 444,
                operation: EditOperation::Insert {
                    byte_offset: 13,
                    text: "// ok\n".to_string(),
                },
            },
        )
        .await
        .unwrap();

    loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::EditAck {
                document_id: 1,
                confirmed_version: 2,
                transaction_id: 444,
            } => break,
            ServerMessage::DecorationSet(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::BehaviorManifest(_) => {}
            other => panic!("expected edit acknowledgement, got {other:?}"),
        }
    }

    loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::DecorationSet(set)
                if set.document_id == 1 && set.document_version == 2 =>
            {
                assert!(!set.spans.is_empty());
                break;
            }
            ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::BehaviorManifest(_) => {}
            other => panic!("expected refreshed syntax decorations, got {other:?}"),
        }
    }

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::GetDocumentStatus {
                client_id: 99,
                document_id: 1,
            },
        )
        .await
        .unwrap();
    loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::DocumentStatus {
                metadata:
                    DocumentMetadata {
                        document_id: 1,
                        version: 2,
                        access: DocumentAccess::Editable { lease_id: 1 },
                        lease_id: Some(1),
                        dirty: true,
                        workspace_root_id: status_root_id,
                        path,
                    },
            } => {
                assert_eq!(status_root_id, root_id);
                assert_eq!(path, "main.rs");
                break;
            }
            ServerMessage::DecorationSet(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::BehaviorManifest(_) => {}
            other => panic!("expected document status, got {other:?}"),
        }
    }

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn viewport_render_requests_answer_one_patch_per_request_id() {
    let root = temp_workspace("viewport-patch-protocol");
    let file = root.join("main.rs");
    fs::write(&file, "fn main() {} // tail\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        Arc::clone(&workspace),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    for _ in 0..9 {
        let _ = codec.read_server_message(&mut client).await.unwrap();
    }
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "main.rs".to_string(),
            },
        )
        .await
        .unwrap();
    let opened_version = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::DocumentOpened { metadata, .. } => metadata.version,
        message => panic!("expected DocumentOpened, got {message:?}"),
    };
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    // Trailing connection-wide manifest follows the document's mode layer.
    let _global_manifest = codec.read_server_message(&mut client).await.unwrap();
    // Drain the open-driven parse output (edit-driven frames) so the
    // later request-scoped assertions see a quiet connection.
    loop {
        let message = timeout(
            Duration::from_secs(2),
            codec.read_server_message(&mut client),
        )
        .await
        .expect("open parse output timed out")
        .unwrap();
        if matches!(
            message,
            ServerMessage::DecorationSet(_)
                | ServerMessage::DecorationBatch(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
        ) {
            break;
        }
    }

    // Stale version: one rejection patch, nothing else scheduled.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::ViewportRenderRequest {
                client_id: 99,
                document_id: 1,
                document_version: opened_version + 5,
                request_id: 1,
                byte_start: 0,
                byte_end: 16,
                trace_id: None,
            },
        )
        .await
        .unwrap();
    let stale_patch = loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::ViewportRenderPatch(patch) => break patch,
            ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::BehaviorManifest(_) => {}
            message => panic!("expected stale-version rejection patch, got {message:?}"),
        }
    };
    assert_eq!(stale_patch.request_id, 1);
    assert_eq!(stale_patch.status, ViewportRenderStatus::Rejected);
    assert_eq!(stale_patch.reason.as_deref(), Some("staleVersion"));
    assert!(stale_patch.decorations.is_empty());

    // Invalid range: rejected before any allocation.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::ViewportRenderRequest {
                client_id: 99,
                document_id: 1,
                document_version: opened_version,
                request_id: 2,
                byte_start: 32,
                byte_end: 16,
                trace_id: None,
            },
        )
        .await
        .unwrap();
    let range_patch = loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::ViewportRenderPatch(patch) => break patch,
            ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::BehaviorManifest(_) => {}
            message => panic!("expected invalid-range rejection patch, got {message:?}"),
        }
    };
    assert_eq!(range_patch.request_id, 2);
    assert_eq!(range_patch.status, ViewportRenderStatus::Rejected);
    assert_eq!(range_patch.reason.as_deref(), Some("invalidRange"));

    // Valid request (clamped past the document end): exactly one complete
    // patch aggregates every scheduled window member, in viewport-key
    // order, with no per-member frames after the request.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::ViewportRenderRequest {
                client_id: 99,
                document_id: 1,
                document_version: opened_version,
                request_id: 3,
                byte_start: 0,
                byte_end: 1 << 20,
                trace_id: None,
            },
        )
        .await
        .unwrap();
    let mut member_frames = 0usize;
    let patch = loop {
        let message = timeout(
            Duration::from_secs(2),
            codec.read_server_message(&mut client),
        )
        .await
        .expect("viewport patch timed out")
        .unwrap();
        match message {
            ServerMessage::ViewportRenderPatch(patch) if patch.request_id == 3 => break patch,
            ServerMessage::ViewportRenderPatch(_) => {
                panic!("no second patch may answer one request id")
            }
            ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_) => member_frames += 1,
            _ => {}
        }
    };
    assert_eq!(patch.status, ViewportRenderStatus::Complete);
    assert!(patch.reason.is_none());
    assert!(!patch.decorations.is_empty());
    assert!(
        patch
            .decorations
            .windows(2)
            .all(|pair| pair[0].viewport_byte_start <= pair[1].viewport_byte_start),
        "patch members arrive in viewport-key order"
    );
    assert!(
        patch
            .covered_ranges
            .iter()
            .all(|range| range.byte_end <= 21),
        "covered ranges stay clamped to the document, got {:?}",
        patch.covered_ranges
    );
    assert_eq!(
        member_frames, 0,
        "viewport replies must not fan out per-member frames"
    );

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn file_browser_open_uses_generic_open_document_followups() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = temp_workspace("file-browser-open-followups");
    let selected = root.join("note.md");
    fs::write(&selected, "# Browser note\n\n- item\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    workspace_state_value.reserve_document_ids_from(2);
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));

    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = sdui_state();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();

    let (client, server) = duplex(16 * 1024 * 1024);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        Arc::clone(&sdui),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation_from(runtime),
        coordinator,
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let tree = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::SduiSnapshot { tree, .. } => tree,
        message => panic!("expected file browser SduiSnapshot, got {message:?}"),
    };
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();
    let action = tree
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            SduiNodeKind::List { items, .. } => items
                .iter()
                .find(|item| item.label == "note.md")
                .and_then(|item| item.action.clone()),
            _ => None,
        })
        .expect("note.md file-browser action");

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::SduiAction {
                client_id: 99,
                ui_version: tree.ui_version,
                intent: action,
            },
        )
        .await
        .unwrap();

    let (opened_version, opened_lease_id) =
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::DocumentOpened { metadata, head } => {
                assert_eq!(metadata.document_id, 2);
                assert_eq!(metadata.workspace_root_id, root_id);
                assert_eq!(metadata.path, "note.md");
                assert_eq!(head.first_chunk, "# Browser note\n\n- item\n");
                let DocumentAccess::Editable { lease_id } = metadata.access else {
                    panic!("file-browser opener must receive editable access");
                };
                (metadata.version, lease_id)
            }
            message => panic!("expected file-browser DocumentOpened, got {message:?}"),
        };
    let behavior_version = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::BehaviorManifest(manifest) => {
            assert_eq!(manifest.manifest_id, "markdown.markdown");
            assert!(matches!(
                manifest.scope,
                BehaviorScope::Document { document_id: 2 }
            ));
            manifest.behavior_version
        }
        message => panic!("expected Markdown BehaviorManifest, got {message:?}"),
    };

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Edit {
                document_id: 2,
                client_id: 99,
                lease_id: Some(opened_lease_id),
                base_version: opened_version,
                behavior_version,
                transaction_id: 7,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "!".to_string(),
                },
            },
        )
        .await
        .unwrap();
    loop {
        match timeout(
            Duration::from_secs(1),
            codec.read_server_message(&mut client),
        )
        .await
        .expect("opened-file edit acknowledgement timed out")
        .unwrap()
        {
            ServerMessage::EditAck {
                document_id: 2,
                confirmed_version,
                transaction_id: 7,
            } => {
                assert_eq!(confirmed_version, opened_version + 1);
                break;
            }
            ServerMessage::DecorationSet(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::BehaviorManifest(_) => {}
            message => panic!("expected opened-file EditAck, got {message:?}"),
        }
    }

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(selected);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn multi_chunk_parse_update_ships_as_single_decoration_batch() {
    let root = temp_workspace("decoration-batch");
    let file = root.join("main.rs");
    // Well past one 128-byte authority chunk.
    let source = "fn main() { let value = 1; }\n".repeat(16);
    fs::write(&file, &source).unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));

    let (client, server) = duplex(64 * 1024);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        Arc::clone(&workspace),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    for _ in 0..8 {
        let _ = codec.read_server_message(&mut client).await.unwrap();
    }
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "main.rs".to_string(),
            },
        )
        .await
        .unwrap();
    // DocumentOpened, BehaviorManifest, replenished capability.
    let _opened = codec.read_server_message(&mut client).await.unwrap();
    let behavior_version = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::BehaviorManifest(manifest) => manifest.behavior_version,
        message => panic!("expected behavior manifest, got {message:?}"),
    };
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    // Register/cache the native handler with one edit, then request the
    // whole visible region so this test isolates multi-chunk wire batching
    // from the edit's expected one-chunk incremental update.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Edit {
                document_id: 1,
                client_id: 99,
                lease_id: Some(1),
                base_version: 1,
                behavior_version,
                transaction_id: 555,
                operation: EditOperation::Insert {
                    byte_offset: 0,
                    text: "// batch\n".to_string(),
                },
            },
        )
        .await
        .unwrap();

    let mut confirmed_version = None;
    let mut edit_update_seen = false;
    let mut viewport_requested = false;
    let mut viewport_patches = 0usize;
    let mut member_frames_after_request = 0usize;
    let batch = loop {
        let message = timeout(
            Duration::from_secs(2),
            codec.read_server_message(&mut client),
        )
        .await
        .expect("viewport patch timed out")
        .unwrap();
        match message {
            ServerMessage::ViewportRenderPatch(patch)
                if viewport_requested && patch.request_id == 1 =>
            {
                assert_eq!(patch.document_id, 1);
                assert_eq!(patch.document_version, 2);
                viewport_patches += 1;
                break patch.decorations;
            }
            ServerMessage::DecorationBatch(chunks)
                if !viewport_requested && chunks[0].document_version == 2 =>
            {
                edit_update_seen = true;
            }
            ServerMessage::EditAck {
                confirmed_version: version,
                ..
            } => confirmed_version = Some(version),
            ServerMessage::DecorationSet(set)
                if set.document_id == 1 && set.document_version == 2 =>
            {
                if viewport_requested {
                    member_frames_after_request += 1;
                } else {
                    edit_update_seen = true;
                }
            }
            ServerMessage::DecorationSet(_)
            | ServerMessage::DecorationBatch(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_) => {}
            message => panic!("expected viewport render patch, got {message:?}"),
        }
        if !viewport_requested
            && edit_update_seen
            && let Some(document_version) = confirmed_version
        {
            codec
                .write_client_message(
                    &mut client,
                    &ClientMessage::ViewportRenderRequest {
                        client_id: 99,
                        document_id: 1,
                        document_version,
                        request_id: 1,
                        byte_start: 0,
                        byte_end: (source.len() + "// batch\n".len()) as u64,
                        trace_id: None,
                    },
                )
                .await
                .unwrap();
            viewport_requested = true;
        }
    };

    assert!(
        batch.len() > 1,
        "multi-chunk window must arrive as one patch with ordered members, got {} members",
        batch.len()
    );
    assert!(batch.iter().all(|set| set.document_id == 1));
    assert!(
        batch
            .windows(2)
            .all(|pair| pair[0].viewport_byte_start <= pair[1].viewport_byte_start),
        "patch members arrive in viewport-key order"
    );
    assert!(batch.iter().all(|set| !set.spans.is_empty()));
    assert_eq!(viewport_patches, 1, "exactly one patch per request id");
    assert_eq!(
        member_frames_after_request, 0,
        "viewport replies must not fan out per-chunk frames after the request"
    );

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(file);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn selected_markdown_file_publishes_manifest_and_decorations() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = temp_workspace("selected-markdown-runtime");
    let selected = root.join("note.md");
    fs::write(
        &selected,
        "# Opened note\n\n- item with `code`\n\n**strong** and *emphasis*\n",
    )
    .unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    workspace_state_value.reserve_document_ids_from(2);
    let workspace = Arc::new(Mutex::new(workspace_state_value));

    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = sdui_state();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();

    let (client, server) = duplex(16 * 1024 * 1024);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        Arc::clone(&sdui),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation_from(runtime),
        coordinator,
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let capability_token = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::FileOpenCapabilityIssued { token } => token,
        message => panic!("expected FileOpenCapabilityIssued, got {message:?}"),
    };

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenSelectedFile {
                client_id: 99,
                capability: capability_token,
                selected_path: selected.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();

    match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::DocumentOpened { metadata, head } => {
            assert_eq!(metadata.document_id, 2);
            assert_eq!(metadata.path, "note.md");
            assert_eq!(
                head.first_chunk,
                "# Opened note\n\n- item with `code`\n\n**strong** and *emphasis*\n"
            );
        }
        message => panic!("expected selected Markdown DocumentOpened, got {message:?}"),
    }
    match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::BehaviorManifest(manifest) => {
            assert_eq!(manifest.manifest_id, "markdown.markdown");
            assert!(matches!(
                manifest.scope,
                BehaviorScope::Document { document_id: 2 }
            ));
            assert!(
                manifest
                    .commands
                    .iter()
                    .any(|command| { command.command_id == "markdown.togglePreview" })
            );
        }
        message => panic!("expected Markdown BehaviorManifest, got {message:?}"),
    }
    // Server re-issues one pending capability after the open attempt; parse
    // decorations are scheduled in the background instead of blocking open.
    // Phase 22.2: the follow-up also carries the connection-wide manifest
    // after the document's mode layer; consume it before the capability.
    match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::BehaviorManifest(_) => {}
        message => panic!("expected trailing global manifest, got {message:?}"),
    }
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));

    // Selected-file activation publishes behavior only on the open path;
    // optional package UI panels stay opt-in, and highlights arrive later
    // through the parse coordinator rather than before the replenished
    // capability.

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(selected);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn default_init_js_load_package_powers_selected_markdown_open() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let config_root = temp_workspace("default-init-loadpackage");
    fs::write(
        config_root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("@clay/markdown");
        "#,
    )
    .unwrap();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = empty_sdui_state();
    let evaluation = runtime
        .load_configuration_from_root(config_root.clone())
        .await
        .expect("default init.js loadPackage should evaluate");
    runtime
        .register_parse_handlers(&coordinator, 1, &evaluation)
        .expect("init.js loadPackage should register parse handler");
    super::super::apply_runtime_outputs(&evaluation, 1, &behavior, &sdui).await;
    assert_eq!(runtime.evaluation_count(), 1);

    let metadata = DocumentMetadata {
        document_id: 2,
        version: 1,
        access: DocumentAccess::Editable { lease_id: 1 },
        lease_id: Some(1),
        dirty: false,
        workspace_root_id: 1,
        path: "note.md".to_string(),
    };
    let document = Arc::new(Mutex::new(DocumentState::new(
        2,
        "# Loaded from init.js\n".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let messages = super::open_document_followup_messages(
        &metadata,
        &document,
        &behavior,
        &sdui,
        1,
        &runtime,
        &coordinator,
    )
    .await;

    assert_eq!(
        runtime.evaluation_count(),
        2,
        "open should classify/activate on the persistent runtime without a fresh per-open runtime"
    );
    assert!(messages.iter().any(|message| matches!(
        message,
        ServerMessage::BehaviorManifest(manifest)
            if manifest.manifest_id == "markdown.markdown"
                && matches!(manifest.scope, BehaviorScope::Document { document_id: 2 })
    )));
    assert!(messages.iter().all(|message| {
        !matches!(message, ServerMessage::DecorationSet(set) if set.document_id == 2)
    }));
    let update = timeout(Duration::from_secs(1), coordinator.next_update())
        .await
        .unwrap()
        .unwrap();
    let set = update
        .decoration_updates
        .into_iter()
        .next()
        .expect("background markdown decorations");
    assert_eq!(set.document_id, 2);
    assert!(
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Heading1)
    );
    assert!(
        set.spans
            .iter()
            .all(|span| span.provenance.package_version == "builtin"),
        "open Markdown decorations must come from compiled Tier 1 grammar, not parser.js"
    );
    let _ = fs::remove_file(config_root.join("init.js"));
    let _ = fs::remove_dir(config_root);
}

#[tokio::test]
async fn native_windows_schedule_once_for_each_first_party_language() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let config_root = temp_workspace("viewport-native-decoration");
    fs::write(
        config_root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("@clay/rust");
        await loadPackage("@clay/typescript");
        await loadPackage("@clay/javascript");
        await loadPackage("@clay/markdown");
        "#,
    )
    .unwrap();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();
    let evaluation = runtime
        .load_configuration_from_root(config_root.clone())
        .await
        .expect("language configuration evaluates");

    for (document_id, path, package_prefix, start_marker, text) in [
        (
            19,
            "main.rs",
            "rust",
            "fn value150",
            (0..300)
                .map(|line| format!("fn value{line}() -> usize {{ {line} }}\n"))
                .collect::<String>(),
        ),
        (
            20,
            "main.ts",
            "typescript",
            "const value150",
            (0..300)
                .map(|line| format!("const value{line}: number = {line};\n"))
                .collect::<String>(),
        ),
        (
            21,
            "main.js",
            "javascript",
            "const value150",
            (0..300)
                .map(|line| format!("const value{line} = {line};\n"))
                .collect::<String>(),
        ),
        (
            22,
            "notes.md",
            "markdown",
            "LAST CODE LINE",
            format!(
                "```text\n{}LAST CODE LINE\n```\n\nPlain prose after fence.\n",
                "code inside fence\n".repeat(300)
            ),
        ),
    ] {
        let metadata = DocumentMetadata {
            document_id,
            version: 1,
            access: DocumentAccess::Editable { lease_id: 1 },
            lease_id: Some(1),
            dirty: false,
            workspace_root_id: 1,
            path: path.to_string(),
        };
        let (meta, policy) = runtime
            .register_native_syntax_handler(
                &coordinator,
                1,
                &evaluation,
                path,
                package_prefix,
                package_prefix,
            )
            .expect("native handler registration succeeds")
            .expect("native handler selected");
        assert_eq!(
            runtime.registered_native_syntax_handler(1, path),
            Some((meta.clone(), policy))
        );
        super::schedule_parse_window(
            &coordinator,
            &metadata,
            &text,
            1,
            &meta.package_prefix,
            &meta.mode_id,
            policy,
            ParseByteRange::new(0, text.len() as u64),
        )
        .expect("opening viewport schedules");
        let opening_end = text
            .len()
            .min(policy.max_window_bytes as usize)
            .min(crate::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES)
            as u64;
        let update = tokio::select! {
            update = coordinator.next_update() => update.expect("opening native update"),
            diagnostic = coordinator.next_diagnostic() => {
                panic!("opening viewport parse failed: {:?}", diagnostic)
            }
        };
        assert_eq!(
            (update.viewport.start, update.viewport.end),
            (0, opening_end),
            "{path}"
        );
        assert!(!update.decoration_updates.is_empty(), "{path}");
        assert!(
            update
                .decoration_updates
                .iter()
                .any(|set| !set.spans.is_empty()),
            "{path}"
        );

        let start = text.find(start_marker).expect("middle line marker") as u64;
        super::schedule_parse_window(
            &coordinator,
            &metadata,
            &text,
            1,
            &meta.package_prefix,
            &meta.mode_id,
            policy,
            ParseByteRange::new(start, text.len() as u64),
        )
        .expect("nonzero viewport schedules");
        let update = tokio::select! {
            update = coordinator.next_update() => update.expect("nonzero native update"),
            diagnostic = coordinator.next_diagnostic() => {
                panic!("nonzero viewport parse failed: {:?}", diagnostic)
            }
        };
        let requested_end = (start
            + crate::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES as u64)
            .min(start + policy.max_window_bytes)
            .min(text.len() as u64);
        assert!(update.viewport.start <= start, "{path}");
        assert!(update.viewport.end >= requested_end, "{path}");
        assert!(!update.decoration_updates.is_empty(), "{path}");
        assert!(
            update.decoration_updates.iter().all(|set| set
                .spans
                .iter()
                .all(|span| span.byte_start >= set.viewport_byte_start)),
            "{path}"
        );
        if path == "notes.md" {
            let prose = text.find("Plain prose after fence.").unwrap() as u64;
            assert!(
                update
                    .decoration_updates
                    .iter()
                    .any(|set| set
                        .spans
                        .iter()
                        .any(|span| span.token_type == TokenType::Paragraph
                            && span.byte_start <= prose
                            && span.byte_end > prose))
            );
            assert!(
                !update
                    .decoration_updates
                    .iter()
                    .any(|set| set
                        .spans
                        .iter()
                        .any(|span| span.token_type == TokenType::CodeBlock
                            && span.byte_start <= prose
                            && span.byte_end > prose))
            );
        }

        // Returning to the head after a distant viewport must reparse and
        // republish the head; one cached window may never make an older
        // viewport permanently blank.
        super::schedule_parse_window(
            &coordinator,
            &metadata,
            &text,
            1,
            &meta.package_prefix,
            &meta.mode_id,
            policy,
            ParseByteRange::new(0, opening_end),
        )
        .expect("return-to-head viewport schedules");
        let returned = tokio::select! {
            update = coordinator.next_update() => update.expect("return-to-head native update"),
            diagnostic = coordinator.next_diagnostic() => {
                panic!("return-to-head viewport parse failed: {:?}", diagnostic)
            }
        };
        assert_eq!(returned.viewport.start, 0, "{path}");
        assert!(
            returned
                .decoration_updates
                .iter()
                .any(|set| !set.spans.is_empty()),
            "returning to the head must restore syntax for {path}"
        );
    }

    let _ = fs::remove_file(config_root.join("init.js"));
    let _ = fs::remove_dir(config_root);
}

#[test]
fn connection_has_no_markdown_specific_open_runtime_branch() {
    let source = include_str!("mod.rs");
    for (left, right) in [
        ("evaluate_", "markdown_open"),
        ("create_", "markdown_open_runtime_root"),
        ("unique_", "markdown_open_runtime_root"),
        ("markdown_", "open_init_source"),
        ("is_", "markdown_path"),
    ] {
        let removed = format!("{left}{right}");
        assert!(
            !source.contains(&removed),
            "connection.rs must not contain removed mode-specific helper `{removed}`"
        );
    }
}

#[tokio::test]
async fn classify_large_markdown_document_uses_markdown_mode() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let text = format!("{}\n", "word ".repeat(1024 * 1024 / 5));
    let metadata = DocumentMetadata {
        document_id: 9,
        version: 1,
        access: DocumentAccess::Editable { lease_id: 1 },
        lease_id: Some(1),
        dirty: false,
        workspace_root_id: 1,
        path: "big.md".to_string(),
    };
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = empty_sdui_state();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();
    load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;
    let activation = super::classify_open_document(
        1,
        &runtime,
        &coordinator,
        &metadata,
        &text,
        &behavior,
        &sdui,
    )
    .await
    .expect("large markdown open classifies");
    assert_eq!(activation.mode_id, "markdown");
}

/// Plan 099: a repeat open whose classification inputs and native grammar
/// registration match a cached activation republishes the cached manifest
/// from Rust instead of evaluating the generated classification module in
/// V8. Also measures the generated V8 open activation for the record.
#[tokio::test]
async fn mode_activation_cache_hit_skips_generated_module_evaluation() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let text = "# Title\n\nSome prose.\n";
    let make_metadata = |document_id| DocumentMetadata {
        document_id,
        version: 1,
        access: DocumentAccess::Editable { lease_id: 1 },
        lease_id: Some(1),
        dirty: false,
        workspace_root_id: 1,
        path: "notes.md".to_string(),
    };
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = empty_sdui_state();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();
    load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;

    let started = std::time::Instant::now();
    let first = super::classify_open_document(
        1,
        &runtime,
        &coordinator,
        &make_metadata(2),
        text,
        &behavior,
        &sdui,
    )
    .await
    .expect("first open classifies through the generated module");
    let v8_elapsed = started.elapsed();
    let evaluations_after_first = runtime.open_activation_evaluation_count();
    assert_eq!(evaluations_after_first, 1);
    let manifest_after_first = behavior.lock().await.manifest().clone();

    let second = super::classify_open_document(
        1,
        &runtime,
        &coordinator,
        &make_metadata(3),
        text,
        &behavior,
        &sdui,
    )
    .await
    .expect("repeat open classifies through the registry fast path");

    assert_eq!(second.package_prefix, first.package_prefix);
    assert_eq!(second.mode_id, first.mode_id);
    assert_eq!(second.parse_handler_mode_id, first.parse_handler_mode_id);
    assert_eq!(second.native_parse_policy, first.native_parse_policy);
    // publish_replacement bumps behavior_version; content must be identical.
    let republished = behavior.lock().await.manifest().clone();
    let mut expected = manifest_after_first;
    expected.behavior_version = republished.behavior_version;
    assert_eq!(
        republished, expected,
        "fast path republishes the identical behavior manifest"
    );
    assert_eq!(
        runtime.open_activation_evaluation_count(),
        evaluations_after_first,
        "repeat open must not evaluate a generated module in V8"
    );
    eprintln!(
        "Plan 099 measurement: generated V8 open activation took {v8_elapsed:?};              registry fast path reuses the cached manifest without V8"
    );
}

#[tokio::test]
async fn mode_activation_cache_evicts_oldest_not_all() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let capacity = crate::perf::budgets::MODE_ACTIVATION_CACHE_ENTRIES;
    let make_metadata = |document_id| DocumentMetadata {
        document_id,
        version: 1,
        access: DocumentAccess::Editable { lease_id: 1 },
        lease_id: Some(1),
        dirty: false,
        workspace_root_id: 1,
        path: "notes.md".to_string(),
    };
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = empty_sdui_state();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();
    load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;

    // Each distinct leading content is a distinct activation key, so one open
    // per key fills the cache exactly to capacity and then overflows by one.
    let text_for = |index: usize| format!("# Title\n\nprobe-{index}\n");
    for index in 0..=capacity {
        super::classify_open_document(
            capacity as u64 + 1,
            &runtime,
            &coordinator,
            &make_metadata(2),
            &text_for(index),
            &behavior,
            &sdui,
        )
        .await
        .unwrap_or_else(|| panic!("open {index} classifies through the generated module"));
    }
    let evaluations = runtime.open_activation_evaluation_count();
    assert_eq!(evaluations, capacity as u64 + 1);

    // The second-oldest key survived the overflow: a repeat open still hits the
    // cache, which the previous clear-all behaviour could not do.
    super::classify_open_document(
        capacity as u64 + 1,
        &runtime,
        &coordinator,
        &make_metadata(3),
        &text_for(1),
        &behavior,
        &sdui,
    )
    .await
    .expect("second-oldest key still classifies through the registry fast path");
    assert_eq!(
        runtime.open_activation_evaluation_count(),
        evaluations,
        "second-oldest key must not re-evaluate its generated module"
    );

    // The oldest key was evicted instead of the whole cache.
    super::classify_open_document(
        capacity as u64 + 1,
        &runtime,
        &coordinator,
        &make_metadata(4),
        &text_for(0),
        &behavior,
        &sdui,
    )
    .await
    .expect("oldest key classifies through the generated module again");
    assert_eq!(
        runtime.open_activation_evaluation_count(),
        evaluations + 1,
        "oldest key must re-evaluate its generated module"
    );
}

#[tokio::test]
async fn open_document_renders_before_background_parse_completes() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let mut text = "# Top\n\n".to_string();
    text.push_str(&"a".repeat(80 * 1024));
    text.push_str("\n# Outside initial window\n");
    let metadata = DocumentMetadata {
        document_id: 2,
        version: 1,
        access: DocumentAccess::Editable { lease_id: 1 },
        lease_id: Some(1),
        dirty: false,
        workspace_root_id: 1,
        path: "large.md".to_string(),
    };
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let sdui = empty_sdui_state();
    let runtime = js_runtime();
    let coordinator = parse_coordinator();
    load_markdown_runtime(&runtime, &coordinator, &behavior, &sdui).await;
    let activation = super::classify_open_document(
        1,
        &runtime,
        &coordinator,
        &metadata,
        &text,
        &behavior,
        &sdui,
    )
    .await
    .expect("loaded package should classify markdown path");

    let document = DocumentState::new(2, text, DocumentAccess::Editable { lease_id: 1 });
    let immediate =
        super::schedule_open_parse(&coordinator, &metadata, &document, &behavior, &activation)
            .await
            .expect("open parse should schedule");
    assert!(
        immediate.is_none(),
        "open follow-up must not wait for parse output"
    );

    let native_window = crate::perf::budgets::INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES;
    let update = timeout(Duration::from_secs(1), coordinator.next_update())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(update.document_id, 2);
    assert_eq!(
        (update.viewport.start, update.viewport.end),
        (0, native_window as u64)
    );
    assert!(update.decoration_updates.iter().any(|set| {
        set.spans
            .iter()
            .any(|span| span.token_type == TokenType::Heading1)
    }));
}

#[tokio::test]
async fn connection_open_selected_file_sends_snapshot_and_single_file_grant() {
    let root = temp_workspace("selected-dispatch");
    let selected = root.join("note.md");
    let sibling = root.join("sibling.md");
    fs::write(&selected, "# selected\n").unwrap();
    fs::write(&sibling, "# sibling\n").unwrap();
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        Arc::clone(&workspace),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let capability_token = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::FileOpenCapabilityIssued { token } => token,
        message => panic!("expected FileOpenCapabilityIssued, got {message:?}"),
    };

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenSelectedFile {
                client_id: 99,
                capability: capability_token,
                selected_path: selected.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();

    let selected_root_id = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::DocumentOpened { metadata, head } => {
            assert_eq!(metadata.document_id, 1);
            assert_eq!(metadata.version, 1);
            assert_eq!(metadata.access, DocumentAccess::Editable { lease_id: 1 });
            assert_eq!(metadata.path, "note.md");
            assert_eq!(head.first_chunk, "# selected\n");
            metadata.workspace_root_id
        }
        message => panic!("expected selected DocumentOpened, got {message:?}"),
    };
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::BehaviorManifest(_)
    ));
    loop {
        if matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ) {
            break;
        }
    }

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: selected_root_id,
                path: sibling.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();
    loop {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::OutsideRoot,
                workspace_root_id: Some(id),
                document_id: None,
                ..
            } if id == selected_root_id => break,
            ServerMessage::DecorationSet(_)
            | ServerMessage::DiagnosticSet(_)
            | ServerMessage::FoldingRangeSet(_)
            | ServerMessage::RuntimeDiagnostic(_) => {}
            other => panic!("expected outside-root failure, got {other:?}"),
        }
    }

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(selected);
    let _ = fs::remove_file(sibling);
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn connection_add_selected_workspace_root_sends_file_browser_snapshot() {
    let root = temp_workspace("selected-folder-dispatch");
    fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        Arc::clone(&document),
        behavior,
        Arc::clone(&workspace),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let capability_token = match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::FileOpenCapabilityIssued { token } => token,
        message => panic!("expected FileOpenCapabilityIssued, got {message:?}"),
    };

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::AddSelectedWorkspaceRoot {
                client_id: 99,
                capability: capability_token,
                selected_path: root.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::SduiSnapshot { client_id: 99, .. }
    ));
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));
    assert_eq!(workspace.lock().await.list_root_metadata().len(), 1);

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(root.join("main.rs"));
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn connection_add_selected_workspace_root_rejects_stale_capability() {
    let root = temp_workspace("selected-folder-stale");
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::default()));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        Arc::clone(&workspace),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::AddSelectedWorkspaceRoot {
                client_id: 99,
                capability: "stale".to_string(),
                selected_path: root.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::RuntimeDiagnostic(diagnostic)
            if diagnostic.code == "client.selected_folder_open.unauthorized"
    ));
    assert!(workspace.lock().await.list_root_metadata().is_empty());

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn file_io_errors_are_typed_protocol_failures() {
    let root = temp_workspace("typed-errors");
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::default()));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace,
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    let _welcome = codec.read_server_message(&mut client).await.unwrap();
    let _snapshot = codec.read_server_message(&mut client).await.unwrap();
    let _manifest = codec.read_server_message(&mut client).await.unwrap();
    let _active_theme = codec.read_server_message(&mut client).await.unwrap();
    let _active_typography = codec.read_server_message(&mut client).await.unwrap();
    let _shell_prefs = codec.read_server_message(&mut client).await.unwrap();
    let _sdui = codec.read_server_message(&mut client).await.unwrap();
    let _tab_registry = codec.read_server_message(&mut client).await.unwrap();
    let _capability = codec.read_server_message(&mut client).await.unwrap();

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "missing.txt".to_string(),
            },
        )
        .await
        .unwrap();

    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::NotFound,
            workspace_root_id: Some(id),
            document_id: None,
            ..
        } if id == root_id
    ));

    let invalid_utf8 = root.join("invalid.txt");
    fs::write(&invalid_utf8, [0xff, 0xfe]).unwrap();
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenDocument {
                client_id: 99,
                workspace_root_id: root_id,
                path: "invalid.txt".to_string(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOperationFailed {
            code: FileErrorCode::InvalidUtf8,
            workspace_root_id: Some(id),
            document_id: None,
            ..
        } if id == root_id
    ));

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_dir(root);
}

#[tokio::test]
async fn server_rejects_invalid_frame_without_panic() {
    let (mut client, server) = duplex(4096);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::default()));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));

    tokio::io::AsyncWriteExt::write_all(&mut client, &[0, 0, 0, 4, 0xde, 0xad, 0xbe, 0xef])
        .await
        .unwrap();
    drop(client);

    let result = server_task.await.unwrap();
    assert!(result.is_err());
}

#[tokio::test]
async fn fragmented_client_frame_survives_concurrent_server_write() {
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    let (mut client, server) = duplex(4096);
    let codec = Codec::default();
    let runtime_generation = runtime_generation();
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document_state(),
        Arc::new(Mutex::new(ActiveBehaviorManifest::default())),
        workspace_state(),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation.clone(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    for _ in 0..9 {
        let _ = codec.read_server_message(&mut client).await.unwrap();
    }

    // Drip-feed a client frame start, then fire a typography broadcast so a
    // server write wins the select race mid-frame. The read pump must keep
    // frame alignment regardless of the interleaving.
    let frame = codec
        .encode_client_message(&ClientMessage::ListDocuments { client_id: 99 })
        .unwrap();
    let split = 6;
    client.write_all(&frame[..split]).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut typography = crate::protocol::ActiveTypography::default();
    typography.monospace.size += 1.0;
    runtime_generation
        .replace_typography(typography)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    client.write_all(&frame[split..]).await.unwrap();

    let mut saw_typography = false;
    let mut saw_status = false;
    for _ in 0..4 {
        match codec.read_server_message(&mut client).await.unwrap() {
            ServerMessage::ActiveTypography(_) => saw_typography = true,
            ServerMessage::DocumentList { .. } => saw_status = true,
            other => panic!("unexpected message during fragmented read: {other:?}"),
        }
        if saw_typography && saw_status {
            break;
        }
    }
    assert!(saw_typography && saw_status);

    // A second full request proves the stream stayed aligned.
    codec
        .write_client_message(&mut client, &ClientMessage::ListDocuments { client_id: 99 })
        .await
        .unwrap();
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::DocumentList { .. }
    ));

    drop(client);
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn open_selected_file_without_capability_is_rejected_with_diagnostic() {
    let root = temp_workspace("selected-unauthorized");
    let target = root.join("secret.md");
    fs::write(&target, "# secret\n").unwrap();
    let workspace = Arc::new(Mutex::new(WorkspaceState::new()));

    let (client, server) = duplex(65536);
    let codec = Codec::default();
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let server_task = tokio::spawn(handle_connection(
        server,
        99,
        document,
        behavior,
        Arc::clone(&workspace),
        sdui_state(),
        active_theme_state(),
        runtime_diagnostics(),
        runtime_generation(),
        parse_coordinator(),
        language_intelligence_coordinator(),
        codec,
    ));
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &ClientMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                client_name: "test-client".to_string(),
            },
        )
        .await
        .unwrap();
    // Consume handshake noise and the post-handshake capability so it is no
    // longer pending.
    loop {
        if matches!(
            codec.read_server_message(&mut client).await.unwrap(),
            ServerMessage::FileOpenCapabilityIssued { .. }
        ) {
            break;
        }
    }

    // Raw path with no valid capability: server must reject and must NOT
    // open or grant the file.
    codec
        .write_client_message(
            &mut client,
            &ClientMessage::OpenSelectedFile {
                client_id: 99,
                capability: String::new(),
                selected_path: target.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();
    // Re-issued pending capability first, then the rejection diagnostic.
    assert!(matches!(
        codec.read_server_message(&mut client).await.unwrap(),
        ServerMessage::FileOpenCapabilityIssued { .. }
    ));
    match codec.read_server_message(&mut client).await.unwrap() {
        ServerMessage::RuntimeDiagnostic(diagnostic) => {
            assert_eq!(diagnostic.code, "client.selected_file_open.unauthorized");
        }
        message => panic!("expected unauthorized RuntimeDiagnostic, got {message:?}"),
    }
    // No document was registered for the rejected path.
    assert!(workspace.lock().await.document_handle(1).is_none());

    drop(client);
    server_task.await.unwrap().unwrap();
    let _ = fs::remove_file(target);
    let _ = fs::remove_dir(root);
}

/// CloseDocument: final-holder close acknowledges and tears down the
/// document; a shared document survives until the last holder closes
/// (Plan 060 T6, P1-4).
#[tokio::test]
async fn close_document_acknowledges_and_tears_down_final_document() {
    let root = temp_workspace("close-document");
    fs::write(root.join("note.md"), "hello\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let runtime_generation = runtime_generation();
    let parse_coordinator = parse_coordinator();
    let document_analysis =
        crate::server::document_analysis::DocumentAnalysisCoordinator::default();

    let mut connection_a = TestConnection::connect(
        99,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation.clone(),
        parse_coordinator.clone(),
        document_analysis.clone(),
        language_intelligence_coordinator(),
    )
    .await;
    let mut connection_b = TestConnection::connect(
        100,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
    )
    .await;

    // A opens the shared document, then B opens it too.
    connection_a
        .send(&ClientMessage::OpenDocument {
            client_id: 99,
            workspace_root_id: root_id,
            path: "note.md".to_string(),
        })
        .await;
    connection_a.drain_until_quiet().await;
    connection_b
        .send(&ClientMessage::OpenDocument {
            client_id: 100,
            workspace_root_id: root_id,
            path: "note.md".to_string(),
        })
        .await;
    connection_b.drain_until_quiet().await;

    // A closes: not the final holder, document survives for B.
    connection_a
        .send(&ClientMessage::CloseDocument {
            client_id: 99,
            document_id: 1,
            force: false,
        })
        .await;
    let closed_a = connection_a.receive_response().await;
    assert!(
        matches!(
            closed_a,
            ServerMessage::DocumentClosed {
                document_id: 1,
                closed: false
            }
        ),
        "non-final close must report closed=false, got {closed_a:?}"
    );
    connection_b
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 100,
            document_id: 1,
        })
        .await;
    let status_b = connection_b.receive_response().await;
    assert!(
        matches!(status_b, ServerMessage::DocumentStatus { .. }),
        "remaining holder must still see the document, got {status_b:?}"
    );
    // A no longer has access.
    connection_a
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 99,
            document_id: 1,
        })
        .await;
    let status_a = connection_a.receive_response().await;
    assert!(
        matches!(
            status_a,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ),
        "closed connection must lose access, got {status_a:?}"
    );

    // B closes: final holder, document is torn down.
    connection_b
        .send(&ClientMessage::CloseDocument {
            client_id: 100,
            document_id: 1,
            force: false,
        })
        .await;
    let closed_b = connection_b.receive_response().await;
    assert!(
        matches!(
            closed_b,
            ServerMessage::DocumentClosed {
                document_id: 1,
                closed: true
            }
        ),
        "final close must report closed=true, got {closed_b:?}"
    );
    assert!(workspace.lock().await.document_handle(1).is_none());

    let _ = fs::remove_file(root.join("note.md"));
    let _ = fs::remove_dir(root);
}

/// Disconnect releases every access grant; documents with no remaining
/// holders are removed from the workspace registry (Plan 060 T6, P1-4).
#[tokio::test]
async fn disconnect_finalizes_documents_with_no_remaining_holders() {
    let root = temp_workspace("disconnect-finalize");
    fs::write(root.join("note.md"), "hello\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));

    let connection = TestConnection::connect(
        99,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation(),
        parse_coordinator(),
        crate::server::document_analysis::DocumentAnalysisCoordinator::default(),
        language_intelligence_coordinator(),
    )
    .await;
    let mut connection = connection;
    connection
        .send(&ClientMessage::OpenDocument {
            client_id: 99,
            workspace_root_id: root_id,
            path: "note.md".to_string(),
        })
        .await;
    connection.drain_until_quiet().await;
    assert!(workspace.lock().await.document_handle(1).is_some());

    // Disconnect: the server task exits and finalizes the document.
    drop(connection.client);
    connection.server_task.await.unwrap().unwrap();
    assert!(
        workspace.lock().await.document_handle(1).is_none(),
        "disconnect must finalize documents with no remaining holders"
    );

    let _ = fs::remove_file(root.join("note.md"));
    let _ = fs::remove_dir(root);
}

/// Phase 22.6 (plan 077 task 6): a reconnected/reclaimed tab regains
/// only its own grants. Tab A's disconnect releases every document
/// grant; a fresh connection re-opening one of the tab's documents
/// inherits nothing — the tab's other document stays unknown until
/// explicitly re-opened.
#[tokio::test]
async fn reconnected_tab_regains_only_its_own_reopened_grants() {
    let root = temp_workspace("tab-reclaim-grants");
    fs::write(root.join("note.md"), "hello\n").unwrap();
    fs::write(root.join("second.md"), "second\n").unwrap();
    let mut workspace_state_value = WorkspaceState::new();
    let root_id = workspace_state_value.add_root(&root).unwrap();
    let workspace = Arc::new(Mutex::new(workspace_state_value));
    let document = Arc::new(Mutex::new(DocumentState::new(
        7,
        "scratch".to_string(),
        DocumentAccess::Editable { lease_id: 1 },
    )));
    let behavior = Arc::new(Mutex::new(ActiveBehaviorManifest::default()));
    let runtime_generation = runtime_generation();
    let parse_coordinator = parse_coordinator();
    let document_analysis =
        crate::server::document_analysis::DocumentAnalysisCoordinator::default();

    // Tab A's connection (99) opens both of the tab's documents.
    let mut connection_a = TestConnection::connect(
        99,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation.clone(),
        parse_coordinator.clone(),
        document_analysis.clone(),
        language_intelligence_coordinator(),
    )
    .await;
    let first_document =
        open_document_until_opened(&mut connection_a, 99, root_id, "note.md").await;
    let second_document =
        open_document_until_opened(&mut connection_a, 99, root_id, "second.md").await;
    assert_eq!(first_document, 1);
    assert_eq!(second_document, 2);
    connection_a.drain_until_quiet().await;
    assert!(workspace.lock().await.document_handle(1).is_some());
    assert!(workspace.lock().await.document_handle(2).is_some());

    // Disconnect: every grant is released and both documents finalize.
    connection_a.close().await;
    assert!(
        workspace.lock().await.document_handle(1).is_none(),
        "disconnect must release the tab's first document grant"
    );
    assert!(
        workspace.lock().await.document_handle(2).is_none(),
        "disconnect must release the tab's second document grant"
    );

    // The reconnected tab (fresh connection 101) inherits nothing.
    let mut connection_c = TestConnection::connect(
        101,
        Arc::clone(&document),
        Arc::clone(&behavior),
        Arc::clone(&workspace),
        runtime_generation,
        parse_coordinator,
        document_analysis,
        language_intelligence_coordinator(),
    )
    .await;
    connection_c
        .send(&ClientMessage::ListDocuments { client_id: 101 })
        .await;
    let list = connection_c.receive_response().await;
    assert!(
        matches!(list, ServerMessage::DocumentList { ref documents } if documents.is_empty()),
        "reconnected tab must inherit no grants, got {list:?}"
    );

    // Re-opening the tab's own document grants only the new connection:
    // the old grant was finalized, so the file re-opens as a fresh
    // document with a fresh lease, not as a restored one.
    let reopened = open_document_until_opened(&mut connection_c, 101, root_id, "note.md").await;
    assert_ne!(
        reopened, first_document,
        "a finalized grant must not be re-attached; re-open is a fresh grant"
    );
    connection_c.drain_until_quiet().await;
    connection_c
        .send(&ClientMessage::GetDocumentStatus {
            client_id: 101,
            document_id: second_document,
        })
        .await;
    let status = connection_c.receive_response().await;
    assert!(
        matches!(
            status,
            ServerMessage::FileOperationFailed {
                code: FileErrorCode::UnknownDocument,
                ..
            }
        ),
        "the tab's second document stays ungranted until re-opened, got {status:?}"
    );
    connection_c
        .send(&ClientMessage::ListDocuments { client_id: 101 })
        .await;
    let list = connection_c.receive_response().await;
    assert!(
        matches!(list, ServerMessage::DocumentList { ref documents }
            if documents.len() == 1 && documents[0].document_id == reopened),
        "re-opened grant is the only grant, got {list:?}"
    );

    connection_c.close().await;
    let _ = fs::remove_file(root.join("note.md"));
    let _ = fs::remove_file(root.join("second.md"));
    let _ = fs::remove_dir(root);
}

/// Open `path` and return the granted document id, skipping open-time
/// follow-up noise (behavior manifest, diagnostics, SDUI snapshot).
async fn open_document_until_opened(
    connection: &mut TestConnection,
    client_id: u64,
    root_id: crate::protocol::WorkspaceRootId,
    path: &str,
) -> crate::protocol::DocumentId {
    connection
        .send(&ClientMessage::OpenDocument {
            client_id,
            workspace_root_id: root_id,
            path: path.to_string(),
        })
        .await;
    loop {
        match connection.receive().await {
            ServerMessage::DocumentOpened { metadata, .. } => return metadata.document_id,
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::SduiSnapshot { .. } => {}
            other => panic!("unexpected message during open: {other:?}"),
        }
    }
}

/// Runtime-diagnostic retention: consecutive duplicates collapse, the
/// deque never exceeds its capacity, and drops are counted (Plan 060 T6,
/// P1-8).
#[test]
fn runtime_diagnostic_store_deduplicates_and_bounds() {
    let mut store = RuntimeDiagnosticStore::default();
    let duplicate = RuntimeDiagnostic::warning("test.dup", "same");
    store.push(duplicate.clone());
    store.push(duplicate);
    assert_eq!(
        store.snapshot().len(),
        1,
        "consecutive duplicate must collapse"
    );
    assert_eq!(store.dropped_count(), 0);

    for index in 0..crate::perf::budgets::RUNTIME_DIAGNOSTIC_CAPACITY + 8 {
        store.push(RuntimeDiagnostic::warning(
            "test.flood",
            format!("diagnostic {index}"),
        ));
    }
    let snapshot = store.snapshot();
    assert_eq!(
        snapshot.len(),
        crate::perf::budgets::RUNTIME_DIAGNOSTIC_CAPACITY,
        "retention must stay within the snapshot cap"
    );
    // 41 total entries (1 duplicate survivor + 40 flood) minus the 32
    // retained = 9 dropped; "diagnostic 8" is the oldest survivor.
    assert_eq!(store.dropped_count(), 9);
    assert_eq!(
        snapshot.first().map(|d| d.message.as_str()),
        Some("diagnostic 8"),
        "oldest entries drop first"
    );
}
#[test]
fn sdui_command_request_forwards_list_item_id_as_argument() {
    let intent = SduiActionIntent {
        command_id: "settings.setTheme".to_string(),
        source: SduiActionSource::ListItem {
            node_id: SduiNodeId(7),
            item_id: "@clay/theme-modus-vivendi".to_string(),
        },
        arguments: Vec::new(),
    };
    let request = sdui_command_request(&intent);
    assert_eq!(request.command_id, "settings.setTheme");
    assert_eq!(
        request.arguments,
        serde_json::json!({ "item_id": "@clay/theme-modus-vivendi" })
    );
}

#[test]
fn sdui_command_request_forwards_button_node_id_as_argument() {
    let intent = SduiActionIntent {
        command_id: "settings.close".to_string(),
        source: SduiActionSource::Button {
            node_id: SduiNodeId(42),
        },
        arguments: Vec::new(),
    };
    let request = sdui_command_request(&intent);
    assert_eq!(request.arguments, serde_json::json!({ "node_id": "42" }));
}

#[test]
fn sdui_command_request_preserves_explicit_arguments() {
    let intent = SduiActionIntent {
        command_id: "workspace.openFile".to_string(),
        source: SduiActionSource::Button {
            node_id: SduiNodeId(1),
        },
        arguments: vec![SduiActionArgument {
            name: "path".to_string(),
            value: SduiActionValue::String("/tmp/a.md".to_string()),
        }],
    };
    let request = sdui_command_request(&intent);
    // Explicit arguments are preserved; source node_id is added additively.
    assert_eq!(
        request.arguments,
        serde_json::json!({ "path": "/tmp/a.md", "node_id": "1" })
    );
}

#[tokio::test]
async fn agent_surface_commands_project_client_toggle_without_server_state() {
    // Plan 108 task 8: `coding-agent.profile` / `coding-agent.close` are
    // user-authorized presentation toggles. The dispatcher answers with the
    // narrow shell-client request (the client re-parses deny-by-default);
    // no runtime generation advances and no server state changes.
    // Plan 117: the agent settings page toggle rides the same lane.
    let workspace = workspace_state();
    let document = document_state();
    let sdui = sdui_state();
    let empty_registry = CommandRegistry::new();

    for command_id in [
        "coding-agent.profile",
        "coding-agent.close",
        "coding-agent.agentSettings.open",
        "coding-agent.agentSettings.close",
    ] {
        let response = execute_command_intent(
            CommandExecutionRequest {
                command_id: command_id.to_string(),
                arguments: serde_json::Value::Null,
                target: CommandExecutionTarget::Global,
                provenance: None,
                expected_permissions: Vec::new(),
            },
            Arc::clone(&workspace),
            &document,
            &sdui,
            1,
            None,
            &empty_registry,
        )
        .await;
        assert!(
            matches!(
                response,
                Some(ServerMessage::ShellClientCommandRequest { command_id: ref id })
                    if id == command_id
            ),
            "{command_id} should project one shell-client request"
        );
    }
}

/// Plan 119 SC-6: the tab's own `TabState` answer must name its session *and*
/// the requesting client, because the relay is a process-wide fan-out — a
/// store can only adopt the binding that carries its own connection identity.
#[test]
fn tab_state_binding_names_the_requesting_client_and_session() {
    let message = session_bound_message(7, 3, "sess-42");
    let AgentServerMessage::AgentRpc { code, result_json } = message else {
        panic!("a tab binding is an agent-RPC answer");
    };
    assert_eq!(code, "session.bound");
    let value: serde_json::Value = serde_json::from_str(&result_json).expect("json payload");
    assert_eq!(value["clientId"], 7);
    assert_eq!(value["tabId"], 3);
    assert_eq!(value["sessionId"], "sess-42");

    // A tab with no session yet still adopts: the empty id means "this tab
    // owns nothing", never "unknown owner" (which stays a client-side state).
    let message = session_bound_message(7, 3, "");
    let AgentServerMessage::AgentRpc { result_json, .. } = message else {
        panic!("a tab binding is an agent-RPC answer");
    };
    let value: serde_json::Value = serde_json::from_str(&result_json).expect("json payload");
    assert_eq!(value["sessionId"], "");
}

/// Plan 129 task 3: crafted state for driving extracted handlers directly,
/// without a socket, a server, or the connection loop. Handlers take only the
/// context, so each one is independently callable from a test.
struct DirectHandlerState {
    codec: Codec,
    client_id: u64,
    document: Arc<Mutex<DocumentState>>,
    workspace: Arc<Mutex<WorkspaceState>>,
    behavior: Arc<Mutex<ActiveBehaviorManifest>>,
    runtime_generation: super::RuntimeGenerationStore,
    sdui: Arc<Mutex<StaticSduiState>>,
    parse_coordinator: ParseCoordinator,
    completion: crate::server::completion::CompletionCoordinator,
    language_intelligence: LanguageIntelligenceCoordinator,
    document_analysis: crate::server::document_analysis::DocumentAnalysisCoordinator,
    menu_sessions: crate::server::menu_sessions::ServerMenuSessions,
    bound_tab_id: Option<crate::protocol::TabId>,
    bound_state: Arc<std::sync::Mutex<Option<super::TabServerState>>>,
    tab_registry: Arc<Mutex<crate::server::tab_registry::TabRegistry>>,
    tab_registry_tx: tokio::sync::broadcast::Sender<crate::protocol::TabRegistrySnapshot>,
    _tab_registry_rx: tokio::sync::broadcast::Receiver<crate::protocol::TabRegistrySnapshot>,
    pending_viewport_patches: std::collections::HashMap<
        (
            crate::protocol::DocumentId,
            crate::protocol::ViewportRequestId,
        ),
        super::PendingViewportPatch,
    >,
    completion_tx: tokio::sync::mpsc::Sender<ServerMessage>,
    _completion_rx: tokio::sync::mpsc::Receiver<ServerMessage>,
    language_intelligence_tx: tokio::sync::mpsc::Sender<ServerMessage>,
    _language_intelligence_rx: tokio::sync::mpsc::Receiver<ServerMessage>,
    dropped_results: Arc<std::sync::atomic::AtomicU64>,
    file_open_capabilities: super::FileOpenCapabilityPool,
}

impl DirectHandlerState {
    fn new() -> Self {
        let (tab_registry_tx, _tab_registry_rx) = tokio::sync::broadcast::channel(8);
        let (completion_tx, _completion_rx) =
            tokio::sync::mpsc::channel(crate::perf::budgets::CONNECTION_RESULT_LANE_CAPACITY);
        let (language_intelligence_tx, _language_intelligence_rx) =
            tokio::sync::mpsc::channel(crate::perf::budgets::CONNECTION_RESULT_LANE_CAPACITY);
        Self {
            codec: Codec::default(),
            client_id: 1,
            document: document_state(),
            workspace: workspace_state(),
            behavior: Arc::new(Mutex::new(
                ActiveBehaviorManifest::new(BehaviorManifest::minimal_text_editing(1))
                    .expect("minimal text editing manifest is valid"),
            )),
            runtime_generation: runtime_generation(),
            sdui: sdui_state(),
            parse_coordinator: parse_coordinator(),
            completion: crate::server::completion::CompletionCoordinator::new(),
            language_intelligence: language_intelligence_coordinator(),
            document_analysis:
                crate::server::document_analysis::DocumentAnalysisCoordinator::default(),
            menu_sessions: crate::server::menu_sessions::ServerMenuSessions::new(),
            bound_tab_id: None,
            bound_state: Arc::new(std::sync::Mutex::new(None)),
            tab_registry: Arc::new(Mutex::new(crate::server::tab_registry::TabRegistry::new())),
            tab_registry_tx,
            _tab_registry_rx,
            pending_viewport_patches: std::collections::HashMap::new(),
            completion_tx,
            _completion_rx,
            language_intelligence_tx,
            _language_intelligence_rx,
            dropped_results: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            file_open_capabilities: super::FileOpenCapabilityPool::new(),
        }
    }

    fn ctx<'a>(
        &'a mut self,
        stream: &'a mut tokio::io::DuplexStream,
    ) -> super::ConnectionCtx<'a, tokio::io::DuplexStream> {
        super::ConnectionCtx {
            codec: self.codec,
            stream,
            client_id: self.client_id,
            document: &mut self.document,
            workspace: &mut self.workspace,
            behavior: &self.behavior,
            runtime_generation: &self.runtime_generation,
            sdui: &self.sdui,
            parse_coordinator: &self.parse_coordinator,
            completion: &self.completion,
            language_intelligence: &self.language_intelligence,
            document_analysis: &self.document_analysis,
            reload_server: None,
            file_open_capabilities: &mut self.file_open_capabilities,
            menu_sessions: &mut self.menu_sessions,
            bound_tab_id: &mut self.bound_tab_id,
            bound_state: &self.bound_state,
            tab_registry: &self.tab_registry,
            tab_registry_tx: &self.tab_registry_tx,
            pending_viewport_patches: &mut self.pending_viewport_patches,
            completion_tx: &self.completion_tx,
            language_intelligence_tx: &self.language_intelligence_tx,
            dropped_results: &self.dropped_results,
        }
    }
}

#[tokio::test]
async fn extracted_list_documents_handler_answers_without_the_loop() {
    let mut state = DirectHandlerState::new();
    let (mut server_side, mut peer) = duplex(64 * 1024);
    {
        let mut ctx = state.ctx(&mut server_side);
        super::documents::handle_list_documents(&mut ctx)
            .await
            .expect("list documents writes its response");
    }
    let ServerMessage::DocumentList { documents } = state
        .codec
        .read_server_message(&mut peer)
        .await
        .expect("the handler wrote one response")
    else {
        panic!("list documents answers with the document list");
    };
    assert!(
        documents.is_empty(),
        "a fresh connection owns no documents: {documents:?}"
    );
}

#[tokio::test]
async fn extracted_launcher_handler_answers_without_the_loop() {
    let mut state = DirectHandlerState::new();
    let (mut server_side, mut peer) = duplex(64 * 1024);
    {
        let mut ctx = state.ctx(&mut server_side);
        super::workspace::handle_list_launcher_entries(&mut ctx)
            .await
            .expect("launcher listing writes its response");
    }
    let ServerMessage::LauncherEntries { client_id, entries } = state
        .codec
        .read_server_message(&mut peer)
        .await
        .expect("the handler wrote one response")
    else {
        panic!("launcher listing answers with the launcher entries");
    };
    assert_eq!(client_id, state.client_id);
    assert!(
        entries.workspaces.is_empty() && entries.agents.is_empty(),
        "no configuration root means the first-run empty launcher: {entries:?}"
    );
    assert_eq!(entries.pruned, 0);
}

#[tokio::test]
async fn extracted_duplicate_hello_handler_rejects_without_the_loop() {
    let mut state = DirectHandlerState::new();
    let (mut server_side, mut peer) = duplex(64 * 1024);
    {
        let mut ctx = state.ctx(&mut server_side);
        super::runtime::handle_duplicate_hello(&mut ctx)
            .await
            .expect("the duplicate-hello reply writes");
    }
    let ServerMessage::Error { code, message } = state
        .codec
        .read_server_message(&mut peer)
        .await
        .expect("the handler wrote one response")
    else {
        panic!("a second Hello answers with a protocol error");
    };
    assert_eq!(code, ProtocolErrorCode::InvalidMessage);
    assert_eq!(message, "duplicate Hello message");
}
