//! Plan 097 Phase 3: serde DTO round trips for every bridged protocol family,
//! plus bridge request validation (size cap, malformed JSON, unknown family).
//!
//! The exhaustive `family` matchers are compile-time guards: adding a
//! `ClientMessage`/`ServerMessage` variant without updating this file breaks
//! the build, so the frontend bridge can never silently miss a family.

use clay::client::ClientConnectionEvent;
use clay::protocol::{
    AgentClientCommand, AgentPickerKind, AgentServerMessage, ClientMessage,
    CompletionReplacementRange, CompletionRequest, CompletionTrigger, DecorationKind,
    DecorationProvenance, DecorationSet, DiagnosticSet, DiagnosticSeverity, DiagnosticSpan,
    DocumentMetadata, FontProfile, LanguageIntelligenceFeature, LanguageIntelligenceRequest,
    ProtocolErrorCode, RuntimeDiagnostic, SduiActionIntent, SduiActionSource, SduiNode,
    SduiNodeKind, SduiTree, SduiTreeUpdate, SelectionQuery, SelectionQueryCursor,
    SelectionQueryRequest, ServerMessage, TabCommand, TabEntry, TabRegistrySnapshot,
    TextThemeOverride, TextobjectDirection, TextobjectKind, TransientMenuActivationData,
    TransientMenuFocusPolicyData, TransientMenuOriginData, TransientMenuSnapshotData,
    TransientMenuStatusData, WrapPolicy,
};

fn provenance() -> DecorationProvenance {
    DecorationProvenance {
        package_name: "core".into(),
        package_version: "1".into(),
        package_prefix: "clay".into(),
    }
}

fn font_profile() -> FontProfile {
    FontProfile {
        families: vec!["monospace".into()],
        size: 13.0,
        ligatures: Box::default(),
    }
}

// ---------------------------------------------------------------- samples

