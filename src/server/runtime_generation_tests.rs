use std::{
    fs,
    time::{Duration, SystemTime},
};

use crate::{
    ipc::IpcEndpoint,
    packages::record::assemble_package_record,
    protocol::{
        ActiveTheme, BehaviorManifest, FontProfile, IncrementalParseUpdate, KeyCode, KeyModifiers,
        KeyStroke, ParseByteRange, ParseEditNotification, ServerMessage,
    },
    server::{
        command_execution::{
            CommandExecutionRequest, CommandExecutionRule, CommandExecutionTarget,
            RELOAD_CONFIGURATION_COMMAND_ID,
        },
        completion::BufferWordCompletionProvider,
        js_runtime::ClayRuntimeEvaluation,
        language_intelligence::LanguageIntelligenceProviderMeta,
        parse_coordinator::ParseScheduleRequest,
        sdui::default_document_tree,
    },
};

use super::{IpcServer, ServerConfig};
use crate::server::runtime_reload::withdraw_package_contributions;

#[tokio::test]
async fn command_catalogue_merges_loaded_packages_with_exact_provenance() {
    // Phase 24.2 verification: the live catalogue must merge real
    // first-party packages with exact (name, version) provenance and
    // declared key bindings — synthetic fixtures cannot catch
    // registration drift in shipped packages.
    let root = temp_config_root(
        "catalogue-provenance",
        r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");
await loadPackage("@clay/settings");
await loadPackage("@clay/javascript");
await loadPackage("@clay/typescript");"#,
    );
    let server = server_with_config(root.clone());
    assert!(server.reload_runtime_generation().await.reloaded);
    // Open a real Markdown document through the follow-up message path so
    // the markdown mode layer (with its keyRouting contribution) becomes
    // the active manifest for the snapshot.
    let metadata = crate::protocol::DocumentMetadata {
        document_id: 2,
        version: 1,
        access: crate::protocol::DocumentAccess::Editable { lease_id: 1 },
        lease_id: Some(1),
        dirty: false,
        workspace_root_id: 1,
        path: "note.md".to_string(),
    };
    let generation_id = server.runtime_generation.generation_id().await;
    let service = server.runtime_generation.current_service().await;
    let document = std::sync::Arc::new(tokio::sync::Mutex::new(
        crate::server::document::DocumentState::new(
            2,
            "# Hi\n".to_string(),
            crate::protocol::DocumentAccess::Editable { lease_id: 1 },
        ),
    ));
    crate::server::connection::open_document_followup_messages(
        &metadata,
        &document,
        &server.behavior,
        &server.sdui,
        generation_id,
        &service,
        &server.parse_coordinator,
    )
    .await;
    let manifest = server
        .behavior
        .lock()
        .await
        .manifest_for(metadata.document_id)
        .clone();
    assert_eq!(manifest.manifest_id, "markdown.markdown");
    let (_, catalogue) = server
        .runtime_generation
        .command_catalogue_snapshot(&manifest)
        .await
        .expect("catalogue with loaded packages should validate");

    let find = |command_id: &str| {
        catalogue
            .commands()
            .iter()
            .find(|command| command.command_id == command_id)
            .unwrap_or_else(|| panic!("catalogue missing {command_id}"))
    };

    // Markdown family: package-owned IDs carry exact package provenance
    // and the package-declared key-routing descriptor with parsed
    // modifiers.
    for (command_id, chord, expected_stroke) in [
        (
            "markdown.togglePreview",
            "Ctrl+Shift+M",
            KeyStroke {
                key: KeyCode::Character("m".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    shift: true,
                    ..KeyModifiers::NONE
                },
            },
        ),
        (
            "markdown.insertHeading",
            "Ctrl+Alt+1",
            KeyStroke {
                key: KeyCode::Character("1".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    alt: true,
                    ..KeyModifiers::NONE
                },
            },
        ),
        (
            "markdown.toggleList",
            "Ctrl+Shift+8",
            KeyStroke {
                key: KeyCode::Character("8".to_string()),
                modifiers: KeyModifiers {
                    control: true,
                    shift: true,
                    ..KeyModifiers::NONE
                },
            },
        ),
    ] {
        let command = find(command_id);
        assert_eq!(command.package_name, "@clay/markdown");
        assert_eq!(command.package_version, "0.1.0");
        assert_eq!(command.api_prefix, "markdown");
        assert_eq!(
            command.routing_policy,
            crate::protocol::RoutingPolicy::ServerFirst
        );
        assert!(
            command
                .key_bindings
                .iter()
                .any(|rule| rule.sequence == vec![expected_stroke.clone()]),
            "{command_id} should keep its declared key-routing descriptor ({chord})"
        );
    }
    // Toggle-comment (no default binding) and the settings family
    // (server-first, no chords) still surface with exact provenance.
    for command_id in [
        "markdown.toggleComment",
        "settings.open",
        "settings.close",
        "settings.setTheme",
        "settings.setDesignSystem",
        "settings.setAppearance",
        "settings.setTypography",
        "settings.reset",
        "javascript.toggleLineComment",
        "typescript.toggleLineComment",
    ] {
        find(command_id);
    }
    assert_eq!(
        find("markdown.toggleComment").package_name,
        "@clay/markdown"
    );
    assert_eq!(
        find("javascript.toggleLineComment").package_name,
        "@clay/javascript"
    );
    assert_eq!(
        find("typescript.toggleLineComment").package_name,
        "@clay/typescript"
    );
    assert_eq!(find("settings.open").package_name, "@clay/settings");
    assert_eq!(find("settings.open").package_version, "0.1.0");
    assert_eq!(find("settings.open").api_prefix, "settings");

    // Clay-owned entries keep BuiltIn provenance (package_name "clay")
    // even when packages are loaded.
    assert_eq!(
        find("runtime.reloadConfiguration").package_name,
        "clay",
        "built-ins must not be mislabeled as package commands"
    );
    assert_eq!(
        find("shell.clientSplitPaneVertical").package_name,
        "clay",
        "shell client commands must stay Clay-owned in the catalogue"
    );

    let _ = fs::remove_dir_all(root);
}

fn temp_config_root(name: &str, init_js: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "clay-runtime-generation-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("init.js"), init_js).unwrap();
    root
}

/// Copy the canonical example tree (init.js + packages/ + agents/) into
/// a fresh temp config root, mirroring the `cp -r examples/. ~/.clay/`
/// setup from test-plan/02.
fn temp_example_config_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "clay-runtime-generation-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/config");
    copy_tree(&examples, &root);
    root
}

/// Recursive copy mirroring `cp -r` (the example tree nests per-agent
/// config under `agents/coding-agent/`).
fn copy_tree(src: &std::path::Path, dst: &std::path::Path) {
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir(&target).unwrap();
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn server_with_config(root: std::path::PathBuf) -> IpcServer {
    let mut config = ServerConfig::new(IpcEndpoint::from_argument("runtime-generation-test"));
    config.configuration_root = Some(root);
    IpcServer::new(config)
}

/// Poll `condition` every 10 ms under a bounded deadline. On timeout the
/// panic names the scenario plus live server state (generation id and
/// diagnostic codes) so a stalled watcher/menu/runtime test points at
/// pending session/runtime-replacement cleanup instead of a bare
/// `Elapsed` panic. Test-only; adds no production work. Callers capture
/// the server by reference in the closure (non-`move` async block).
async fn wait_until<C, F>(
    server: &IpcServer,
    scenario: &str,
    deadline: std::time::Duration,
    mut condition: C,
) where
    C: FnMut() -> F,
    F: std::future::Future<Output = bool>,
{
    let timed_out = tokio::time::timeout(deadline, async {
        loop {
            if condition().await {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .is_err();

    if timed_out {
        let generation_id = server.runtime_generation.generation_id().await;
        let diagnostics: Vec<String> = server
            .runtime_diagnostics
            .lock()
            .await
            .snapshot()
            .iter()
            .map(|diagnostic| diagnostic.code.clone())
            .collect();
        panic!(
            "{scenario} exceeded its {deadline:?} bound; \
             generation_id={generation_id} diagnostics={diagnostics:?} — \
             look for pending session/runtime-replacement cleanup"
        );
    }
}

async fn bind_test_tab(client: &mut tokio::io::DuplexStream, codec: crate::protocol::codec::Codec) {
    let client_id = loop {
        match codec.read_server_message(client).await.unwrap() {
            ServerMessage::Welcome { client_id, .. } => break client_id,
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::RuntimeStateSnapshot(_)
            | ServerMessage::TabRegistry(_)
            | ServerMessage::FileOpenCapabilityIssued { .. } => {}
            message => panic!("expected handshake message, got {message:?}"),
        }
    };
    loop {
        match codec.read_server_message(client).await.unwrap() {
            ServerMessage::FileOpenCapabilityIssued { .. } => break,
            ServerMessage::BehaviorManifest(_)
            | ServerMessage::ActiveTheme(_)
            | ServerMessage::ActiveTypography(_)
            | ServerMessage::CaretStyleOverride(_)
            | ServerMessage::EditorLayoutOverride(_)
            | ServerMessage::ShellPreferences(_)
            | ServerMessage::RuntimeDiagnostic(_)
            | ServerMessage::RuntimeStateSnapshot(_)
            | ServerMessage::TabRegistry(_) => {}
            message => panic!("expected handshake message, got {message:?}"),
        }
    }
    codec
        .write_client_message(
            client,
            &crate::protocol::ClientMessage::TabCommand {
                client_id,
                command: crate::protocol::TabCommand::New {
                    workspace_root: String::new(),
                },
            },
        )
        .await
        .unwrap();
    loop {
        match codec.read_server_message(client).await.unwrap() {
            ServerMessage::InitialDocument { .. } => break,
            ServerMessage::SduiSnapshot { .. }
            | ServerMessage::RuntimeStateSnapshot(_)
            | ServerMessage::TabRegistry(_) => {}
            message => panic!("expected InitialDocument, got {message:?}"),
        }
    }
    loop {
        match codec.read_server_message(client).await.unwrap() {
            ServerMessage::TabRegistry(_) => break,
            ServerMessage::SduiSnapshot { .. } | ServerMessage::RuntimeStateSnapshot(_) => {}
            message => panic!("expected tab registry after bind, got {message:?}"),
        }
    }
}

fn reload_request() -> CommandExecutionRequest {
    CommandExecutionRequest {
        command_id: RELOAD_CONFIGURATION_COMMAND_ID.to_string(),
        arguments: serde_json::Value::Null,
        target: CommandExecutionTarget::Global,
        provenance: None,
        expected_permissions: Vec::new(),
    }
}

#[tokio::test]
async fn concurrent_reload_commands_commit_at_most_one_candidate_at_a_time() {
    let root = temp_config_root("reload-in-progress", "");
    let server = server_with_config(root);
    let active_attempt = server.reload_attempt.lock().await;

    let error = server
        .execute_reload_command(reload_request())
        .await
        .expect_err("concurrent reload must not queue");
    assert_eq!(error.rule, CommandExecutionRule::ReloadInProgress);
    assert_eq!(server.runtime_generation.generation_id().await, 1);

    drop(active_attempt);
    assert!(
        server
            .execute_reload_command(reload_request())
            .await
            .expect("next reload runs after release")
            .reloaded
    );
}

#[tokio::test]
async fn failed_reload_releases_attempt_lock() {
    let root = temp_config_root("reload-lock-release", "const = broken;");
    let server = server_with_config(root.clone());

    assert!(
        !server
            .execute_reload_command(reload_request())
            .await
            .expect("invalid configuration is a completed reload attempt")
            .reloaded
    );
    fs::write(root.join("init.js"), "").unwrap();
    assert!(
        server
            .execute_reload_command(reload_request())
            .await
            .expect("failed attempt released reload locks")
            .reloaded
    );
}

#[tokio::test]
async fn failed_candidate_commit_releases_behavior_lock() {
    let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
        "candidate-lock-release",
    )));
    let candidate = server
        .prepare_runtime_generation_candidate(
            1,
            2,
            super::ClayJsRuntimeService::default(),
            ClayRuntimeEvaluation::default(),
        )
        .await
        .expect("prepare candidate");
    *server.active_theme.lock().await = Some(ActiveTheme {
        specifier: "@clay/conflict".to_string(),
        overrides: Vec::new(),
        design_tokens: Vec::new(),
    });

    assert!(server.commit_runtime_generation(candidate).await.is_err());

    let next = server
        .prepare_runtime_generation_candidate(
            1,
            2,
            super::ClayJsRuntimeService::default(),
            ClayRuntimeEvaluation::default(),
        )
        .await
        .expect("prepare replacement candidate");
    assert!(server.commit_runtime_generation(next).await.is_ok());
}

#[tokio::test]
async fn candidate_validation_failure_changes_no_active_state() {
    let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
        "candidate-validation-rollback",
    )));
    let generation_before = server.runtime_generation.current().await;
    let behavior_before = server.behavior.lock().await.clone();
    let sdui_before = server.sdui.lock().await.clone();
    let theme_before = server.active_theme.lock().await.clone();
    let typography_before = server.runtime_generation.active_typography().await;
    let completion_before = server.completion.providers();
    let intelligence_before = server.language_intelligence.providers();

    let mut evaluation = ClayRuntimeEvaluation::default();
    let mut manifest = BehaviorManifest::minimal_text_editing(99);
    manifest.manifest_id = "test.candidate".to_string();
    evaluation.behavior_manifest = Some(manifest);
    evaluation.published_sdui_tree = Some(default_document_tree(2, 1));

    let result = server
        .prepare_runtime_generation_candidate(
            1,
            2,
            super::ClayJsRuntimeService::default(),
            evaluation,
        )
        .await;

    assert!(result.is_err());
    assert_eq!(
        server.runtime_generation.current().await.id,
        generation_before.id
    );
    assert_eq!(*server.behavior.lock().await, behavior_before);
    assert_eq!(*server.sdui.lock().await, sdui_before);
    assert_eq!(*server.active_theme.lock().await, theme_before);
    assert_eq!(
        server.runtime_generation.active_typography().await,
        typography_before
    );
    assert_eq!(server.completion.providers(), completion_before);
    assert_eq!(
        server.language_intelligence.providers(),
        intelligence_before
    );
}

