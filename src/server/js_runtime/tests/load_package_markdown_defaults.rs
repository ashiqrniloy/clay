use super::*;

#[tokio::test]
async fn load_package_resolves_and_activates_first_party_markdown_end_to_end() {
    // The one-line default end-user path: a configuration module that does
    // `await loadPackage("@clay/markdown")` activates the package — the
    // loadEntry imports curated clay:* facades and registers its mode,
    // commands, and parse handler under Clay's authority.
    let root = config_fixture("loadpackage-e2e");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
        const summary = await loadPackage("@clay/markdown");
        const classification = serverClassifyDocument({ documentId: 1, path: "README.md" });
        serverActivateClassifiedMode(classification, { path: "README.md" });
        Deno.core.ops.op_clay_runtime_record(
          `loaded:${summary.name}:modes:${summary.modes.join(",")}`
        );
        "#,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("loadPackage('@clay/markdown') must succeed end-to-end");

    // The resolver summary reaches the caller.
    assert!(
        result
            .op_records
            .iter()
            .any(|record| record == "loaded:@clay/markdown:modes:markdown"),
        "loadPackage must return the typed summary with name + modes, got {:?}",
        result.op_records
    );
    // The loadEntry's default activation registered a parse handler.
    assert!(
        !result.parse_handlers.is_empty(),
        "loadPackage must activate the markdown parse handler, got none"
    );
    // Modes/commands/keymaps surfaced through the behavior manifest.
    assert!(
        result.behavior_manifest.is_some(),
        "loadPackage must register mode/commands/keymaps into the behavior manifest"
    );
}

#[tokio::test]
async fn load_package_is_idempotent_per_persistent_runtime() {
    let root = config_fixture("loadpackage-idempotent");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        await loadPackage("@clay/markdown");
        await loadPackage("@clay/markdown");
        Deno.core.ops.op_clay_runtime_record("loaded-once");
        "#,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("repeated loadPackage calls must reuse the already-loaded package");

    assert!(
        result
            .op_records
            .iter()
            .any(|record| record == "loaded-once")
    );
    assert_eq!(result.js_parse_handlers.len(), 1);
}

#[tokio::test]
async fn load_package_remains_idempotent_inside_one_generation() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let evaluation = service
        .evaluate_controlled_module(
            r#"
            import { loadPackage } from "clay:packages";
            const first = await loadPackage("@clay/markdown");
            const second = await loadPackage("@clay/markdown");
            const third = await loadPackage("@clay/rust");
            const fourth = await loadPackage("@clay/rust");
            Deno.core.ops.op_clay_runtime_record(JSON.stringify({
              markdownSame: first === second,
              rustSame: third === fourth,
              markdownCached: Boolean(globalThis.__clayLoadedPackages?.["@clay/markdown"]),
              rustCached: Boolean(globalThis.__clayLoadedPackages?.["@clay/rust"]),
            }));
            "#,
        )
        .await
        .expect("in-generation repeated loads must succeed");

    assert_eq!(
        evaluation
            .js_parse_handlers
            .iter()
            .filter(|handler| handler.package.manifest.name == "@clay/markdown")
            .count(),
        1,
        "markdown parse handler must register once per generation"
    );
    assert_eq!(
        evaluation
            .completion_providers
            .iter()
            .filter(|provider| provider.id == "rust.keywords")
            .count(),
        1,
        "rust completion provider must register once per generation"
    );
    let record = evaluation
        .op_records
        .into_iter()
        .next()
        .expect("idempotency record");
    let parsed: serde_json::Value = serde_json::from_str(&record).expect("valid JSON");
    assert_eq!(parsed["markdownSame"], true);
    assert_eq!(parsed["rustSame"], true);
    assert_eq!(parsed["markdownCached"], true);
    assert_eq!(parsed["rustCached"], true);
}

