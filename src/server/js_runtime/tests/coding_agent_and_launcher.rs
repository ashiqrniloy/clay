use super::*;

#[test]
fn first_party_example_loads_coding_agent_with_one_uncommented_line() {
    let source = fs::read_to_string("examples/config/packages/first-party.js").unwrap();
    assert!(
        source.contains(r#"await loadPackage("@clay/coding-agent");"#),
        "canonical first-party module must opt into the agent with one uncommented load"
    );
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.contains(r#"loadPackage("@clay/coding-agent")"#) {
            return;
        }
    }
    panic!("@clay/coding-agent load must not be comment-only");
}

#[test]
fn coding_agent_load_entry_is_execute_only() {
    let load = fs::read_to_string("packages/coding-agent/dist/load.js").unwrap();
    assert!(!load.contains("Deno.core"));
    assert!(
        load.contains("export default loadCodingAgentPackage"),
        "loadPackage must invoke the package-owned default export"
    );
}

#[test]
fn coding_agent_load_entry_registers_profile_without_hardcoded_skills() {
    let load = fs::read_to_string("packages/coding-agent/dist/load.js")
        .expect("read coding-agent load entry");
    assert!(!load.contains("Deno.core"), "no raw ops in package JS");
    assert!(
        load.contains("clay:agent"),
        "profile registration rides the documented facade"
    );
    assert!(
        load.contains("export default loadCodingAgentPackage"),
        "loadPackage must invoke the package-owned default export"
    );
    // No hardcoded skill ships with the package anymore: skills come from
    // the daemon's `.agents/skills/` disk discovery, so the load entry must
    // not register any skill and the profile must not name any.
    assert!(
        !load.contains("skillRegister"),
        "no hardcoded skill registration; disk discovery owns skills"
    );
    assert!(
        !load.contains("skills:"),
        "the coding profile declares no skills"
    );
    let profile_at = load
        .find("await profileRegister(")
        .expect("profile registration call");
    let command_at = load
        .find("await commandRegister(")
        .expect("command registration call");
    assert!(
        profile_at < command_at,
        "the profile registers before the slash commands"
    );
    assert!(
        load.contains("\"/compact\"") && load.contains("\"/open-session-as-fork\""),
        "the pi-parity slash surface registers with the package"
    );
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string("packages/coding-agent/package.json").unwrap())
            .unwrap();
    assert_eq!(manifest["clay"]["apiPrefix"], "coding-agent");
    assert!(
        load.contains("repo_search") && load.contains("ask_user_decision"),
        "the registered tool set is the nine coding tools plus ask_user_decision"
    );
}

#[tokio::test]
async fn third_party_cannot_import_trusted_package_modules() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let error = service
        .evaluate_third_party_module(
            r#"import { loadCodingAgentPackage } from "clay://packages/@clay/coding-agent/dist/load.js";"#,
        )
        .await
        .expect_err("third-party runtime must not import trusted package modules");
    assert!(
        error.to_string().contains("runtime.invalid_import")
            || error.to_string().contains("denied"),
        "unexpected deny: {error}"
    );
}

/// Plan 118 Part D: the launcher owns the window's landing, the coding agent
/// keeps its named `pane` surface, and neither competes for the other's slot.
#[tokio::test]
async fn launcher_claims_the_empty_tab_landing_and_the_agent_keeps_its_pane() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = config_fixture("launcher-empty-tab-landing");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("@clay/launcher");
        await loadPackage("@clay/coding-agent");
        "#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("both first-party packages load");
    // Resolved exactly as the wire snapshot resolves it: bundled packages are
    // trusted, so the host renders the compiled launcher panel for it.
    let wire = result
        .ui_contributions
        .wire_snapshot(1, |_| crate::protocol::PackageUiTrustDomain::Trusted)
        .expect("no election conflict");
    let winner = wire.empty_tab.expect("the launcher claims the empty tab");
    assert_eq!(winner.id, "launcher.start");
    assert_eq!(winner.package_name, "@clay/launcher");
    assert_eq!(
        winner.provenance.trust_domain,
        crate::protocol::PackageUiTrustDomain::Trusted
    );
    // The landing carries only the inert folder-dialog action.
    assert_eq!(
        winner.action_targets,
        vec!["workspace.clientOpenFolderDialog".to_string()]
    );

    assert_eq!(
        wire.surfaces
            .iter()
            .map(|surface| surface.id.as_str())
            .collect::<Vec<_>>(),
        vec!["coding-agent.surface"],
        "the agent keeps exactly its named pane surface"
    );
    assert!(
        result
            .ui_contributions
            .pane_contents
            .iter()
            .filter(|entry| entry.activation == "empty-tab")
            .all(|entry| entry.id == "launcher.start"),
        "the agent never competes for the empty tab"
    );
}

#[tokio::test]
async fn chat_package_removal_leaves_the_core_empty_tab_fallback() {
    // Plan 118: `@clay/chat` is deleted from the bundled inventory and its
    // load line is gone from the canonical first-party module, so the shipped
    // configuration contributes no empty-tab landing at all: the empty tab
    // renders the core Open File / Open Folder fallback (and the launcher
    // package takes the landing in a later plan 118 task). Loading the retired
    // specifier now fails as an unknown package instead of silently installing
    // a landing.
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = config_fixture("chat-package-removed");
    fs::write(root.join("init.js"), "// no packages\n").unwrap();
    let service = ClayJsRuntimeService::default();
    let result = service
        .load_configuration_from_root(root)
        .await
        .expect("empty init.js must load");
    assert_eq!(
        result.ui_contributions.empty_tab().expect("no conflict"),
        None,
        "no package contribution → core empty-tab fallback"
    );
    let (trusted, _) = service.command_registry_snapshots();
    assert!(
        trusted
            .iter()
            .all(|command| !command.command_id.starts_with("chat.")),
        "no loadPackage → no chat commands"
    );

    let root = config_fixture("chat-package-retired-specifier");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("@clay/chat");
        "#,
    )
    .unwrap();
    let service = ClayJsRuntimeService::default();
    let error = service
        .load_configuration_from_root(root)
        .await
        .expect_err("a retired bundled specifier must not load");
    assert!(
        format!("{error:?}").contains("chat"),
        "the failure names the requested specifier: {error:?}"
    );
}