#[tokio::test]
async fn candidate_commit_advances_all_server_generation_state_once() {
    let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
        "candidate-commit",
    )));
    let mut evaluation = ClayRuntimeEvaluation::default();
    let mut manifest = BehaviorManifest::minimal_text_editing(99);
    manifest.manifest_id = "test.committed".to_string();
    evaluation.behavior_manifest = Some(manifest);
    evaluation.published_sdui_tree = Some(default_document_tree(1, 1));
    evaluation.active_theme = Some(ActiveTheme {
        specifier: "@clay/theme-test".to_string(),
        overrides: Vec::new(),
        design_tokens: Vec::new(),
    });
    evaluation.active_typography = Some(crate::protocol::ActiveTypography {
        revision: 99,
        monospace: FontProfile {
            families: vec!["monospace".to_string()],
            size: 17.0,
            ..FontProfile::default()
        },
        proportional: FontProfile {
            families: vec!["sans-serif".to_string()],
            size: 18.0,
            ..FontProfile::default()
        },
        ui: FontProfile {
            families: vec!["system-ui".to_string()],
            size: 14.0,
            ..FontProfile::default()
        },
        ..crate::protocol::ActiveTypography::default()
    });
    let candidate = server
        .prepare_runtime_generation_candidate(
            1,
            2,
            super::ClayJsRuntimeService::default(),
            evaluation,
        )
        .await
        .expect("valid candidate");

    server
        .commit_runtime_generation(candidate)
        .await
        .expect("candidate commit");

    let current = server.runtime_generation.current().await;
    assert_eq!(current.id, 2);
    assert_eq!(server.behavior.lock().await.version(), 2);
    assert_eq!(
        server.behavior.lock().await.manifest().manifest_id,
        "test.committed"
    );
    assert!(server.sdui.lock().await.snapshot_message(1).is_some());
    assert_eq!(
        server
            .active_theme
            .lock()
            .await
            .as_ref()
            .map(|theme| theme.specifier.as_str()),
        Some("@clay/theme-test")
    );
    assert_eq!(
        server.runtime_generation.active_typography().await.revision,
        1
    );
    assert!(current.evaluation.is_some());
}

#[tokio::test]
async fn example_configuration_loads_cleanly_and_applies_effects() {
    // Plan 086 task 7: whole-workflow bound. A hang here means pending
    // session/runtime cleanup, not a slow machine (measured ~0.06s); the
    // timeout names the failure instead of waiting indefinitely.
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        example_configuration_loads_cleanly_and_applies_effects_scenario(),
    )
    .await
    .expect(
        "example_configuration_loads_cleanly_and_applies_effects exceeded its 5s whole-workflow bound; \
         look for pending session or runtime-replacement cleanup",
    );
}

async fn example_configuration_loads_cleanly_and_applies_effects_scenario() {
    // The examples/ tree (init.js + packages/) is the canonical
    // user-facing configuration. Any edit to it — or to the APIs it
    // calls — must keep it loading through the real runtime-generation
    // path: syntax, package specifiers, language-server contribution
    // ids, grant-before-loadPackage ordering, theme/typography/keybinding
    // effects, and the fault-isolated optional module loads. Language-
    // server grants degrade independently when tooling is absent (see
    // the grantLanguageServer helper in packages/first-party.js), so
    // this test is environment-independent.
    let root = temp_example_config_root("example-configuration");
    let server = server_with_config(root.clone());

    let outcome = server.reload_runtime_generation().await;

    assert!(
        outcome.reloaded,
        "examples tree must load cleanly; diagnostics: {:?}",
        outcome.diagnostics
    );
    assert!(
        !outcome
            .diagnostics
            .iter()
            .any(|d| d.code == "configuration.module_failed"),
        "example optional modules must load; diagnostics: {:?}",
        outcome.diagnostics
    );
    // Observable proof the configuration actually executed: the example's
    // setTypography call replaced the default monospace profile.
    let typography = server.runtime_generation.active_typography().await;
    assert_eq!(typography.monospace.size, 16.0);
    assert_eq!(typography.proportional.size, 17.0);
    assert_eq!(typography.ui.size, 13.0);
    assert_eq!(
        typography.hierarchy,
        crate::protocol::UiTypographyHierarchy::DEFAULT,
        "canonical example hierarchy must preserve the documented defaults"
    );
    assert!(
        typography
            .monospace
            .families
            .iter()
            .any(|family| family == "MartianMono Nerd Font"),
        "example typography families missing: {:?}",
        typography.monospace.families
    );
    assert!(
        typography
            .proportional
            .families
            .iter()
            .any(|family| family == "Noto Sans"),
        "example proportional families missing: {:?}",
        typography.proportional.families
    );
    assert!(
        typography
            .ui
            .families
            .iter()
            .any(|family| family == "system-ui"),
        "example UI families missing: {:?}",
        typography.ui.families
    );
    let active_theme = server
        .active_theme
        .lock()
        .await
        .clone()
        .expect("canonical example must install its explicit theme");
    assert_eq!(active_theme.specifier, "@clay/theme-gruvbox-material-dark");
    // And the first-party package module actually loaded: the markdown
    // package registers a mode (grant-before-loadPackage ordering held).
    let current = server.runtime_generation.current().await;
    let evaluation = current
        .evaluation
        .as_ref()
        .expect("committed generation must retain evaluation snapshot");
    assert!(
        evaluation.behavior_manifest.is_some(),
        "example must publish a behavior manifest from its package loads"
    );
    // The one-line loads inside packages/first-party.js produced the
    // same package outcomes as an equivalent flat init.js: parse
    // handlers, syntax grammars, and completion providers for every
    // loaded grammar package, and nothing third-party.
    assert!(
        evaluation
            .js_parse_handlers
            .iter()
            .any(|handler| handler.package.manifest.name == "@clay/markdown"),
        "markdown parse handler must load from packages/first-party.js"
    );
    for language in ["rust", "typescript", "javascript", "markdown"] {
        assert!(
            evaluation
                .syntax_grammars
                .iter()
                .any(|grammar| grammar.language_id == language),
            "{language} syntax grammar must load from packages/first-party.js"
        );
    }
    assert!(
        evaluation.js_parse_handlers.iter().all(|handler| handler
            .package
            .manifest
            .name
            .starts_with("@clay/")),
        "the commented third-party template must cause zero package activity"
    );
    assert!(
        !outcome
            .diagnostics
            .iter()
            .any(|d| d.code.starts_with("packages.")),
        "no package diagnostics from the example tree; diagnostics: {:?}",
        outcome.diagnostics
    );
    // Plan 118 Part D: the example tree's landing is the launcher package
    // (the chat landing is gone), and the agent keeps its named pane surface —
    // the two never compete for the same activation.
    let landing = evaluation
        .ui_contributions
        .empty_tab()
        .expect("example empty-tab must not conflict")
        .expect("the launcher claims the example tree's empty tab");
    assert_eq!(landing.id, "launcher.start");
    assert_eq!(landing.package_name, "@clay/launcher");
    assert!(
        evaluation
            .ui_contributions
            .pane_contents
            .iter()
            .any(|content| content.id == "coding-agent.surface"),
        "the agent keeps its named pane surface"
    );
    let manifest = evaluation
        .behavior_manifest
        .as_ref()
        .expect("example must publish a behavior manifest");
    let (_, catalogue) = server
        .runtime_generation
        .command_catalogue_snapshot(manifest)
        .await
        .expect("example catalogue must validate");
    assert!(
        catalogue
            .commands()
            .iter()
            .any(|command| command.command_id == "coding-agent.profile"),
        "one-line Coding Agent load must register coding-agent.profile"
    );
    assert!(
        catalogue
            .commands()
            .iter()
            .all(|command| !command.command_id.starts_with("chat.")),
        "the removed chat commands are absent from the catalogue"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn alternate_configuration_layout_loads_identical_packages() {
    // The three-file example layout is a convention, not a requirement:
    // any local module layout under the config root drives the same
    // one-line loadPackage calls. Regression: a nested folder plus a
    // static-import chain must produce identical package outcomes to the
    // shipped examples/ tree.
    let root = temp_config_root(
        "alternate-layout",
        r#"import { loadConfigurationModule } from "clay:configuration";
await loadConfigurationModule({ path: "./a/b.js" });"#,
    );
    fs::create_dir_all(root.join("a")).unwrap();
    // b.js static-imports its sibling c.js (evaluated first), then runs
    // its own one-line load; c.js loads its own package the same way.
    fs::write(
        root.join("a/b.js"),
        r#"import "./c.js";
import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");"#,
    )
    .unwrap();
    fs::write(
        root.join("a/c.js"),
        r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/rust");"#,
    )
    .unwrap();
    let server = server_with_config(root.clone());

    let outcome = server.reload_runtime_generation().await;

    assert!(
        outcome.reloaded,
        "alternate layout must load cleanly; diagnostics: {:?}",
        outcome.diagnostics
    );
    let evaluation = server
        .runtime_generation
        .current()
        .await
        .evaluation
        .expect("committed generation must retain evaluation snapshot");
    assert!(
        evaluation
            .js_parse_handlers
            .iter()
            .any(|handler| handler.package.manifest.name == "@clay/markdown"),
        "markdown parse handler must load from the alternate layout"
    );
    for language in ["rust", "markdown"] {
        assert!(
            evaluation
                .syntax_grammars
                .iter()
                .any(|grammar| grammar.language_id == language),
            "{language} syntax grammar must load from the alternate layout"
        );
    }
    fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn example_configuration_survives_broken_package_module() {
    // The "use Clay to fix Clay" requirement: a broken optional package
    // module must not block the base configuration or reload — it records
    // a bounded configuration.module_failed diagnostic instead.
    let root = temp_example_config_root("example-configuration-broken-module");
    fs::write(root.join("packages/first-party.js"), "export const = ;").unwrap();
    let server = server_with_config(root.clone());

    let outcome = server.reload_runtime_generation().await;

    assert!(
        outcome.reloaded,
        "base config must still reload with a broken optional module; diagnostics: {:?}",
        outcome.diagnostics
    );
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|d| d.code == "configuration.module_failed"),
        "broken optional module must be recorded; diagnostics: {:?}",
        outcome.diagnostics
    );
    // Base-config effects still applied.
    let typography = server.runtime_generation.active_typography().await;
    assert_eq!(typography.monospace.size, 16.0);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn typography_defaults_exist_without_init_configuration() {
    let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
        "typography-defaults",
    )));

    assert_eq!(
        server.runtime_generation.active_typography().await,
        crate::protocol::ActiveTypography::default()
    );
}