#[tokio::test]
async fn load_package_rejects_non_string_specifier() {
    // The facade validates the specifier type before touching the op,
    // mirroring bindKey/serverLoadPackage validation.
    for invalid in ["loadPackage(123)", "loadPackage()", "loadPackage(null)"] {
        let root = config_fixture("loadpackage-invalid");
        fs::write(
            root.join("init.js"),
            format!(
                r#"
                import {{ loadPackage }} from "clay:packages";
                try {{
                  await {invalid};
                  Deno.core.ops.op_clay_runtime_record("no-throw");
                }} catch (error) {{
                  Deno.core.ops.op_clay_runtime_record(String(error));
                }}
                "#
            ),
        )
        .unwrap();
        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .expect("the invalid-specifier facade call must not crash the runtime");
        assert!(
            result
                .op_records
                .iter()
                .any(|record| record.contains("packages.invalid_specifier")),
            "`{invalid}` must throw packages.invalid_specifier, got {:?}",
            result.op_records
        );
        assert!(
            !result.op_records.iter().any(|record| record == "no-throw"),
            "`{invalid}` must throw, not return normally"
        );
    }
}

#[tokio::test]
async fn markdown_optional_preview_is_valid_panel_contribution() {
    // Phase 20 task 4: the optional Markdown preview helper registers a
    // valid clay:ui PanelContribution (hidden right slot, toggle action
    // target, package provenance) — but ONLY when called explicitly. The
    // default load path never invokes it (guarded separately by the
    // `load_package_markdown_default_activates_full_mode_from_init_js`
    // test, which asserts no panel contribution is published by default).
    let root = config_fixture("markdown-optional-preview-panel");
    // load.js imports the `clay:ui` facade and `markdownPackageManifest`
    // from index.js, so the dist module graph must be copied.
    for file_name in ["index.js", "load.js"] {
        fs::write(
            root.join(file_name),
            fs::read_to_string(format!("packages/markdown/dist/{file_name}"))
                .expect("first-party Markdown runtime module must exist"),
        )
        .unwrap();
    }
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        import { registerMarkdownPreview } from "./load.js";

        // Realistic opt-in order: load the package first (registers the
        // markdown.togglePreview command and stamps host provenance for
        // this evaluation), THEN publish the optional panel.
        await loadPackage("@clay/markdown");
        const panel = registerMarkdownPreview();
        Deno.core.ops.op_clay_runtime_record(`${panel.id}:${panel.slot}:${panel.defaultVisibility}`);
        "#,
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("registerMarkdownPreview must succeed");

    // The returned declaration reached the caller with the contract shape.
    assert!(
        result
            .op_records
            .iter()
            .any(|record| record == "markdown.preview:right:hidden"),
        "registerMarkdownPreview must return the hidden right-slot panel, got {:?}",
        result.op_records
    );
    // The server-side PackageUiRegistry validated and recorded it with
    // package provenance.
    let panel = result
        .ui_contributions
        .panels
        .iter()
        .find(|panel| panel.id == "markdown.preview")
        .expect("the optional preview must register as a validated PanelContribution");
    assert_eq!(panel.slot, "right");
    assert_eq!(panel.default_visibility, "hidden");
    assert_eq!(panel.provenance.api_prefix, "markdown");
    assert!(
        panel
            .action_targets
            .iter()
            .any(|target| target == "markdown.togglePreview"),
        "preview panel must target the toggle command, got {:?}",
        panel.action_targets
    );
}