fn client_samples() -> Vec<ClientMessage> {
    vec![
        ClientMessage::Hello {
            protocol_version: 25,
            client_name: "probe".into(),
        },
        ClientMessage::Edit {
            document_id: 1,
            client_id: 2,
            lease_id: Some(3),
            base_version: 4,
            behavior_version: 5,
            transaction_id: 6,
            operation: clay::protocol::EditOperation::Insert {
                byte_offset: 0,
                text: "hi".into(),
            },
        },
        ClientMessage::EditorIntent {
            document_id: 1,
            client_id: 2,
            lease_id: None,
            base_version: 4,
            behavior_version: 5,
            transaction_id: 6,
            intent: clay::protocol::EditorIntent::DeleteRange { start: 1, end: 2 },
        },
        ClientMessage::RequestResync {
            client_id: 2,
            document_id: 1,
            known_version: 4,
        },
        ClientMessage::DecorationViewportRequest {
            client_id: 2,
            document_id: 1,
            document_version: 4,
            byte_start: 0,
            byte_end: 64,
        },
        ClientMessage::OpenDocument {
            client_id: 2,
            workspace_root_id: 7,
            path: "notes/todo.md".into(),
        },
        ClientMessage::OpenSelectedFile {
            client_id: 2,
            capability: "cap-token".into(),
            selected_path: "/tmp/a.md".into(),
        },
        ClientMessage::AddSelectedWorkspaceRoot {
            client_id: 2,
            capability: "cap-token".into(),
            selected_path: "/tmp/ws".into(),
        },
        ClientMessage::SaveDocument {
            client_id: 2,
            document_id: 1,
            known_version: 4,
        },
        ClientMessage::ReloadDocument {
            client_id: 2,
            document_id: 1,
            known_version: 4,
            force: false,
        },
        ClientMessage::GetDocumentStatus {
            client_id: 2,
            document_id: 1,
        },
        ClientMessage::ListDocuments { client_id: 2 },
        ClientMessage::SduiAction {
            client_id: 2,
            ui_version: 9,
            intent: SduiActionIntent {
                command_id: "palette.open".into(),
                source: SduiActionSource::Button {
                    node_id: clay::protocol::SduiNodeId(4),
                },
                arguments: Vec::new(),
            },
        },
        ClientMessage::CommandIntent {
            client_id: 2,
            document_id: 1,
            behavior_version: 5,
            command_id: "editor.toggleComment".into(),
        },
        ClientMessage::CompletionRequest {
            request: CompletionRequest {
                request_id: 11,
                client_id: 2,
                document_id: 1,
                document_version: 4,
                behavior_version: 5,
                cursor_byte_offset: 8,
                replacement_range: CompletionReplacementRange::new(4, 8),
                trigger: CompletionTrigger::Character(">".into()),
                provider_generation: 1,
                recent_completions: Box::new([]),
            },
        },
        ClientMessage::LanguageIntelligenceRequest {
            request: LanguageIntelligenceRequest {
                request_id: 12,
                client_id: 2,
                document_id: 1,
                document_version: 4,
                behavior_version: 5,
                cursor_byte_offset: 8,
                feature: LanguageIntelligenceFeature::Hover,
                provider_generation: 1,
            },
        },
        ClientMessage::SelectionQueryRequest {
            request: SelectionQueryRequest {
                request_id: 13,
                client_id: 2,
                document_id: 1,
                document_version: 4,
                behavior_version: 5,
                query: SelectionQuery::Textobject {
                    kind: TextobjectKind::Function,
                    around: true,
                    direction: TextobjectDirection::Next,
                },
                selections: vec![SelectionQueryCursor {
                    anchor: 0,
                    focus: 3,
                }],
            },
        },
        ClientMessage::RuntimeGenerationInstalled {
            client_id: 2,
            runtime_generation_id: 14,
        },
        ClientMessage::CloseDocument {
            client_id: 2,
            document_id: 1,
            force: true,
        },
        ClientMessage::TabCommand {
            client_id: 2,
            command: TabCommand::New {
                workspace_root: String::new(),
            },
        },
        ClientMessage::MenuQueryUpdate {
            client_id: 2,
            session_id: menu_session_id(),
            query: "rea".into(),
        },
        ClientMessage::MenuBackspace {
            client_id: 2,
            session_id: menu_session_id(),
        },
        ClientMessage::MenuSelectionMove {
            client_id: 2,
            session_id: menu_session_id(),
            delta: -1,
        },
        ClientMessage::MenuActivate {
            client_id: 2,
            session_id: menu_session_id(),
            kind: TransientMenuActivationData::Primary,
        },
        ClientMessage::MenuCancel {
            client_id: 2,
            session_id: menu_session_id(),
        },
        ClientMessage::Agent {
            client_id: 2,
            command: Box::new(AgentClientCommand::Prompt {
                session_id: "sess-1".into(),
                text: "hello agent".into(),
            }),
        },
    ]
}

fn menu_session_id() -> u64 {
    // Server-partitioned id (high bit set): exceeds JS safe integers.
    1u64 << 63 | 7
}

/// Exhaustive variant guard for [`ClientMessage`].
fn client_family(message: &ClientMessage) -> &'static str {
    match message {
        ClientMessage::Hello { .. } => "hello",
        ClientMessage::Edit { .. } => "edit",
        ClientMessage::EditorIntent { .. } => "editorIntent",
        ClientMessage::RequestResync { .. } => "requestResync",
        ClientMessage::DecorationViewportRequest { .. } => "decorationViewportRequest",
        ClientMessage::OpenDocument { .. } => "openDocument",
        ClientMessage::OpenSelectedFile { .. } => "openSelectedFile",
        ClientMessage::AddSelectedWorkspaceRoot { .. } => "addSelectedWorkspaceRoot",
        ClientMessage::SaveDocument { .. } => "saveDocument",
        ClientMessage::ReloadDocument { .. } => "reloadDocument",
        ClientMessage::GetDocumentStatus { .. } => "getDocumentStatus",
        ClientMessage::ListDocuments { .. } => "listDocuments",
        ClientMessage::SduiAction { .. } => "sduiAction",
        ClientMessage::CommandIntent { .. } => "commandIntent",
        ClientMessage::CompletionRequest { .. } => "completionRequest",
        ClientMessage::LanguageIntelligenceRequest { .. } => "languageIntelligenceRequest",
        ClientMessage::SelectionQueryRequest { .. } => "selectionQueryRequest",
        ClientMessage::RuntimeGenerationInstalled { .. } => "runtimeGenerationInstalled",
        ClientMessage::CloseDocument { .. } => "closeDocument",
        ClientMessage::TabCommand { .. } => "tabCommand",
        ClientMessage::MenuQueryUpdate { .. } => "menuQueryUpdate",
        ClientMessage::MenuBackspace { .. } => "menuBackspace",
        ClientMessage::MenuSelectionMove { .. } => "menuSelectionMove",
        ClientMessage::MenuActivate { .. } => "menuActivate",
        ClientMessage::MenuCancel { .. } => "menuCancel",
        ClientMessage::Agent { .. } => "agent",
    }
}