#[tokio::test]
async fn typography_update_reaches_connected_clients_once() {
    let root = temp_config_root(
        "typography-live-update",
        r#"import { setTypography } from "clay:theme";
        setTypography({
          monospace: { families: ["monospace"], size: 16 },
          proportional: { families: ["sans-serif"], size: 17 },
          ui: { families: ["system-ui"], size: 13 },
        });"#,
    );
    let server = server_with_config(root.clone());
    let mut updates = server.runtime_generation.subscribe_typography();

    assert!(server.reload_runtime_generation().await.reloaded);
    let update = tokio::time::timeout(std::time::Duration::from_millis(100), updates.recv())
        .await
        .expect("connected client receives typography update")
        .expect("typography channel remains open");
    assert_eq!(update.revision, 1);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), updates.recv())
            .await
            .is_err(),
        "one atomic replacement emits one update"
    );

    fs::write(
        root.join("init.js"),
        r#"import { setTypography } from "clay:theme";
        setTypography({
          monospace: { families: ["monospace"], size: 16 },
          proportional: { families: ["sans-serif"], size: 17 },
        });"#,
    )
    .unwrap();
    assert!(!server.reload_runtime_generation().await.reloaded);
    assert_eq!(
        server.runtime_generation.active_typography().await.revision,
        1
    );
}

#[tokio::test]
async fn reload_runtime_generation_swaps_only_after_successful_configuration_load() {
    let root = temp_config_root(
        "success",
        r#"Deno.core.ops.op_clay_runtime_record("reload ok");"#,
    );
    let server = server_with_config(root);
    let opened_path = temp_config_root("opened-doc", "").join("note.md");
    fs::write(&opened_path, "# kept open\n").unwrap();
    let opened = server
        .workspace
        .lock()
        .await
        .open_selected_file(&opened_path, 77)
        .await
        .unwrap();
    let original_service = server.runtime_generation.current_service().await;
    original_service
        .evaluate_controlled_module("globalThis.__reloadMarker = 41;")
        .await
        .unwrap();

    let outcome = server.reload_runtime_generation().await;

    assert!(outcome.reloaded);
    assert_eq!(outcome.previous_generation_id, 1);
    assert_eq!(outcome.active_generation_id, 2);
    let documents = server
        .workspace
        .lock()
        .await
        .list_documents(77)
        .await
        .unwrap();
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].document_id, opened.document_id);
    assert_eq!(documents[0].lease_id, opened.access.lease_id());
    let current_service = server.runtime_generation.current_service().await;
    let evaluation = current_service
        .evaluate_controlled_module(
            r#"Deno.core.ops.op_clay_runtime_record(String(globalThis.__reloadMarker ?? "empty"));"#,
        )
        .await
        .unwrap();
    assert_eq!(evaluation.op_records.last().unwrap(), "empty");
}

#[tokio::test]
async fn optional_configuration_module_warning_survives_successful_reload() {
    let root = temp_config_root(
        "optional-module-reload",
        r#"
        import { loadConfigurationModule } from "clay:configuration";
        await loadConfigurationModule({ path: "./missing.js", optional: true });
        "#,
    );
    let server = server_with_config(root);

    let outcome = server.reload_runtime_generation().await;

    assert!(outcome.reloaded);
    assert_eq!(outcome.diagnostics.len(), 1);
    assert_eq!(outcome.diagnostics[0].code, "configuration.module_failed");
    assert!(outcome.diagnostics[0].message.contains("./missing.js"));
    assert!(
        server
            .runtime_diagnostics
            .lock()
            .await
            .snapshot()
            .iter()
            .any(|diagnostic| diagnostic.code == "configuration.module_failed")
    );
}