#[tokio::test]
async fn load_package_markdown_default_activates_full_mode_from_init_js() {
    // Phase 18.6 task 6: the one-line default end-user path activates the
    // FULL markdown setup (parse handler + commands + mode) from a genuinely
    // minimal init.js — no inline manifest, no per-primitive registration,
    // no manual clay facade plumbing in user config.
    let root = config_fixture("loadpackage-default");
    let init_js = r#"
        import { loadPackage } from "clay:packages";
        import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
        await loadPackage("@clay/markdown");
        const classification = serverClassifyDocument({ documentId: 1, path: "README.md" });
        serverActivateClassifiedMode(classification, { path: "README.md" });
        "#;
    fs::write(root.join("init.js"), init_js).unwrap();

    // The user config carries no manifest object and no per-primitive
    // registration calls — loadPackage does all of it.
    for forbidden in [
        "contributions",
        "modePattern",
        "serverRegisterCommand",
        "serverRegisterParseHandler",
        "serverActivateMajorMode",
        "markdownPackageManifest",
    ] {
        assert!(
            !init_js.contains(forbidden),
            "default init.js must not carry `{forbidden}` — loadPackage owns activation"
        );
    }

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("loadPackage('@clay/markdown') default must succeed");

    // The markdown parse handler registered (mode_id `markdown`).
    assert!(
        result
            .parse_handlers
            .iter()
            .any(|handler| handler.mode_id == "markdown"),
        "default load must register the markdown parse handler, got {:?}",
        result.parse_handlers
    );
    // The markdown commands surfaced into the behavior manifest.
    let manifest = result
        .behavior_manifest
        .as_ref()
        .expect("default load must activate the major mode into the behavior manifest");
    assert!(
        manifest
            .commands
            .iter()
            .any(|command| command.command_id == "markdown.togglePreview"),
        "default load must register the markdown.togglePreview command, got {:?}",
        manifest
            .commands
            .iter()
            .map(|c| &c.command_id)
            .collect::<Vec<_>>()
    );
    // The markdown keymap surfaced into the behavior manifest (distinct from
    // any Ctrl+O file-open binding, which loadPackage must NOT install).
    assert!(
        manifest
            .keymaps
            .iter()
            .any(|rule| rule.command_id == "markdown.togglePreview"),
        "default load must register the markdown togglePreview keymap, got {:?}",
        manifest
            .keymaps
            .iter()
            .map(|k| &k.command_id)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn default_loading_preserves_explicit_ctrl_o_separation() {
    // Phase 18.6 task 6: loadPackage must NOT install the Ctrl+O file-open
    // binding. That binding stays a separate explicit bindKey call so the
    // package never owns a global file-open key. This test verifies both
    // halves: loadPackage alone installs no clientOpenFileDialog keymap, and
    // adding the documented separate bindKey call does install it.
    let root = config_fixture("loadpackage-no-ctrlo");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
        await loadPackage("@clay/markdown");
        const classification = serverClassifyDocument({ documentId: 1, path: "README.md" });
        serverActivateClassifiedMode(classification, { path: "README.md" });
        "#,
    )
    .unwrap();
    let without_binding = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("loadPackage-only config must load");
    let manifest = without_binding
        .behavior_manifest
        .as_ref()
        .expect("loadPackage must still produce a behavior manifest");
    assert!(
        !manifest
            .keymaps
            .iter()
            .any(|rule| rule.command_id == "documents.clientOpenFileDialog"),
        "loadPackage must NOT install the Ctrl+O file-open keymap; it stays a separate bindKey call, got {:?}",
        manifest
            .keymaps
            .iter()
            .map(|k| &k.command_id)
            .collect::<Vec<_>>()
    );

    // The documented default adds the Ctrl+O binding as a separate explicit
    // bindKey call after loadPackage, and it lands in the manifest.
    let root = config_fixture("loadpackage-with-ctrlo");
    fs::write(
        root.join("init.js"),
        r#"
        import { loadPackage } from "clay:packages";
        import { bindKey } from "clay:keybindings";
        import { serverActivateClassifiedMode, serverClassifyDocument } from "clay:modes";
        await loadPackage("@clay/markdown");
        const classification = serverClassifyDocument({ documentId: 1, path: "README.md" });
        serverActivateClassifiedMode(classification, { path: "README.md" });
        bindKey("Ctrl+O", "documents.clientOpenFileDialog", { scope: "editor" });
        "#,
    )
    .unwrap();
    let with_binding = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("loadPackage + bindKey config must load");
    let manifest = with_binding
        .behavior_manifest
        .as_ref()
        .expect("config with bindKey must produce a behavior manifest");
    assert!(
        manifest
            .keymaps
            .iter()
            .any(|rule| rule.command_id == "documents.clientOpenFileDialog"),
        "the separate bindKey call must install the Ctrl+O file-open keymap, got {:?}",
        manifest
            .keymaps
            .iter()
            .map(|k| &k.command_id)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn preferences_override_init_js_theme_on_reload() {
    // Precedence: init.js setTheme < persisted UI-session theme. The
    // preferences.json theme is applied AFTER init.js so the UI choice wins.
    let root = config_fixture("preferences-override-init-theme");
    fs::write(
        root.join("init.js"),
        r#"
        import { setTheme } from "clay:theme";
        setTheme("@clay/theme-gruvbox-material-light");
        "#,
    )
    .unwrap();
    fs::write(
        root.join("preferences.json"),
        r#"{"theme":"@clay/theme-modus-vivendi"}"#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("preferences + init.js config must load");
    let theme = result.active_theme.expect("active theme emitted");
    assert_eq!(theme.specifier, "@clay/theme-modus-vivendi");
    assert_eq!(theme.overrides.len(), 49);
}

#[tokio::test]
async fn preferences_appearance_applies_when_init_js_is_silent() {
    // No init.js theme; preferences.appearance drives the canonical default.
    let root = config_fixture("preferences-appearance-only");
    fs::write(
        root.join("init.js"),
        "// silent
",
    )
    .unwrap();
    fs::write(root.join("preferences.json"), r#"{"appearance":"light"}"#).unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("appearance preference config must load");
    let theme = result.active_theme.expect("canonical default emitted");
    assert_eq!(theme.specifier, "@clay/theme-modus-operandi");
}

#[tokio::test]
async fn preferences_theme_beats_appearance_canonical_default() {
    // Both theme and appearance persisted: explicit theme wins (applied
    // first, marks explicit; appearance apply does not re-resolve).
    let root = config_fixture("preferences-theme-over-appearance");
    fs::write(
        root.join("init.js"),
        "// silent
",
    )
    .unwrap();
    fs::write(
        root.join("preferences.json"),
        r#"{"theme":"@clay/theme-gruvbox-material-dark","appearance":"light"}"#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("theme+appearance preference config must load");
    let theme = result.active_theme.expect("active theme emitted");
    assert_eq!(theme.specifier, "@clay/theme-gruvbox-material-dark");
}

#[tokio::test]
async fn preferences_typography_round_trips_through_reload() {
    let root = config_fixture("preferences-typography-roundtrip");
    fs::write(
        root.join("init.js"),
        "// silent
",
    )
    .unwrap();
    fs::write(
        root.join("preferences.json"),
        r#"{"typography":{"monospace":{"families":["JetBrains Mono","monospace"],"size":18},
           "proportional":{"families":["Inter","sans-serif"],"size":17},
           "ui":{"families":["system-ui"],"size":13}}}"#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("typography preference config must load");
    let typography = result.active_typography.expect("typography emitted");
    assert!(typography.revision >= 1, "revision assigned on apply");
    assert_eq!(typography.monospace.families[0], "JetBrains Mono");
    assert!((typography.monospace.size - 18.0).abs() < f32::EPSILON);
    assert!((typography.proportional.size - 17.0).abs() < f32::EPSILON);
    assert_eq!(typography.ui.families, ["system-ui"]);
}

#[tokio::test]
async fn invalid_preferences_theme_falls_back_safely_with_diagnostic() {
    // A corrupted theme field is dropped; init.js / canonical default applies.
    let root = config_fixture("preferences-invalid-theme-fallback");
    fs::write(
        root.join("init.js"),
        "// silent
",
    )
    .unwrap();
    fs::write(
        root.join("preferences.json"),
        r#"{"theme":"@vendor/evil","appearance":"dark"}"#,
    )
    .unwrap();
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("invalid-preference config must still load");
    // Invalid theme dropped; appearance=dark resolves the canonical default.
    let theme = result.active_theme.expect("canonical dark default emitted");
    assert_eq!(theme.specifier, "@clay/theme-modus-vivendi");
    assert!(
        result
            .op_records
            .iter()
            .any(|record| record.contains("preferences:"))
            || result
                .op_records
                .iter()
                .any(|record| record.contains("preferences.json")),
        "invalid preference field must record a diagnostic, got {:?}",
        result.op_records
    );
}

#[tokio::test]
async fn no_preferences_lets_init_js_win() {
    let root = config_fixture("preferences-absent-init-wins");
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
        .expect("init.js-only config must load");
    let theme = result.active_theme.expect("active theme emitted");
    assert_eq!(theme.specifier, "@clay/theme-gruvbox-material-light");
}