fn server_samples() -> Vec<ServerMessage> {
    let metadata = || DocumentMetadata {
        document_id: 1,
        version: 4,
        access: clay::protocol::DocumentAccess::ReadOnly,
        lease_id: Some(3),
        dirty: false,
        workspace_root_id: 7,
        path: "notes/todo.md".into(),
    };
    let deco_set = || DecorationSet {
        document_id: 1,
        document_version: 4,
        package_prefix: "clay".into(),
        kind: DecorationKind::Syntax,
        viewport_byte_start: 0,
        viewport_byte_end: 64,
        spans: Vec::new(),
    };
    vec![
        ServerMessage::Welcome {
            client_id: 2,
            protocol_version: 25,
        },
        ServerMessage::InitialDocument {
            document_id: 1,
            version: 4,
            text: "# hello".into(),
            access: clay::protocol::DocumentAccess::ReadOnly,
            lease_id: Some(3),
            workspace_root: "/tmp/ws".into(),
        },
        ServerMessage::BehaviorManifest(Box::new(
            clay::protocol::BehaviorManifest::minimal_text_editing(5),
        )),
        ServerMessage::SduiSnapshot {
            client_id: 2,
            tree: SduiTree {
                ui_version: 9,
                root_id: clay::protocol::SduiNodeId(1),
                nodes: vec![SduiNode::new(
                    clay::protocol::SduiNodeId(1),
                    SduiNodeKind::Label {
                        text: "ready".into(),
                    },
                )],
            },
        },
        ServerMessage::FileOpenCapabilityIssued {
            token: "cap-token".into(),
        },
        ServerMessage::SduiUpdate {
            update: SduiTreeUpdate {
                base_ui_version: 9,
                new_ui_version: 10,
                operations: vec![clay::protocol::SduiTreeOperation::ReplaceRoot {
                    root_id: clay::protocol::SduiNodeId(2),
                }],
            },
        },
        ServerMessage::DecorationSet(deco_set()),
        ServerMessage::DecorationBatch(vec![deco_set()]),
        ServerMessage::DiagnosticSet(DiagnosticSet {
            document_id: 1,
            document_version: 4,
            viewport_byte_start: 0,
            viewport_byte_end: 64,
            source: "test".into(),
            provenance: provenance(),
            spans: vec![DiagnosticSpan {
                byte_start: 0,
                byte_end: 4,
                severity: DiagnosticSeverity::Warning,
                code: "t.w".into(),
                message: "watch out".into(),
                source: "test".into(),
                provenance: provenance(),
            }],
        }),
        ServerMessage::EditAck {
            document_id: 1,
            confirmed_version: 5,
            transaction_id: 6,
        },
        ServerMessage::EditRejected {
            document_id: 1,
            transaction_id: 6,
            reason: clay::protocol::EditRejection::LeaseRequired,
        },
        ServerMessage::EditTransaction {
            document_id: 1,
            version: 5,
            transaction_id: 6,
            operations: vec![clay::protocol::EditOperation::Insert {
                byte_offset: 0,
                text: "x".into(),
            }],
        },
        ServerMessage::ResyncSnapshot {
            document_id: 1,
            version: 5,
            text: "state".into(),
            access: clay::protocol::DocumentAccess::ReadOnly,
            lease_id: Some(3),
        },
        ServerMessage::DocumentOpened {
            metadata: metadata(),
            text: "body".into(),
        },
        ServerMessage::DocumentSaved {
            document_id: 1,
            version: 5,
            dirty: false,
        },
        ServerMessage::DocumentReloaded {
            metadata: metadata(),
            text: "body".into(),
        },
        ServerMessage::DocumentStatus {
            metadata: metadata(),
        },
        ServerMessage::DocumentList {
            documents: vec![metadata()],
        },
        ServerMessage::FileOperationFailed {
            code: clay::protocol::FileErrorCode::NotFound,
            message: "gone".into(),
            workspace_root_id: Some(7),
            document_id: Some(1),
        },
        ServerMessage::RuntimeDiagnostic(RuntimeDiagnostic::warning("w", "careful")),
        ServerMessage::Error {
            code: ProtocolErrorCode::InvalidMessage,
            message: "bad".into(),
        },
        ServerMessage::DocumentClosed {
            document_id: 1,
            closed: true,
        },
        ServerMessage::TransientMenuSnapshot(Box::new(TransientMenuSnapshotData::new(
            menu_session_id(),
            "Go to file",
            "rea",
            Vec::new(),
            0,
            TransientMenuStatusData::Active,
            TransientMenuFocusPolicyData::Modal,
            TransientMenuOriginData::CommandPalette,
        ))),
        ServerMessage::TransientMenuClosed {
            session_id: menu_session_id(),
        },
        ServerMessage::ShellClientCommandRequest {
            command_id: "workspace.openSettings".into(),
        },
        ServerMessage::Agent(Box::new(AgentServerMessage::Picker {
            kind: AgentPickerKind::Model,
            items: Vec::new(),
        })),
    ]
}