#[tokio::test]
async fn configuration_watcher_reloads_changed_root_without_command_intent() {
    let root = temp_config_root(
        "watcher-reload",
        r#"Deno.core.ops.op_clay_runtime_record("initial");"#,
    );
    let server = server_with_config(root.clone());
    assert!(server.reload_runtime_generation().await.reloaded);

    let watcher_server = server.clone();
    let watcher = tokio::spawn(
        super::config_watch::watch_configuration_root_with_intervals(
            root.clone(),
            move || {
                let watcher_server = watcher_server.clone();
                async move {
                    let _ = watcher_server.reload_runtime_generation().await;
                }
            },
            std::time::Duration::from_millis(10),
            std::time::Duration::from_millis(20),
        ),
    );
    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    fs::write(
        root.join("init.js"),
        r#"Deno.core.ops.op_clay_runtime_record("reloaded");"#,
    )
    .unwrap();

    wait_until(
        &server,
        "watcher reloads changed configuration",
        std::time::Duration::from_secs(5),
        || async { server.runtime_generation.generation_id().await >= 3 },
    )
    .await;
    watcher.abort();

    let current = server.runtime_generation.current().await;
    assert!(
        current
            .evaluation
            .as_ref()
            .is_some_and(|evaluation| evaluation.op_records == ["reloaded"])
    );
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn configuration_watcher_preserves_generation_on_failure_and_recovers() {
    let root = temp_config_root(
        "watcher-recovery",
        r#"Deno.core.ops.op_clay_runtime_record("working");"#,
    );
    let server = server_with_config(root.clone());
    assert!(server.reload_runtime_generation().await.reloaded);

    let watcher_server = server.clone();
    let watcher = tokio::spawn(
        super::config_watch::watch_configuration_root_with_intervals(
            root.clone(),
            move || {
                let watcher_server = watcher_server.clone();
                async move {
                    let _ = watcher_server.reload_runtime_generation().await;
                }
            },
            std::time::Duration::from_millis(10),
            std::time::Duration::from_millis(20),
        ),
    );
    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    fs::write(root.join("init.js"), "export const = ;").unwrap();

    wait_until(
        &server,
        "watcher records failed reload diagnostic",
        std::time::Duration::from_secs(5),
        || async {
            server
                .runtime_diagnostics
                .lock()
                .await
                .snapshot()
                .iter()
                .any(|diagnostic| diagnostic.code == "runtime.syntax_error")
        },
    )
    .await;
    assert_eq!(server.runtime_generation.generation_id().await, 2);

    fs::write(
        root.join("init.js"),
        r#"Deno.core.ops.op_clay_runtime_record("recovered");"#,
    )
    .unwrap();
    wait_until(
        &server,
        "watcher reloads after fixing configuration",
        std::time::Duration::from_secs(5),
        || async { server.runtime_generation.generation_id().await >= 3 },
    )
    .await;
    watcher.abort();
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn configuration_watcher_detects_new_optional_module() {
    let root = temp_config_root(
        "watcher-new-module",
        r#"
        import { loadConfigurationModule } from "clay:configuration";
        await loadConfigurationModule({ path: "./new.js", optional: true });
        "#,
    );
    let server = server_with_config(root.clone());
    assert!(server.reload_runtime_generation().await.reloaded);

    let watcher_server = server.clone();
    let watcher = tokio::spawn(
        super::config_watch::watch_configuration_root_with_intervals(
            root.clone(),
            move || {
                let watcher_server = watcher_server.clone();
                async move {
                    let _ = watcher_server.reload_runtime_generation().await;
                }
            },
            std::time::Duration::from_millis(10),
            std::time::Duration::from_millis(20),
        ),
    );
    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
    fs::write(
        root.join("new.js"),
        r#"Deno.core.ops.op_clay_runtime_record("new module");"#,
    )
    .unwrap();

    wait_until(
        &server,
        "watcher reloads a newly created module",
        std::time::Duration::from_secs(5),
        || async {
            let current = server.runtime_generation.current().await;
            current.id >= 3
                && current.evaluation.as_ref().is_some_and(|evaluation| {
                    evaluation
                        .op_records
                        .iter()
                        .any(|record| record == "new module")
                })
        },
    )
    .await;
    watcher.abort();
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[should_panic(expected = "deliberately pending scenario")]
async fn wait_until_panics_with_scenario_and_server_state_on_timeout() {
    let root = temp_config_root("wait-until-timeout", "");
    let server = server_with_config(root.clone());
    assert!(server.reload_runtime_generation().await.reloaded);

    // A deliberately pending condition must time out with the scenario
    // name and live server state (generation id, diagnostics) in the
    // panic, never a bare Elapsed.
    wait_until(
        &server,
        "deliberately pending scenario",
        std::time::Duration::from_millis(50),
        || async { false },
    )
    .await;

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn tab_command_bindings_work_and_survive_configuration_reload() {
    let root = temp_config_root(
        "tab-bindings",
        r#"
        import { clientTabNew } from "clay:shell";
        import { bindKey } from "clay:keybindings";
        bindKey("Ctrl+Alt+T", clientTabNew(), { scope: "global" });
        "#,
    );
    let server = server_with_config(root);
    let first = server.reload_runtime_generation().await;
    assert!(first.reloaded);

    async fn bound_tab_records(server: &IpcServer) -> String {
        let service = server.runtime_generation.current_service().await;
        let evaluation = service
            .evaluate_controlled_module(
                r#"
                import { listKeyBindings } from "clay:keybindings";
                const matches = listKeyBindings("global")
                    .filter((candidate) => candidate.command === "shell.clientTabNew")
                    .map((candidate) => `${candidate.key}:${candidate.command}`)
                    .join(",");
                Deno.core.ops.op_clay_runtime_record(matches);
                "#,
            )
            .await
            .unwrap();
        evaluation.op_records.last().unwrap().clone()
    }

    assert!(
        bound_tab_records(&server)
            .await
            .contains("Ctrl+Alt+T:shell.clientTabNew"),
        "init.js tab binding must be live after configuration load"
    );

    let second = server.reload_runtime_generation().await;
    assert!(second.reloaded);
    assert!(
        bound_tab_records(&server)
            .await
            .contains("Ctrl+Alt+T:shell.clientTabNew"),
        "init.js tab binding must survive a configuration reload"
    );
}

#[tokio::test]
async fn successful_reload_refreshes_open_documents_without_full_snapshots() {
    let root = temp_config_root(
        "open-refresh",
        r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");"#,
    );
    let server = server_with_config(root);
    let file_root = temp_config_root("open-refresh-docs", "");
    let markdown_path = file_root.join("note.md");
    let text_path = file_root.join("plain.txt");
    fs::write(&markdown_path, "# Reloaded\n").unwrap();
    fs::write(&text_path, "plain text\n").unwrap();
    let markdown = server
        .workspace
        .lock()
        .await
        .open_selected_file(&markdown_path, 77)
        .await
        .unwrap();
    let text = server
        .workspace
        .lock()
        .await
        .open_selected_file(&text_path, 77)
        .await
        .unwrap();

    let outcome = server.reload_runtime_generation().await;

    assert!(outcome.reloaded);
    assert_eq!(outcome.refreshed_documents.len(), 2);
    let markdown_refresh = outcome
        .refreshed_documents
        .iter()
        .find(|refresh| refresh.document_id == markdown.document_id)
        .unwrap();
    assert!(markdown_refresh.messages.iter().any(|message| matches!(
        message,
        ServerMessage::BehaviorManifest(manifest)
            if manifest.manifest_id == "markdown.markdown"
                && matches!(manifest.scope, crate::protocol::BehaviorScope::Document { document_id } if document_id == markdown.document_id)
    )));
    assert!(
        markdown_refresh
            .messages
            .iter()
            .all(|message| !matches!(message, ServerMessage::DecorationSet(_))),
        "reload refresh should not block on background parse decorations"
    );
    assert!(
        outcome
            .refreshed_documents
            .iter()
            .all(|refresh| refresh.messages.iter().all(|message| !matches!(
                message,
                ServerMessage::DocumentOpened { .. } | ServerMessage::DocumentReloaded { .. }
            )))
    );
    let text_refresh = outcome
        .refreshed_documents
        .iter()
        .find(|refresh| refresh.document_id == text.document_id)
        .unwrap();
    assert!(
        text_refresh
            .messages
            .iter()
            .all(|message| !matches!(message, ServerMessage::DecorationSet(_)))
    );
}

#[tokio::test]
async fn runtime_reload_refreshes_large_document_without_full_text() {
    use std::io::Write;

    let root = temp_config_root(
        "large-refresh",
        r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");"#,
    );
    let server = server_with_config(root);
    let file_root = temp_config_root("large-refresh-docs", "");
    let large_path = file_root.join("large.md");
    {
        let mut file = fs::File::create(&large_path).unwrap();
        file.write_all(b"# Large\n").unwrap();
        let chunk = vec![b'x'; 1024 * 1024];
        for _ in 0..50 {
            file.write_all(&chunk).unwrap();
        }
    }
    let opened = {
        let mut workspace = server.workspace.lock().await;
        let root_id = workspace.add_root(&file_root).unwrap();
        workspace
            .open_existing_file(root_id, "large.md", 77)
            .await
            .unwrap()
    };
    assert_eq!(
        opened.document.lock().await.byte_len(),
        8 + 50 * 1024 * 1024
    );

    let outcome = server.reload_runtime_generation().await;
    assert!(outcome.reloaded);
    let refresh = outcome
        .refreshed_documents
        .iter()
        .find(|refresh| refresh.document_id == opened.document_id)
        .expect("large document is refreshed");
    assert!(refresh.messages.iter().all(|message| !matches!(
        message,
        ServerMessage::DocumentOpened { .. } | ServerMessage::DocumentReloaded { .. }
    )));
    assert!(refresh.messages.iter().any(|message| matches!(
        message,
        ServerMessage::RuntimeDiagnostic(diagnostic)
            if diagnostic.code == "analysis.document_too_large"
    )));
    assert!(
        refresh
            .messages
            .iter()
            .all(|message| format!("{message:?}").len() < 64 * 1024),
        "refresh must not carry a full 50 MiB string"
    );

    let _ = fs::remove_file(large_path);
    let _ = fs::remove_dir_all(file_root);
}

#[tokio::test]
async fn reload_reruns_init_js_package_load_in_fresh_generation_and_preserves_old_on_failure() {
    let root = temp_config_root(
        "package-cache",
        r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/markdown");
await loadPackage("@clay/markdown");"#,
    );
    let server = server_with_config(root.clone());

    let loaded = server.reload_runtime_generation().await;
    assert!(loaded.reloaded);
    assert_eq!(loaded.active_generation_id, 2);
    let current_service = server.runtime_generation.current_service().await;
    let cached = current_service
        .evaluate_controlled_module(
            r#"import { loadPackage } from "clay:packages";
Deno.core.ops.op_clay_runtime_record(String(Boolean(globalThis.__clayLoadedPackages?.["@clay/markdown"])));
await loadPackage("@clay/markdown");
Deno.core.ops.op_clay_runtime_record("cached");"#,
        )
        .await
        .unwrap();
    assert!(cached.op_records.iter().any(|record| record == "true"));
    assert_eq!(cached.op_records.last().unwrap(), "cached");

    fs::write(
        root.join("init.js"),
        r#"import { loadPackage } from "clay:packages";
await loadPackage("@clay/not-installed");"#,
    )
    .unwrap();
    let failed = server.reload_runtime_generation().await;
    assert!(!failed.reloaded);
    assert_eq!(failed.active_generation_id, 2);
    let still_cached = server
        .runtime_generation
        .current_service()
        .await
        .evaluate_controlled_module(
            r#"import { loadPackage } from "clay:packages";
Deno.core.ops.op_clay_runtime_record(String(Boolean(globalThis.__clayLoadedPackages?.["@clay/markdown"])));
await loadPackage("@clay/markdown");
Deno.core.ops.op_clay_runtime_record("still-cached");"#,
        )
        .await
        .unwrap();
    assert!(
        still_cached
            .op_records
            .iter()
            .any(|record| record == "true")
    );
    assert_eq!(still_cached.op_records.last().unwrap(), "still-cached");
    assert!(
        server
            .runtime_diagnostics
            .lock()
            .await
            .snapshot()
            .iter()
            .any(|diagnostic| diagnostic.code == "packages.not_installed")
    );
}

#[tokio::test]
async fn reload_with_missing_design_system_preserves_previous_generation_and_reports_diagnostic() {
    let root = temp_config_root(
        "design-system-reload-preserve",
        r#"import { setDesignSystem } from "clay:theme";
setDesignSystem("@clay/core");"#,
    );
    let server = server_with_config(root.clone());

    let loaded = server.reload_runtime_generation().await;
    assert!(loaded.reloaded);
    let baseline = server.active_design_system.lock().await.clone();
    assert_eq!(baseline.specifier, "@clay/core");

    fs::write(
        root.join("init.js"),
        r#"import { setDesignSystem } from "clay:theme";
setDesignSystem("@vendor/never-installed-ds");"#,
    )
    .unwrap();
    let failed = server.reload_runtime_generation().await;
    assert!(!failed.reloaded);
    assert_eq!(failed.active_generation_id, loaded.active_generation_id);
    let retained = server.active_design_system.lock().await.clone();
    assert_eq!(retained, baseline);
    assert!(
        server
            .runtime_diagnostics
            .lock()
            .await
            .snapshot()
            .iter()
            .any(|diagnostic| diagnostic.code == "theme.load_failed")
    );
}

#[tokio::test]
async fn reload_reruns_one_line_loads_and_rebuilds_representative_contributions() {
    let root = temp_config_root(
        "one-line-rebuild",
        r#"import { loadPackage } from "clay:packages";
import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
await loadPackage("@clay/markdown");
await loadPackage("@clay/rust");
await loadPackage("@clay/typescript");
await loadPackage("@clay/javascript");
const classification = serverClassifyDocument({ documentId: 1, path: "README.md" });
serverActivateClassifiedMode(classification, { path: "README.md" });"#,
    );
    let server = server_with_config(root);

    let outcome = server.reload_runtime_generation().await;
    assert!(outcome.reloaded);
    assert_eq!(outcome.active_generation_id, 2);

    let evaluation = server
        .runtime_generation
        .current()
        .await
        .evaluation
        .expect("committed generation must retain evaluation snapshot");

    assert!(
        evaluation
            .js_parse_handlers
            .iter()
            .any(|handler| handler.package.manifest.name == "@clay/markdown"),
        "markdown parse handler must rebuild in G2"
    );
    assert!(
        evaluation.behavior_manifest.is_some(),
        "language/command behavior contributions must rebuild in G2"
    );
    for language in ["rust", "typescript", "javascript", "markdown"] {
        assert!(
            evaluation
                .syntax_grammars
                .iter()
                .any(|grammar| grammar.language_id == language),
            "{language} syntax grammar must rebuild in G2"
        );
    }
    for provider_id in [
        "markdown.keywords",
        "rust.keywords",
        "typescript.keywords",
        "javascript.keywords",
    ] {
        assert!(
            evaluation
                .completion_providers
                .iter()
                .any(|provider| provider.id == provider_id),
            "{provider_id} completion metadata must rebuild in G2"
        );
    }
    assert!(
        !evaluation.ui_contributions.components.is_empty()
            || !evaluation.ui_contributions.panels.is_empty(),
        "package UI contributions must rebuild in G2"
    );

    let cached = server
        .runtime_generation
        .current_service()
        .await
        .evaluate_controlled_module(
            r#"import { loadPackage } from "clay:packages";
Deno.core.ops.op_clay_runtime_record(String(Boolean(globalThis.__clayLoadedPackages?.["@clay/markdown"])));
Deno.core.ops.op_clay_runtime_record(String(Boolean(globalThis.__clayLoadedPackages?.["@clay/rust"])));
await loadPackage("@clay/markdown");
await loadPackage("@clay/rust");
Deno.core.ops.op_clay_runtime_record("idempotent");"#,
        )
        .await
        .unwrap();
    assert!(cached.op_records.iter().any(|record| record == "true"));
    assert_eq!(cached.op_records.last().unwrap(), "idempotent");
}

#[tokio::test]
async fn runtime_timeout_drops_candidate_service_and_keeps_old_generation() {
    let root = temp_config_root("candidate-timeout", "while (true) {}");
    let server = server_with_config(root);
    let original = server.runtime_generation.current().await;
    let candidate_service = super::ClayJsRuntimeService::with_timeout_and_heap_limit(
        Duration::from_millis(10),
        crate::perf::budgets::JS_RUNTIME_HEAP_LIMIT_BYTES,
    );

    let error = server
        .load_configuration_for_service(&candidate_service)
        .await
        .expect_err("candidate evaluation must time out");

    assert!(matches!(
        error,
        super::js_runtime::ClayRuntimeError::Timeout
    ));
    assert_eq!(server.runtime_generation.current().await.id, original.id);
    assert!(
        server
            .runtime_generation
            .current()
            .await
            .evaluation
            .is_none()
    );
}

#[tokio::test]
async fn failed_reload_keeps_previous_runtime_generation_active() {
    let root = temp_config_root("failure", "export const = ;");
    let server = server_with_config(root);
    let original_service = server.runtime_generation.current_service().await;
    original_service
        .evaluate_controlled_module("globalThis.__reloadMarker = 7;")
        .await
        .unwrap();

    let outcome = server.reload_runtime_generation().await;

    assert!(!outcome.reloaded);
    assert_eq!(outcome.previous_generation_id, 1);
    assert_eq!(outcome.active_generation_id, 1);
    let current_service = server.runtime_generation.current_service().await;
    let evaluation = current_service
        .evaluate_controlled_module(
            r#"Deno.core.ops.op_clay_runtime_record(String(globalThis.__reloadMarker));"#,
        )
        .await
        .unwrap();
    assert_eq!(evaluation.op_records.last().unwrap(), "7");
    assert!(
        server
            .runtime_diagnostics
            .lock()
            .await
            .snapshot()
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime.syntax_error")
    );
}

fn seed_package() -> crate::packages::record::PackageRecord {
    assemble_package_record(&serde_json::json!({
        "name": "@clay/markdown",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "markdown",
            "entry": "./dist/index.js",
            "permissions": ["parse-document", "completion-provider"],
            "modes": ["markdown"],
            "docs": "./docs/index.md"
        }
    }))
    .expect("seed package validates")
}

fn seed_old_generation_contributions(server: &IpcServer) {
    let package = seed_package();
    server
        .parse_coordinator
        .register_handler_for_generation(
            &package,
            1,
            "markdown",
            |_notification: ParseEditNotification| async move {
                Ok(IncrementalParseUpdate {
                    document_id: 7,
                    document_version: 1,
                    behavior_version: 1,
                    package_prefix: "markdown".to_string(),
                    mode_id: "markdown".to_string(),
                    parse_unit: crate::protocol::ParseUnit::File,
                    viewport: ParseByteRange::new(0, 1),
                    invalidated_ranges: Vec::new(),
                    syntax_tree_delta: None,
                    decoration_updates: Vec::new(),
                    diagnostic_update: None,
                    folding_update: None,
                    trace_id: None,
                    request_id: None,
                    client_id: None,
                })
            },
        )
        .expect("seed parse handler");
    server
        .completion
        .register_builtin_buffer_words(1)
        .expect("seed completion");
    server
        .language_intelligence
        .register_builtin(
            LanguageIntelligenceProviderMeta::builtin_core(
                "hover",
                vec![crate::protocol::LanguageIntelligenceFeature::Hover],
                1,
                500,
                1,
            ),
            |_request, _window| async move {
                Err(
                    crate::server::language_intelligence::LanguageIntelligenceProviderError::ProviderFailed(
                        "seed".to_string(),
                    ),
                )
            },
        )
        .expect("seed language intelligence");
}

#[tokio::test]
async fn successful_reload_replaces_all_provider_registries_and_cancels_old_work() {
    let root = temp_config_root(
        "replace-all",
        r#"Deno.core.ops.op_clay_runtime_record("generation-two");"#,
    );
    let server = server_with_config(root);
    seed_old_generation_contributions(&server);
    assert_eq!(server.parse_coordinator.registered_generations(), vec![1]);
    assert_eq!(server.completion.registered_generations(), vec![1]);
    assert_eq!(
        server.language_intelligence.registered_generations(),
        vec![1]
    );

    let outcome = server.reload_runtime_generation().await;

    assert!(outcome.reloaded);
    assert_eq!(outcome.active_generation_id, 2);
    assert!(
        server
            .parse_coordinator
            .registered_generations()
            .iter()
            .all(|&generation| generation >= 2)
    );
    assert!(
        server
            .completion
            .registered_generations()
            .iter()
            .all(|&generation| generation >= 2)
    );
    assert!(
        server
            .language_intelligence
            .registered_generations()
            .iter()
            .all(|&generation| generation >= 2)
    );
    assert!(
        server.document_analysis.registered_generations().is_empty()
            || server
                .document_analysis
                .registered_generations()
                .iter()
                .all(|&generation| generation >= 2)
    );
    assert!(
        server.document_analysis.worker_generations().is_empty()
            || server
                .document_analysis
                .worker_generations()
                .iter()
                .all(|&generation| generation >= 2)
    );
}

#[tokio::test]
async fn failed_reload_keeps_workers_sessions_and_outputs_on_previous_generation() {
    let root = temp_config_root("keep-old", "export const = ;");
    let server = server_with_config(root);
    seed_old_generation_contributions(&server);
    let sessions_before = server
        .runtime_generation
        .current_service()
        .await
        .language_server_session_count()
        .await;

    let outcome = server.reload_runtime_generation().await;

    assert!(!outcome.reloaded);
    assert_eq!(outcome.active_generation_id, 1);
    assert_eq!(server.parse_coordinator.registered_generations(), vec![1]);
    assert_eq!(server.completion.registered_generations(), vec![1]);
    assert_eq!(
        server.language_intelligence.registered_generations(),
        vec![1]
    );
    assert_eq!(
        server
            .runtime_generation
            .current_service()
            .await
            .language_server_session_count()
            .await,
        sessions_before
    );
}

#[tokio::test]
async fn late_old_generation_parse_completion_diagnostic_and_intelligence_output_is_dropped() {
    let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
        "stale-output-drop",
    )));
    let package = seed_package();
    server
        .parse_coordinator
        .register_handler_for_generation(
            &package,
            1,
            "markdown",
            |notification: ParseEditNotification| async move {
                tokio::time::sleep(Duration::from_millis(150)).await;
                Ok(IncrementalParseUpdate {
                    document_id: notification.document_id,
                    document_version: notification.document_version,
                    behavior_version: notification.behavior_version,
                    package_prefix: notification.package_prefix,
                    mode_id: notification.mode_id,
                    parse_unit: crate::protocol::ParseUnit::File,
                    viewport: notification.viewport,
                    invalidated_ranges: notification.invalidated_ranges,
                    syntax_tree_delta: None,
                    decoration_updates: Vec::new(),
                    diagnostic_update: None,
                    folding_update: None,
                    trace_id: notification.trace_id,
                    request_id: notification.request_id,
                    client_id: None,
                })
            },
        )
        .unwrap();
    server
        .completion
        .register_builtin(
            BufferWordCompletionProvider::meta(1),
            BufferWordCompletionProvider,
        )
        .unwrap();

    server
        .parse_coordinator
        .schedule_parse(ParseScheduleRequest {
            document_id: 7,
            document_version: 1,
            behavior_version: 1,
            package_prefix: "markdown".to_string(),
            mode_id: "markdown".to_string(),
            viewport: ParseByteRange::new(0, 8),
            invalidated_ranges: vec![ParseByteRange::new(0, 8)],
            accepted_edit: None,
            trace_id: None,
            request_id: None,
            client_id: None,
        })
        .unwrap();
    server.parse_coordinator.cancel_older_generations(2);
    server.completion.cancel_older_generations(2);
    server.language_intelligence.cancel_older_generations(2);
    server.document_analysis.cancel_older_generations(2);

    assert!(
        tokio::time::timeout(
            Duration::from_millis(300),
            server.parse_coordinator.next_update()
        )
        .await
        .is_err(),
        "late old-generation parse output must be drained/dropped"
    );
    assert!(server.parse_coordinator.registered_generations().is_empty());
    assert!(server.completion.registered_generations().is_empty());
    assert_eq!(server.completion.stats().stale_results_rejected, 0);
}

