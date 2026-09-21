use super::*;

#[tokio::test]
async fn set_theme_resolves_first_party_modus_themes() {
    for specifier in ["@clay/theme-modus-operandi", "@clay/theme-modus-vivendi"] {
        let root = config_fixture(&format!(
            "set-theme-modus-e2e-{}",
            specifier.trim_start_matches("@clay/theme-")
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
                .any(|record| record == &format!("theme:{specifier}:overrides:49")),
            "setTheme summary must reach init.js for {specifier}"
        );
    }
}

#[tokio::test]
async fn canonical_default_is_modus_not_gruvbox() {
    // Gruvbox stays opt-in: a silent init.js resolves the Modus canonical
    // default (dark / Modus Vivendi), never a Gruvbox theme. There is no
    // promotion-by-naming for Gruvbox.
    let root = config_fixture("canonical-default-not-gruvbox");
    fs::write(root.join("init.js"), "// silent\n").unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("silent config must evaluate");
    let theme = result.active_theme.expect("canonical default emitted");
    assert_eq!(theme.specifier, "@clay/theme-modus-vivendi");
    assert_ne!(theme.specifier, "@clay/theme-gruvbox-material-dark");
    assert_ne!(theme.specifier, "@clay/theme-gruvbox-material-light");
}

#[tokio::test]
async fn explicit_set_theme_wins_over_canonical_default() {
    // An explicit `setTheme` for a non-default bundled theme overrides the
    // appearance-derived canonical default without any `loadPackage` call.
    let root = config_fixture("explicit-theme-beats-canonical-default");
    fs::write(
        root.join("init.js"),
        r#"
        import { setTheme } from "clay:theme";
        setTheme("@clay/theme-gruvbox-material-light");
        "#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("explicit setTheme config must evaluate");
    let theme = result.active_theme.expect("active theme emitted");
    assert_eq!(theme.specifier, "@clay/theme-gruvbox-material-light");
}

#[tokio::test]
async fn absent_init_js_loads_no_runtime_theme() {
    // Boundary: with no init.js at all the default-config loader returns
    // None and resolves no runtime theme (the editor/shell core default
    // applies; the canonical Modus default requires an init.js entry
    // point to run). This documents the loading-experience boundary:
    // canonical defaults need no `loadPackage`, but they do need the
    // `init.js` entry point to evaluate.
    let root = config_fixture("absent-init-js");
    // Deliberately do NOT create init.js.
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await;
    assert!(
        result.is_err(),
        "absent init.js must not silently evaluate, got {:?}",
        result
    );
}

#[tokio::test]
async fn set_typography_replaces_all_profiles_atomically() {
    let root = config_fixture("set-typography-e2e");
    fs::write(
        root.join("init.js"),
        r#"
        import { setTypography } from "clay:theme";
        const summary = setTypography({
          monospace: { families: ["JetBrains Mono", "monospace"], size: 16 },
          proportional: { families: ["Inter", "sans-serif"], size: 17 },
          ui: { families: ["system-ui"], size: 13 },
        });
        Deno.core.ops.op_clay_runtime_record(`typography:${summary.revision}`);
        "#,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("setTypography must accept one complete valid replacement");
    let typography = result
        .active_typography
        .expect("complete typography candidate emitted");
    assert_eq!(typography.revision, 1);
    assert_eq!(typography.monospace.families[0], "JetBrains Mono");
    assert_eq!(typography.proportional.size, 17.0);
    assert_eq!(typography.ui.families, ["system-ui"]);
    assert!(
        result
            .op_records
            .iter()
            .any(|record| record == "typography:1")
    );
}

#[tokio::test]
async fn set_typography_failure_preserves_previous_revision() {
    let service = ClayJsRuntimeService::default();
    let first = service
        .evaluate_controlled_module(
            r#"import { setTypography } from "clay:theme";
            setTypography({
              monospace: { families: ["monospace"], size: 16 },
              proportional: { families: ["sans-serif"], size: 17 },
              ui: { families: ["system-ui"], size: 13 },
            });"#,
        )
        .await
        .expect("initial typography succeeds");
    assert_eq!(first.active_typography.unwrap().revision, 1);

    assert!(
        service
            .evaluate_controlled_module(
                r#"import { setTypography } from "clay:theme";
                setTypography({
                  monospace: { families: ["monospace"], size: 16 },
                  proportional: { families: ["sans-serif"], size: 17 },
                });"#,
            )
            .await
            .is_err(),
        "incomplete candidate fails before state replacement"
    );

    let second = service
        .evaluate_controlled_module(
            r#"import { setTypography } from "clay:theme";
            setTypography({
              monospace: { families: ["monospace"], size: 18 },
              proportional: { families: ["sans-serif"], size: 19 },
              ui: { families: ["system-ui"], size: 14 },
            });"#,
        )
        .await
        .expect("valid replacement after failure succeeds");
    let typography = second.active_typography.unwrap();
    assert_eq!(typography.revision, 2);
    assert_eq!(typography.monospace.size, 18.0);
}

#[tokio::test]
async fn typography_configuration_rejects_oversized_snapshot() {
    let service = ClayJsRuntimeService::default();
    assert!(
        service
            .evaluate_controlled_module(
                r#"import { setTypography } from "clay:theme";
                const named = "x".repeat(128);
                setTypography({
                  monospace: { families: [named, named, named, named, named, named, named, "monospace"], size: 16 },
                  proportional: { families: [named, named, named, named, named, named, named, "sans-serif"], size: 17 },
                  ui: { families: [named, named, named, named, named, named, named, "system-ui"], size: 13 },
                });"#,
            )
            .await
            .is_err(),
        "one typography update remains bounded even when individual fields are valid"
    );
}

#[tokio::test]
async fn typography_configuration_grants_no_additional_authority() {
    let service = ClayJsRuntimeService::default();
    assert!(
        service
            .evaluate_controlled_module(
                r#"import { setTypography } from "clay:theme";
                setTypography({
                  monospace: { families: ["monospace"], size: 16, fontUrl: "https://example.com/font" },
                  proportional: { families: ["sans-serif"], size: 17 },
                  ui: { families: ["system-ui"], size: 13 },
                });"#,
            )
            .await
            .is_err(),
        "font URLs and all unrecognized authority fields are rejected"
    );
}