/// Exhaustive variant guard for [`ServerMessage`].
fn server_family(message: &ServerMessage) -> &'static str {
    match message {
        ServerMessage::Welcome { .. } => "welcome",
        ServerMessage::InitialDocument { .. } => "initialDocument",
        ServerMessage::BehaviorManifest(_) => "behaviorManifest",
        ServerMessage::SduiSnapshot { .. } => "sduiSnapshot",
        ServerMessage::FileOpenCapabilityIssued { .. } => "fileOpenCapabilityIssued",
        ServerMessage::SduiUpdate { .. } => "sduiUpdate",
        ServerMessage::DecorationSet(_) => "decorationSet",
        ServerMessage::FoldingRangeSet(_) => "foldingRangeSet",
        ServerMessage::DecorationBatch(_) => "decorationBatch",
        ServerMessage::DiagnosticSet(_) => "diagnosticSet",
        ServerMessage::EditAck { .. } => "editAck",
        ServerMessage::EditRejected { .. } => "editRejected",
        ServerMessage::EditTransaction { .. } => "editTransaction",
        ServerMessage::ResyncSnapshot { .. } => "resyncSnapshot",
        ServerMessage::DocumentOpened { .. } => "documentOpened",
        ServerMessage::DocumentSaved { .. } => "documentSaved",
        ServerMessage::DocumentReloaded { .. } => "documentReloaded",
        ServerMessage::DocumentStatus { .. } => "documentStatus",
        ServerMessage::DocumentList { .. } => "documentList",
        ServerMessage::FileOperationFailed { .. } => "fileOperationFailed",
        ServerMessage::RuntimeDiagnostic(_) => "runtimeDiagnostic",
        ServerMessage::CompletionResult { .. } => "completionResult",
        ServerMessage::CompletionRejected { .. } => "completionRejected",
        ServerMessage::LanguageIntelligenceResult { .. } => "languageIntelligenceResult",
        ServerMessage::LanguageIntelligenceRejected { .. } => "languageIntelligenceRejected",
        ServerMessage::SelectionQueryResult { .. } => "selectionQueryResult",
        ServerMessage::EditorCommandRequest(_) => "editorCommandRequest",
        ServerMessage::CaretStyleOverride(_) => "caretStyleOverride",
        ServerMessage::EditorLayoutOverride(_) => "editorLayoutOverride",
        ServerMessage::ShellPreferences(_) => "shellPreferences",
        ServerMessage::TabRegistry(_) => "tabRegistry",
        ServerMessage::ActiveTheme(_) => "activeTheme",
        ServerMessage::ActiveTypography(_) => "activeTypography",
        ServerMessage::RuntimeStateSnapshot(_) => "runtimeStateSnapshot",
        ServerMessage::Error { .. } => "error",
        ServerMessage::DocumentClosed { .. } => "documentClosed",
        ServerMessage::TransientMenuSnapshot(_) => "transientMenuSnapshot",
        ServerMessage::TransientMenuClosed { .. } => "transientMenuClosed",
        ServerMessage::ShellClientCommandRequest { .. } => "shellClientCommandRequest",
        ServerMessage::Agent(_) => "agent",
    }
}