#[tokio::test]
async fn removed_language_package_reclassifies_to_core_fallback() {
    use crate::packages::modes::{DocumentClassificationInput, ModeDeclaration, ModeRegistry};

    let package = assemble_package_record(&serde_json::json!({
        "name": "@clay/markdown",
        "version": "0.1.0",
        "type": "module",
        "exports": { ".": "./dist/index.js" },
        "clay": {
            "apiPrefix": "markdown",
            "entry": "./dist/index.js",
            "permissions": ["mode-registration", "mode-activation"],
            "modes": ["markdown"],
            "docs": "./docs/index.md"
        }
    }))
    .expect("mode package validates");
    let mut registry = ModeRegistry::new();
    registry
        .register_mode(
            &package.manifest,
            ModeDeclaration {
                package_name: package.manifest.name.clone(),
                package_version: package.manifest.version.clone(),
                api_prefix: package.manifest.clay.api_prefix.clone(),
                mode_id: "markdown".to_string(),
                display_name: "Markdown".to_string(),
                document_font_role: crate::protocol::DocumentFontRole::Proportional,
                extensions: vec!["md".to_string()],
                mime_types: Vec::new(),
                file_names: Vec::new(),
                file_name_patterns: Vec::new(),
                shebang_patterns: Vec::new(),
                content_probes: Vec::new(),
            },
        )
        .expect("register markdown mode");
    let classified = registry
        .classify(&DocumentClassificationInput {
            document_id: 7,
            path: Some("note.md".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("markdown claims .md");
    assert_eq!(classified.mode_id, "markdown");
    assert_eq!(registry.unregister_package_modes("markdown"), 1);

    let fallback = registry
        .classify(&DocumentClassificationInput {
            document_id: 7,
            path: Some("note.md".to_string()),
            mime_type: None,
            shebang: None,
            leading_content: None,
        })
        .expect("core fallback remains after package withdrawal");
    assert_eq!(fallback.mode_id, "core.text");
    assert_eq!(fallback.api_prefix, "core");
}

#[tokio::test]
async fn withdraw_package_contributions_reuses_generation_cancel_primitives() {
    let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
        "withdraw-package",
    )));
    seed_old_generation_contributions(&server);
    withdraw_package_contributions(
        "@clay/markdown",
        "markdown",
        &server.parse_coordinator,
        &server.completion,
        &server.document_analysis,
        &server.language_intelligence,
    );
    assert!(server.parse_coordinator.registered_generations().is_empty());
    // Built-in buffer-word completion uses clay.core provenance, so package
    // withdraw leaves it; language-intelligence seed uses clay.core too.
    assert!(
        server
            .completion
            .providers()
            .iter()
            .all(|meta| meta.provenance.package_prefix != "markdown")
    );

    let mut commands = crate::packages::commands::CommandRegistry::new();
    commands.insert_test_command(crate::packages::commands::RegisteredCommand {
        package_name: "@clay/markdown".to_string(),
        package_version: "0.1.0".to_string(),
        api_prefix: "markdown".to_string(),
        command_id: "markdown.togglePreview".to_string(),
        display_name: "Toggle Preview".to_string(),
        routing_policy: crate::protocol::RoutingPolicy::ServerFirst,
        key_bindings: Vec::new(),
        custom_properties: Default::default(),
        permissions: Vec::new(),
    });
    assert_eq!(commands.remove_package_commands("@clay/markdown"), 1);
    assert!(commands.list().next().is_none());
}

