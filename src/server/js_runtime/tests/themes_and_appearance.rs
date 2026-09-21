use super::*;

#[tokio::test]
async fn set_theme_resolves_first_party_gruvbox_theme() {
    // Gruvbox stays opt-in: both Gruvbox Material variants are selectable by
    // a one-line `setTheme` call; neither is a canonical default.
    for specifier in [
        "@clay/theme-gruvbox-material-dark",
        "@clay/theme-gruvbox-material-light",
    ] {
        let root = config_fixture(&format!(
            "set-theme-gruvbox-e2e-{}",
            specifier.trim_start_matches("@clay/theme-gruvbox-material-")
        ));
        fs::write(
            root.join("init.js"),
            format!(
                r#"
                import {{ setTheme }} from "clay:theme";
                const summary = setTheme("{specifier}");
                Deno.core.ops.op_clay_runtime_record(
                  `theme:${{summary.specifier}}:overrides:${{summary.overrideCount}}`
                );
                "#
            ),
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap_or_else(|err| panic!("setTheme('{specifier}') must succeed: {err:?}"));

        let theme = result.active_theme.expect("active theme snapshot emitted");
        assert_eq!(theme.specifier, specifier);
        assert_eq!(theme.overrides.len(), 49);
        assert!(
            result
                .op_records
                .iter()
                .any(|record| *record == format!("theme:{specifier}:overrides:49")),
            "setTheme('{specifier}') summary must reach init.js"
        );
    }
}

#[tokio::test]
async fn default_config_resolves_canonical_dark_modus_vivendi() {
    // No explicit setTheme: appearance defaults to System, which with no OS
    // signal falls back to dark → canonical default Modus Vivendi.
    let root = config_fixture("default-theme-appearance-e2e");
    fs::write(root.join("init.js"), "// no setTheme call\n").unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("default config must evaluate");
    let theme = result
        .active_theme
        .as_ref()
        .expect("canonical default theme must be resolved when no explicit theme is set");
    assert_eq!(theme.specifier, "@clay/theme-modus-vivendi");
    assert_eq!(theme.overrides.len(), 49);
}

#[tokio::test]
async fn set_appearance_light_resolves_canonical_modus_operandi() {
    let root = config_fixture("set-appearance-light-e2e");
    fs::write(
        root.join("init.js"),
        r#"
        import { setAppearance } from "clay:theme";
        const summary = setAppearance("light");
        Deno.core.ops.op_clay_runtime_record(
          `appearance:${summary.appearance}:theme:${summary.resolvedTheme}`
        );
        "#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("setAppearance('light') must succeed");
    let theme = result
        .active_theme
        .as_ref()
        .expect("canonical light default must be resolved");
    assert_eq!(theme.specifier, "@clay/theme-modus-operandi");
    assert!(
        result
            .op_records
            .iter()
            .any(|r| r == "appearance:light:theme:@clay/theme-modus-operandi"),
        "setAppearance summary must reach init.js"
    );
}

#[tokio::test]
async fn explicit_set_theme_wins_over_appearance() {
    let root = config_fixture("explicit-theme-wins-e2e");
    fs::write(
        root.join("init.js"),
        r#"
        import { setTheme, setAppearance } from "clay:theme";
        setTheme("@clay/theme-gruvbox-material-dark");
        const summary = setAppearance("light");
        Deno.core.ops.op_clay_runtime_record(
          `appearance:${summary.appearance}:resolved:${summary.resolvedTheme}`
        );
        "#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("config must evaluate");
    let theme = result
        .active_theme
        .as_ref()
        .expect("explicit theme must remain active");
    assert_eq!(
        theme.specifier, "@clay/theme-gruvbox-material-dark",
        "explicit setTheme must win over appearance-derived default"
    );
    // setAppearance reports no re-resolution once an explicit theme is active.
    assert!(
        result
            .op_records
            .iter()
            .any(|r| r == "appearance:light:resolved:null"),
        "setAppearance must not re-resolve over an explicit theme"
    );
}

#[tokio::test]
async fn set_appearance_rejects_unknown_value() {
    let root = config_fixture("set-appearance-invalid-e2e");
    fs::write(
        root.join("init.js"),
        r#"
        import { setAppearance } from "clay:theme";
        try {
          setAppearance("nope");
          Deno.core.ops.op_clay_runtime_record("appearance:accepted");
        } catch (err) {
          Deno.core.ops.op_clay_runtime_record(`appearance:rejected:${err.message.split(":")[0]}`);
        }
        "#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("config must evaluate");
    assert!(
        result
            .op_records
            .iter()
            .any(|r| r == "appearance:rejected:theme.invalid_request"),
        "unknown appearance must be rejected with theme.invalid_request"
    );
}

#[tokio::test]
async fn settings_package_registers_catalog_only_panel() {
    let root = config_fixture("settings-package-e2e");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("@clay/settings");
        "#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("@clay/settings must load");
    let panel = result
        .ui_contributions
        .panels
        .iter()
        .find(|panel| panel.id == "settings.surface")
        .expect("settings.surface panel contribution must register");
    // Every action target is a settings.* command intent.
    for target in &panel.action_targets {
        assert!(
            target.starts_with("settings."),
            "settings panel action target `{target}` must be a settings.* intent"
        );
    }
    // Every component kind in the tree is an implemented catalog kind.
    let mut kinds: Vec<&str> = Vec::new();
    collect_kinds(&panel.component_tree, &mut kinds);
    assert!(
        kinds.iter().all(|kind| matches!(
            *kind,
            "panel"
                | "label"
                | "button"
                | "list"
                | "flex"
                | "stack"
                | "overlay"
                | "scroll"
                | "portal"
                | "statusItem"
                | "dropdown"
                | "collapse"
                | "modal"
                | "textInput"
                | "editorView"
        )),
        "settings surface must use only catalog kinds, got {kinds:?}"
    );
    // Theme and appearance dropdowns plus typography textInputs are present.
    assert!(
        kinds.contains(&"dropdown"),
        "theme/appearance dropdowns present"
    );
    assert!(
        kinds.contains(&"textInput"),
        "typography textInputs present"
    );
    assert!(kinds.contains(&"collapse"), "collapsible sections present");
    assert!(kinds.contains(&"button"), "action buttons present");
}