// ---------------------------------------------------------------- tests

#[test]
fn every_client_message_family_round_trips_through_json() {
    for sample in client_samples() {
        let json = serde_json::to_string(&sample).expect("serialize");
        let parsed: ClientMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            parsed,
            sample,
            "round trip diverged for {}",
            client_family(&sample)
        );
    }
}

#[test]
fn every_server_message_sample_round_trips_through_json() {
    for sample in server_samples() {
        let json = serde_json::to_string(&sample).expect("serialize");
        let parsed: ServerMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            parsed,
            sample,
            "round trip diverged for {}",
            server_family(&sample)
        );
    }
}

#[test]
fn envelope_envelopes_round_trip_with_camel_case_kinds() {
    let event = ClientConnectionEvent::EditAck {
        document_id: 1,
        version: 5,
        transaction_id: 6,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(
        json.contains(r#""kind":"editAck""#),
        "unexpected shape: {json}"
    );
    let parsed: ClientConnectionEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, event);
}

#[test]
fn large_menu_session_ids_cross_json_as_strings() {
    let message = ClientMessage::MenuCancel {
        client_id: 2,
        session_id: menu_session_id(),
    };
    let json = serde_json::to_value(&message).unwrap();
    let payload = &json["payload"];
    assert_eq!(
        payload["sessionId"],
        serde_json::Value::String((1u64 << 63 | 7).to_string())
    );
}

#[test]
fn theme_and_typography_snapshots_round_trip() {
    let theme = clay::protocol::ActiveTheme {
        specifier: "@clay/theme-gruvbox-material-dark".into(),
        overrides: vec![TextThemeOverride {
            token: "Keyword".into(),
            color: Some([240, 180, 60, 255]),
            background: None,
            bold: Some(true),
            italic: None,
            underline: None,
            strike: None,
            scale: None,
            provenance: "clay".into(),
        }],
        design_tokens: Vec::new(),
    };
    let json = serde_json::to_string(&theme).unwrap();
    let parsed: clay::protocol::ActiveTheme = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, theme);

    let typography = clay::protocol::ActiveTypography {
        revision: 3,
        monospace: font_profile(),
        proportional: font_profile(),
        ui: font_profile(),
        hierarchy: Default::default(),
    };
    let json = serde_json::to_string(&typography).unwrap();
    let parsed: clay::protocol::ActiveTypography = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, typography);
}

#[test]
fn wrap_policy_and_tab_registry_round_trip() {
    let registry = TabRegistrySnapshot {
        tabs: vec![TabEntry {
            tab_id: 1,
            workspace_root_id: 7,
            client_id: 2,
            workspace_root: "/tmp/ws".into(),
        }],
        active: Some(1),
        revision: 3,
    };
    let json = serde_json::to_string(&registry).unwrap();
    let parsed: TabRegistrySnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, registry);

    let json = serde_json::to_string(&WrapPolicy::None).unwrap();
    assert_eq!(json, r#""none""#);
}

#[test]
fn theme_snapshot_envelope_shape() {
    use clay::shell::theme::ThemeTokenValueDto;
    use std::collections::BTreeMap;

    let mut tokens = BTreeMap::new();
    tokens.insert(
        "surface.main".to_string(),
        ThemeTokenValueDto::Color("#100f17".to_string()),
    );
    tokens.insert(
        "density.default".to_string(),
        ThemeTokenValueDto::Level("default".to_string()),
    );
    let snapshot = clay_desktop_lib::bridge::ThemeSnapshotDto {
        specifier: "@clay/theme-x".into(),
        tokens,
        editor_styles: BTreeMap::new(),
        density_scale: 1.0,
    };
    let json = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(json["specifier"], "@clay/theme-x");
    assert_eq!(json["tokens"]["surface.main"]["type"], "color");
    assert_eq!(json["tokens"]["surface.main"]["value"], "#100f17");
    assert_eq!(json["tokens"]["density.default"]["type"], "level");
}