fn sample_runtime_snapshot(generation: u64) -> crate::protocol::RuntimeStateSnapshot {
    let snapshot = crate::protocol::RuntimeStateSnapshot {
        runtime_generation_id: generation,
        client_id: 0,
        behavior: BehaviorManifest::minimal_text_editing(generation),
        active_theme: ActiveTheme {
            specifier: "@clay/default".to_string(),
            overrides: Vec::new(),
            design_tokens: Vec::new(),
        },
        active_typography: crate::protocol::ActiveTypography::default(),
        active_design_system: crate::shell::design_system::ActiveDesignSystem::core_fallback(
            generation,
        ),
        active_icon_pack: None,
        sdui_tree: default_document_tree(1, 1),
        package_ui: crate::protocol::PackageUiSnapshot {
            version: generation,
            ..Default::default()
        },
        documents: Vec::new(),
        diagnostics: Vec::new(),
        ui_choices: crate::protocol::UiChoicesSnapshot::default(),
    };
    snapshot.validate().expect("sample snapshot");
    snapshot
}

#[tokio::test]
async fn successful_reload_publishes_runtime_state_snapshot_to_subscribers() {
    let root = temp_config_root(
        "snapshot-fanout",
        r#"Deno.core.ops.op_clay_runtime_record("fanout");"#,
    );
    let server = server_with_config(root.clone());
    let mut updates = server.runtime_generation.subscribe_runtime_state();

    let outcome = server.reload_runtime_generation().await;
    assert!(outcome.reloaded);
    assert_eq!(outcome.active_generation_id, 2);

    let generation = tokio::time::timeout(Duration::from_millis(200), updates.recv())
        .await
        .expect("subscriber receives generation notice")
        .expect("runtime-state channel remains open");
    assert_eq!(generation, 2);
    let snapshot = server
        .runtime_generation
        .latest_runtime_snapshot_for(42)
        .await
        .expect("latest snapshot retained after commit");
    assert_eq!(snapshot.runtime_generation_id, 2);
    assert_eq!(snapshot.client_id, 42);
    assert!(snapshot.package_ui.version >= 2);
    snapshot.validate().expect("published snapshot validates");

    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn lagged_connection_receives_latest_snapshot_not_intermediate_generations() {
    let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
        "lagged-runtime-snapshot",
    )));
    let mut updates = server.runtime_generation.subscribe_runtime_state();

    for generation in 1..=20 {
        server
            .runtime_generation
            .publish_runtime_snapshot(sample_runtime_snapshot(generation))
            .await;
    }

    match updates.recv().await {
        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
        Ok(generation) => {
            // Capacity may still deliver the newest notice without lagging
            // when the runtime coalesces; either way recovery uses latest.
            assert_eq!(generation, 20);
        }
        Err(other) => panic!("unexpected broadcast error: {other:?}"),
    }

    let latest = server
        .runtime_generation
        .latest_runtime_snapshot_for(9)
        .await
        .expect("latest complete snapshot");
    assert_eq!(latest.runtime_generation_id, 20);
    assert_eq!(latest.client_id, 9);
    assert_ne!(latest.runtime_generation_id, 19);
}

#[tokio::test]
async fn spoofed_or_future_install_ack_is_ignored() {
    let server = IpcServer::new(ServerConfig::new(IpcEndpoint::from_argument(
        "spoofed-runtime-ack",
    )));
    server
        .runtime_generation
        .publish_runtime_snapshot(sample_runtime_snapshot(3))
        .await;

    assert!(
        !server
            .runtime_generation
            .note_runtime_generation_installed(99, 7, 3)
            .await,
        "spoofed client id must be ignored"
    );
    assert!(
        !server
            .runtime_generation
            .note_runtime_generation_installed(7, 7, 4)
            .await,
        "future generation must be ignored"
    );
    assert!(
        !server
            .runtime_generation
            .note_runtime_generation_installed(7, 7, 0)
            .await,
        "zero generation must be ignored"
    );
    assert!(
        server
            .runtime_generation
            .note_runtime_generation_installed(7, 7, 3)
            .await
    );
    assert_eq!(
        server
            .runtime_generation
            .acknowledged_runtime_generation(7)
            .await,
        Some(3)
    );
}

#[tokio::test]
async fn successful_reload_reaches_two_connected_clients() {
    use std::sync::Arc;
    use tokio::io::duplex;

    let root = temp_config_root(
        "two-clients",
        r#"Deno.core.ops.op_clay_runtime_record("two clients");"#,
    );
    let server = server_with_config(root.clone());
    let codec = crate::protocol::codec::Codec::default();

    async fn bootstrap_client(
        server: &IpcServer,
        client_id: u64,
        codec: crate::protocol::codec::Codec,
    ) -> (
        tokio::io::DuplexStream,
        tokio::task::JoinHandle<Result<(), crate::protocol::codec::CodecError>>,
    ) {
        let (client, server_stream) = duplex(64 * 1024);
        let connection_server = server.clone();
        let tab_registry = Arc::clone(&connection_server.tab_registry);
        let tab_registry_tx = connection_server.tab_registry_tx.clone();
        let handle = tokio::spawn(async move {
            crate::server::connection::handle_connection_with_analysis(
                server_stream,
                client_id,
                Arc::clone(&connection_server.document),
                Arc::clone(&connection_server.behavior),
                Arc::clone(&connection_server.workspace),
                Arc::clone(&connection_server.sdui),
                Arc::clone(&connection_server.active_theme),
                Arc::clone(&connection_server.runtime_diagnostics),
                connection_server.runtime_generation.clone(),
                connection_server.parse_coordinator.clone(),
                connection_server.completion.clone(),
                connection_server.document_analysis.clone(),
                connection_server.language_intelligence.clone(),
                Some(connection_server),
                tab_registry,
                tab_registry_tx,
                codec,
            )
            .await
        });
        let mut client = client;
        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::Hello {
                    protocol_version: crate::protocol::PROTOCOL_VERSION,
                    client_name: format!("client-{client_id}"),
                },
            )
            .await
            .unwrap();
        loop {
            match codec.read_server_message(&mut client).await.unwrap() {
                ServerMessage::Welcome {
                    client_id: welcome_id,
                    ..
                } => {
                    assert_eq!(welcome_id, client_id);
                    break;
                }
                ServerMessage::Error { code, message } => {
                    panic!("bootstrap failed: {code:?} {message}");
                }
                _ => {}
            }
        }
        // Drain remaining bootstrap messages so reload fan-out is next.
        for _ in 0..16 {
            match tokio::time::timeout(
                Duration::from_millis(10),
                codec.read_server_message(&mut client),
            )
            .await
            {
                Ok(Ok(_)) => {}
                Ok(Err(_)) | Err(_) => break,
            }
        }
        (client, handle)
    }

    let (mut client_a, task_a) = bootstrap_client(&server, 21, codec).await;
    let (mut client_b, task_b) = bootstrap_client(&server, 22, codec).await;

    assert!(server.reload_runtime_generation().await.reloaded);

    async fn read_snapshot(
        codec: &crate::protocol::codec::Codec,
        client: &mut tokio::io::DuplexStream,
        expected_client_id: u64,
    ) -> crate::protocol::RuntimeStateSnapshot {
        loop {
            match tokio::time::timeout(
                Duration::from_millis(500),
                codec.read_server_message(client),
            )
            .await
            .expect("client receives fan-out")
            .unwrap()
            {
                ServerMessage::RuntimeStateSnapshot(snapshot) => {
                    assert_eq!(snapshot.client_id, expected_client_id);
                    assert_eq!(snapshot.runtime_generation_id, 2);
                    return *snapshot;
                }
                ServerMessage::ActiveTypography(_)
                | ServerMessage::BehaviorManifest(_)
                | ServerMessage::DecorationSet(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::SduiSnapshot { .. }
                | ServerMessage::SduiUpdate { .. } => {}
                other => panic!("unexpected fan-out message: {other:?}"),
            }
        }
    }

    let snapshot_a = read_snapshot(&codec, &mut client_a, 21).await;
    let snapshot_b = read_snapshot(&codec, &mut client_b, 22).await;
    assert_eq!(
        snapshot_a.runtime_generation_id,
        snapshot_b.runtime_generation_id
    );

    codec
        .write_client_message(
            &mut client_a,
            &crate::protocol::ClientMessage::RuntimeGenerationInstalled {
                client_id: 21,
                runtime_generation_id: 2,
            },
        )
        .await
        .unwrap();
    codec
        .write_client_message(
            &mut client_b,
            &crate::protocol::ClientMessage::RuntimeGenerationInstalled {
                client_id: 22,
                runtime_generation_id: 2,
            },
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        server
            .runtime_generation
            .acknowledged_runtime_generation(21)
            .await,
        Some(2)
    );
    assert_eq!(
        server
            .runtime_generation
            .acknowledged_runtime_generation(22)
            .await,
        Some(2)
    );

    drop(client_a);
    drop(client_b);
    let _ = task_a.await;
    let _ = task_b.await;
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn edit_sent_before_snapshot_install_is_accepted_once_under_previous_generation() {
    use std::sync::Arc;
    use tokio::io::duplex;

    let root = temp_config_root(
        "grace-accept",
        r#"
        import { bindKey } from "clay:keybindings";
        bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
        Deno.core.ops.op_clay_runtime_record("grace accept");
        "#,
    );
    let server = server_with_config(root.clone());
    let previous_behavior = server.behavior.lock().await.manifest().clone();
    assert_eq!(previous_behavior.behavior_version, 1);

    let outcome = server.reload_runtime_generation().await;
    assert!(outcome.reloaded);
    assert_eq!(server.runtime_generation.generation_id().await, 2);
    assert_eq!(server.behavior.lock().await.version(), 2);

    let (client, server_stream) = duplex(64 * 1024);
    let codec = crate::protocol::codec::Codec::default();
    let connection_server = server.clone();
    let tab_registry = Arc::clone(&connection_server.tab_registry);
    let tab_registry_tx = connection_server.tab_registry_tx.clone();
    let server_task = tokio::spawn(async move {
        crate::server::connection::handle_connection_with_analysis(
            server_stream,
            31,
            Arc::clone(&connection_server.document),
            Arc::clone(&connection_server.behavior),
            Arc::clone(&connection_server.workspace),
            Arc::clone(&connection_server.sdui),
            Arc::clone(&connection_server.active_theme),
            Arc::clone(&connection_server.runtime_diagnostics),
            connection_server.runtime_generation.clone(),
            connection_server.parse_coordinator.clone(),
            connection_server.completion.clone(),
            connection_server.document_analysis.clone(),
            connection_server.language_intelligence.clone(),
            Some(connection_server),
            tab_registry,
            tab_registry_tx,
            codec,
        )
        .await
    });
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &crate::protocol::ClientMessage::Hello {
                protocol_version: crate::protocol::PROTOCOL_VERSION,
                client_name: "grace-client".to_string(),
            },
        )
        .await
        .unwrap();
    bind_test_tab(&mut client, codec).await;

    let document = server.document.lock().await;
    let document_id = document.document_id();
    let version = document.version();
    let lease_id = match document.access_for_client(31) {
        crate::protocol::DocumentAccess::Editable { lease_id } => Some(lease_id),
        _ => None,
    };
    drop(document);

    codec
        .write_client_message(
            &mut client,
            &crate::protocol::ClientMessage::Edit {
                document_id,
                client_id: 31,
                lease_id,
                base_version: version,
                behavior_version: previous_behavior.behavior_version,
                transaction_id: 501,
                operation: crate::protocol::EditOperation::Insert {
                    byte_offset: 0,
                    text: "g1".to_string(),
                },
            },
        )
        .await
        .unwrap();

    match codec.read_server_message(&mut client).await.unwrap() {
        crate::protocol::ServerMessage::EditAck {
            document_id: ack_document_id,
            transaction_id,
            confirmed_version,
        } => {
            assert_eq!(ack_document_id, document_id);
            assert_eq!(transaction_id, 501);
            assert_eq!(confirmed_version, version + 1);
        }
        other => panic!("expected EditAck under grace, got {other:?}"),
    }

    drop(client);
    let _ = server_task.await;
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn previous_generation_edit_after_ack_or_expiry_is_rejected_and_snapshot_resent() {
    use std::sync::Arc;
    use tokio::io::duplex;

    let root = temp_config_root(
        "grace-reject",
        r#"
        import { bindKey } from "clay:keybindings";
        bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
        Deno.core.ops.op_clay_runtime_record("grace reject");
        "#,
    );
    let server = server_with_config(root.clone());
    let previous_version = server.behavior.lock().await.version();
    assert!(server.reload_runtime_generation().await.reloaded);

    let (client, server_stream) = duplex(64 * 1024);
    let codec = crate::protocol::codec::Codec::default();
    let connection_server = server.clone();
    let tab_registry = Arc::clone(&connection_server.tab_registry);
    let tab_registry_tx = connection_server.tab_registry_tx.clone();
    let server_task = tokio::spawn(async move {
        crate::server::connection::handle_connection_with_analysis(
            server_stream,
            32,
            Arc::clone(&connection_server.document),
            Arc::clone(&connection_server.behavior),
            Arc::clone(&connection_server.workspace),
            Arc::clone(&connection_server.sdui),
            Arc::clone(&connection_server.active_theme),
            Arc::clone(&connection_server.runtime_diagnostics),
            connection_server.runtime_generation.clone(),
            connection_server.parse_coordinator.clone(),
            connection_server.completion.clone(),
            connection_server.document_analysis.clone(),
            connection_server.language_intelligence.clone(),
            Some(connection_server),
            tab_registry,
            tab_registry_tx,
            codec,
        )
        .await
    });
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &crate::protocol::ClientMessage::Hello {
                protocol_version: crate::protocol::PROTOCOL_VERSION,
                client_name: "grace-reject-client".to_string(),
            },
        )
        .await
        .unwrap();
    bind_test_tab(&mut client, codec).await;

    codec
        .write_client_message(
            &mut client,
            &crate::protocol::ClientMessage::RuntimeGenerationInstalled {
                client_id: 32,
                runtime_generation_id: 2,
            },
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let document = server.document.lock().await;
    let document_id = document.document_id();
    let version = document.version();
    let lease_id = match document.access_for_client(32) {
        crate::protocol::DocumentAccess::Editable { lease_id } => Some(lease_id),
        _ => None,
    };
    let text_before = document.text();
    drop(document);

    codec
        .write_client_message(
            &mut client,
            &crate::protocol::ClientMessage::Edit {
                document_id,
                client_id: 32,
                lease_id,
                base_version: version,
                behavior_version: previous_version,
                transaction_id: 777,
                operation: crate::protocol::EditOperation::Insert {
                    byte_offset: 0,
                    text: "stale".to_string(),
                },
            },
        )
        .await
        .unwrap();

    match codec.read_server_message(&mut client).await.unwrap() {
        crate::protocol::ServerMessage::EditRejected {
            reason:
                crate::protocol::EditRejection::InvalidBehaviorVersion {
                    behavior_version,
                    server_behavior_version,
                },
            ..
        } => {
            assert_eq!(behavior_version, previous_version);
            assert_eq!(server_behavior_version, 2);
        }
        other => panic!("expected InvalidBehaviorVersion, got {other:?}"),
    }
    match codec.read_server_message(&mut client).await.unwrap() {
        crate::protocol::ServerMessage::RuntimeStateSnapshot(snapshot) => {
            assert_eq!(snapshot.runtime_generation_id, 2);
            assert_eq!(snapshot.client_id, 32);
        }
        other => panic!("expected RuntimeStateSnapshot republish, got {other:?}"),
    }
    assert_eq!(server.document.lock().await.text(), text_before);

    server
        .runtime_generation
        .behavior_grace()
        .expire_for_test()
        .await;
    assert!(
        server
            .runtime_generation
            .behavior_grace()
            .validate_edit_version(
                &*server.behavior.lock().await,
                99,
                document_id,
                1,
                previous_version,
                2,
                None,
                std::time::Instant::now(),
            )
            .await
            .is_err()
    );

    drop(client);
    let _ = server_task.await;
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn grace_never_bypasses_lease_validation() {
    use std::sync::Arc;
    use tokio::io::duplex;

    let root = temp_config_root(
        "grace-lease",
        r#"
        import { bindKey } from "clay:keybindings";
        bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
        Deno.core.ops.op_clay_runtime_record("grace lease");
        "#,
    );
    let server = server_with_config(root.clone());
    let previous_version = server.behavior.lock().await.version();
    assert!(server.reload_runtime_generation().await.reloaded);

    let (client, server_stream) = duplex(64 * 1024);
    let codec = crate::protocol::codec::Codec::default();
    let connection_server = server.clone();
    let tab_registry = Arc::clone(&connection_server.tab_registry);
    let tab_registry_tx = connection_server.tab_registry_tx.clone();
    let server_task = tokio::spawn(async move {
        crate::server::connection::handle_connection_with_analysis(
            server_stream,
            33,
            Arc::clone(&connection_server.document),
            Arc::clone(&connection_server.behavior),
            Arc::clone(&connection_server.workspace),
            Arc::clone(&connection_server.sdui),
            Arc::clone(&connection_server.active_theme),
            Arc::clone(&connection_server.runtime_diagnostics),
            connection_server.runtime_generation.clone(),
            connection_server.parse_coordinator.clone(),
            connection_server.completion.clone(),
            connection_server.document_analysis.clone(),
            connection_server.language_intelligence.clone(),
            Some(connection_server),
            tab_registry,
            tab_registry_tx,
            codec,
        )
        .await
    });
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &crate::protocol::ClientMessage::Hello {
                protocol_version: crate::protocol::PROTOCOL_VERSION,
                client_name: "grace-lease-client".to_string(),
            },
        )
        .await
        .unwrap();
    bind_test_tab(&mut client, codec).await;

    let document = server.document.lock().await;
    let document_id = document.document_id();
    let version = document.version();
    let text_before = document.text();
    drop(document);

    codec
        .write_client_message(
            &mut client,
            &crate::protocol::ClientMessage::Edit {
                document_id,
                client_id: 33,
                lease_id: Some(u64::MAX),
                base_version: version,
                behavior_version: previous_version,
                transaction_id: 808,
                operation: crate::protocol::EditOperation::Insert {
                    byte_offset: 0,
                    text: "nope".to_string(),
                },
            },
        )
        .await
        .unwrap();

    match codec.read_server_message(&mut client).await.unwrap() {
        crate::protocol::ServerMessage::EditRejected {
            reason:
                crate::protocol::EditRejection::LeaseExpired { .. }
                | crate::protocol::EditRejection::LeaseRequired,
            ..
        } => {}
        other => panic!("grace must not bypass lease checks, got {other:?}"),
    }
    assert_eq!(server.document.lock().await.text(), text_before);

    drop(client);
    let _ = server_task.await;
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn typing_and_edit_ack_continue_while_candidate_runtime_is_blocked_on_test_barrier() {
    use std::sync::Arc;
    use tokio::io::duplex;

    let root = temp_config_root(
        "barrier-typing",
        r#"Deno.core.ops.op_clay_runtime_record("barrier ok");"#,
    );
    let server = server_with_config(root.clone());
    let (entered_rx, release_tx) = server.arm_reload_candidate_barrier().await;

    let (client, server_stream) = duplex(64 * 1024);
    let codec = crate::protocol::codec::Codec::default();
    let connection_server = server.clone();
    let tab_registry = Arc::clone(&connection_server.tab_registry);
    let tab_registry_tx = connection_server.tab_registry_tx.clone();
    let server_task = tokio::spawn(async move {
        crate::server::connection::handle_connection_with_analysis(
            server_stream,
            41,
            Arc::clone(&connection_server.document),
            Arc::clone(&connection_server.behavior),
            Arc::clone(&connection_server.workspace),
            Arc::clone(&connection_server.sdui),
            Arc::clone(&connection_server.active_theme),
            Arc::clone(&connection_server.runtime_diagnostics),
            connection_server.runtime_generation.clone(),
            connection_server.parse_coordinator.clone(),
            connection_server.completion.clone(),
            connection_server.document_analysis.clone(),
            connection_server.language_intelligence.clone(),
            Some(connection_server),
            tab_registry,
            tab_registry_tx,
            codec,
        )
        .await
    });
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &crate::protocol::ClientMessage::Hello {
                protocol_version: crate::protocol::PROTOCOL_VERSION,
                client_name: "barrier-client".to_string(),
            },
        )
        .await
        .unwrap();
    bind_test_tab(&mut client, codec).await;

    let reload_server = server.clone();
    let reload_task = tokio::spawn(async move { reload_server.reload_runtime_generation().await });
    entered_rx
        .await
        .expect("reload candidate must reach the test barrier");
    assert_eq!(
        server.runtime_generation.generation_id().await,
        1,
        "generation must stay on G1 while candidate is blocked"
    );

    let document = server.document.lock().await;
    let document_id = document.document_id();
    let version = document.version();
    let lease_id = match document.access_for_client(41) {
        crate::protocol::DocumentAccess::Editable { lease_id } => Some(lease_id),
        _ => None,
    };
    let behavior_version = server.behavior.lock().await.version();
    drop(document);

    codec
        .write_client_message(
            &mut client,
            &crate::protocol::ClientMessage::Edit {
                document_id,
                client_id: 41,
                lease_id,
                base_version: version,
                behavior_version,
                transaction_id: 7001,
                operation: crate::protocol::EditOperation::Insert {
                    byte_offset: 0,
                    text: "typed".to_string(),
                },
            },
        )
        .await
        .unwrap();

    match tokio::time::timeout(
        Duration::from_millis(250),
        codec.read_server_message(&mut client),
    )
    .await
    .expect("edit ack must not wait for blocked reload")
    .unwrap()
    {
        crate::protocol::ServerMessage::EditAck {
            document_id: ack_document_id,
            transaction_id,
            confirmed_version,
        } => {
            assert_eq!(ack_document_id, document_id);
            assert_eq!(transaction_id, 7001);
            assert_eq!(confirmed_version, version + 1);
        }
        other => panic!("expected EditAck while reload is blocked, got {other:?}"),
    }

    release_tx.send(()).expect("release blocked reload");
    let outcome = reload_task.await.expect("reload task joins");
    assert!(outcome.reloaded);
    assert_eq!(outcome.active_generation_id, 2);

    drop(client);
    let _ = server_task.await;
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn failed_reload_broadcasts_diagnostic_but_no_generation_snapshot() {
    use std::sync::Arc;
    use tokio::io::duplex;

    let root = temp_config_root(
        "failed-no-snapshot",
        r#"Deno.core.ops.op_clay_runtime_record("baseline");"#,
    );
    let server = server_with_config(root.clone());
    assert_eq!(server.runtime_generation.generation_id().await, 1);

    let (client, server_stream) = duplex(64 * 1024);
    let codec = crate::protocol::codec::Codec::default();
    let connection_server = server.clone();
    let tab_registry = Arc::clone(&connection_server.tab_registry);
    let tab_registry_tx = connection_server.tab_registry_tx.clone();
    let server_task = tokio::spawn(async move {
        crate::server::connection::handle_connection_with_analysis(
            server_stream,
            42,
            Arc::clone(&connection_server.document),
            Arc::clone(&connection_server.behavior),
            Arc::clone(&connection_server.workspace),
            Arc::clone(&connection_server.sdui),
            Arc::clone(&connection_server.active_theme),
            Arc::clone(&connection_server.runtime_diagnostics),
            connection_server.runtime_generation.clone(),
            connection_server.parse_coordinator.clone(),
            connection_server.completion.clone(),
            connection_server.document_analysis.clone(),
            connection_server.language_intelligence.clone(),
            Some(connection_server),
            tab_registry,
            tab_registry_tx,
            codec,
        )
        .await
    });
    let mut client = client;

    codec
        .write_client_message(
            &mut client,
            &crate::protocol::ClientMessage::Hello {
                protocol_version: crate::protocol::PROTOCOL_VERSION,
                client_name: "failed-reload-client".to_string(),
            },
        )
        .await
        .unwrap();
    bind_test_tab(&mut client, codec).await;

    let mut updates = server.runtime_generation.subscribe_runtime_state();
    fs::write(root.join("init.js"), "export const = ;").unwrap();

    let outcome = server
        .execute_reload_command(reload_request())
        .await
        .expect("failed reload still returns a command outcome");
    assert!(!outcome.reloaded);
    assert_eq!(outcome.active_generation_id, 1);
    assert!(
        outcome
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime.syntax_error")
    );
    assert!(
        server
            .runtime_diagnostics
            .lock()
            .await
            .snapshot()
            .iter()
            .any(|diagnostic| diagnostic.code == "runtime.syntax_error")
    );
    assert!(
        server
            .runtime_generation
            .latest_runtime_snapshot_for(42)
            .await
            .is_none()
    );
    assert!(
        matches!(
            updates.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ),
        "failed reload must not publish a runtime generation id"
    );

    // Live connection must stay on G1: no RuntimeStateSnapshot appears.
    match tokio::time::timeout(
        Duration::from_millis(80),
        codec.read_server_message(&mut client),
    )
    .await
    {
        Err(_) => {}
        Ok(Ok(ServerMessage::RuntimeStateSnapshot(_))) => {
            panic!("failed reload must not fan out a generation snapshot")
        }
        Ok(Ok(ServerMessage::RuntimeDiagnostic(diagnostic))) => {
            assert_ne!(diagnostic.code, "runtime.reload_succeeded");
        }
        Ok(Ok(other)) => panic!("unexpected live message after failed reload: {other:?}"),
        Ok(Err(error)) => panic!("client read failed: {error}"),
    }

    drop(client);
    let _ = server_task.await;
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn successful_reload_is_observed_as_one_generation_by_all_clients() {
    use std::sync::Arc;
    use tokio::io::duplex;

    let root = temp_config_root(
        "one-generation",
        r#"
        import { bindKey } from "clay:keybindings";
        bindKey("Ctrl+S", "documents.serverSaveDocument", { scope: "editor" });
        Deno.core.ops.op_clay_runtime_record("one generation");
        "#,
    );
    let server = server_with_config(root.clone());
    let codec = crate::protocol::codec::Codec::default();

    async fn bootstrap_client(
        server: &IpcServer,
        client_id: u64,
        codec: crate::protocol::codec::Codec,
    ) -> (tokio::io::DuplexStream, tokio::task::JoinHandle<()>) {
        let (client, server_stream) = duplex(64 * 1024);
        let connection_server = server.clone();
        let tab_registry = Arc::clone(&connection_server.tab_registry);
        let tab_registry_tx = connection_server.tab_registry_tx.clone();
        let handle = tokio::spawn(async move {
            let _ = crate::server::connection::handle_connection_with_analysis(
                server_stream,
                client_id,
                Arc::clone(&connection_server.document),
                Arc::clone(&connection_server.behavior),
                Arc::clone(&connection_server.workspace),
                Arc::clone(&connection_server.sdui),
                Arc::clone(&connection_server.active_theme),
                Arc::clone(&connection_server.runtime_diagnostics),
                connection_server.runtime_generation.clone(),
                connection_server.parse_coordinator.clone(),
                connection_server.completion.clone(),
                connection_server.document_analysis.clone(),
                connection_server.language_intelligence.clone(),
                Some(connection_server),
                tab_registry,
                tab_registry_tx,
                codec,
            )
            .await;
        });
        let mut client = client;
        codec
            .write_client_message(
                &mut client,
                &crate::protocol::ClientMessage::Hello {
                    protocol_version: crate::protocol::PROTOCOL_VERSION,
                    client_name: format!("client-{client_id}"),
                },
            )
            .await
            .unwrap();
        for _ in 0..16 {
            match tokio::time::timeout(
                Duration::from_millis(10),
                codec.read_server_message(&mut client),
            )
            .await
            {
                Ok(Ok(_)) => {}
                Ok(Err(_)) | Err(_) => break,
            }
        }
        (client, handle)
    }

    let (mut client_a, task_a) = bootstrap_client(&server, 51, codec).await;
    let (mut client_b, task_b) = bootstrap_client(&server, 52, codec).await;

    assert!(server.reload_runtime_generation().await.reloaded);
    assert_eq!(server.runtime_generation.generation_id().await, 2);

    async fn read_complete_snapshot(
        codec: &crate::protocol::codec::Codec,
        client: &mut tokio::io::DuplexStream,
        expected_client_id: u64,
    ) -> crate::protocol::RuntimeStateSnapshot {
        loop {
            match tokio::time::timeout(
                Duration::from_millis(500),
                codec.read_server_message(client),
            )
            .await
            .expect("client receives one complete generation")
            .unwrap()
            {
                ServerMessage::RuntimeStateSnapshot(snapshot) => {
                    assert_eq!(snapshot.client_id, expected_client_id);
                    assert_eq!(snapshot.runtime_generation_id, 2);
                    assert_eq!(snapshot.behavior.behavior_version, 2);
                    snapshot.validate().expect("fan-out snapshot validates");
                    return *snapshot;
                }
                ServerMessage::ActiveTypography(_)
                | ServerMessage::BehaviorManifest(_)
                | ServerMessage::DecorationSet(_)
                | ServerMessage::DiagnosticSet(_)
                | ServerMessage::FoldingRangeSet(_)
                | ServerMessage::RuntimeDiagnostic(_)
                | ServerMessage::SduiSnapshot { .. }
                | ServerMessage::SduiUpdate { .. } => {}
                other => panic!("unexpected fan-out message: {other:?}"),
            }
        }
    }

    let snapshot_a = read_complete_snapshot(&codec, &mut client_a, 51).await;
    let snapshot_b = read_complete_snapshot(&codec, &mut client_b, 52).await;
    assert_eq!(
        snapshot_a.runtime_generation_id,
        snapshot_b.runtime_generation_id
    );
    assert_eq!(
        snapshot_a.behavior.behavior_version,
        snapshot_b.behavior.behavior_version
    );
    assert_eq!(snapshot_a.active_theme, snapshot_b.active_theme);
    assert_eq!(
        snapshot_a.active_typography.revision,
        snapshot_b.active_typography.revision
    );

    // Neither client may observe a second generation id for this commit.
    for (client, client_id) in [(&mut client_a, 51u64), (&mut client_b, 52)] {
        if let Ok(Ok(ServerMessage::RuntimeStateSnapshot(snapshot))) =
            tokio::time::timeout(Duration::from_millis(40), codec.read_server_message(client)).await
        {
            panic!(
                "client {client_id} observed a second snapshot generation {}",
                snapshot.runtime_generation_id
            );
        }
    }

    drop(client_a);
    drop(client_b);
    let _ = task_a.await;
    let _ = task_b.await;
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[tokio::test]
async fn reload_preserves_authority_denials_and_cleans_old_lsp_worker() {
    use std::{
        io::Write,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
    };

    use crate::server::language_server::LanguageServerSpawn;

    fn fake_echo_child(root: &Path) -> PathBuf {
        let path = root.join("fake-echo");
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(
            b"#!/bin/sh\nwhile IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done\n",
        )
        .unwrap();
        file.sync_all().unwrap();
        drop(file);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    let root = temp_config_root(
        "lsp-cleanup",
        r#"Deno.core.ops.op_clay_runtime_record("lsp cleanup");"#,
    );
    let server = server_with_config(root.clone());
    let previous = server.runtime_generation.current_service().await;
    let process = previous.language_server_process_for_test();
    let executable = fake_echo_child(&root);
    let session = process
        .start(LanguageServerSpawn {
            package_name: "example".to_string(),
            contribution_id: "example.echo".to_string(),
            descriptor_fingerprint: 0,
            canonical_executable: executable,
            args: Vec::new(),
            inherit_environment: Vec::new(),
            cwd: root.clone(),
        })
        .await
        .expect("seed previous-generation language-server session");
    assert_eq!(previous.language_server_session_count().await, 1);
    let _ = session;

    let outcome = server.reload_runtime_generation().await;
    assert!(outcome.reloaded);
    assert_eq!(outcome.active_generation_id, 2);
    assert_eq!(
        previous.language_server_session_count().await,
        0,
        "successful commit must shut down previous-generation language-server sessions"
    );

    // Authority denials remain deny-by-default after a successful swap.
    fs::write(
        root.join("init.js"),
        r#"import "https://example.com/not-allowed.js";"#,
    )
    .unwrap();
    let denied = server.reload_runtime_generation().await;
    assert!(!denied.reloaded);
    assert_eq!(denied.active_generation_id, 2);
    let diagnostic = denied.diagnostics.last().expect("denial diagnostic");
    assert_eq!(diagnostic.code, "configuration.invalid_module");
    assert!(
        !diagnostic
            .message
            .contains("https://example.com/not-allowed.js"),
        "diagnostics must not leak denied module URLs"
    );
    assert!(
        !diagnostic
            .message
            .contains(&root.to_string_lossy().to_string()),
        "diagnostics must not leak configuration paths"
    );

    let _ = fs::remove_dir_all(root);
}
