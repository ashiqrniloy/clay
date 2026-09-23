use super::*;

#[tokio::test]
async fn plan112_load_then_select_recommended_path_activates_pack() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(plan112_icon_fixture("load-select"))
        .await
        .expect("documented loadPackage + setIconPack example must succeed");

    let pack = result
        .active_icon_pack
        .expect("explicit selection emits an active icon-pack snapshot");
    assert_eq!(pack.specifier, "@clay/icons-phosphor-regular");
    assert_eq!(pack.icons.len(), 22);
    assert!(
        result
            .op_records
            .iter()
            .any(|record| record == "icons:@clay/icons-phosphor-regular:22:v1"),
        "load+select summary must reach configuration evaluation"
    );
}

#[tokio::test]
async fn plan112_both_packs_loaded_either_order_explicit_selection_wins() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;

    for (load_order, selection) in [
        ("regular", "duotone"),
        ("duotone", "regular"),
        ("regular", "regular"),
        ("duotone", "duotone"),
    ] {
        let root = config_fixture("plan112-both-packs-order");
        fs::write(
            root.join("init.js"),
            format!(
                r#"
                import {{ loadPackage }} from "clay:packages";
                import {{ setIconPack }} from "clay:theme";

                await loadPackage("@clay/icons-phosphor-{load_order}");
                await loadPackage("@clay/icons-phosphor-{selection}");
                setIconPack("@clay/icons-phosphor-{selection}");
                "#
            ),
        )
        .unwrap();

        let result = ClayJsRuntimeService::default()
            .load_configuration_from_root(root)
            .await
            .unwrap_or_else(|error| {
                panic!("load order {load_order} -> select {selection} must succeed: {error}")
            });

        let pack = result
            .active_icon_pack
            .expect("explicit selection must emit an active icon-pack snapshot");
        assert_eq!(
            pack.specifier,
            format!("@clay/icons-phosphor-{selection}"),
            "load order must not influence the active pack"
        );
        assert_eq!(pack.icons.len(), 22);
    }
}

#[tokio::test]
async fn plan112_modular_import_selection_and_unchanged_reload_behavior() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = plan112_icon_fixture("modular");
    let runtime = ClayJsRuntimeService::default();

    let first = runtime
        .load_configuration_from_root(root.clone())
        .await
        .expect("modular icon selection must succeed");
    let pack = first
        .active_icon_pack
        .expect("modular import selection emits an active icon-pack snapshot");
    assert_eq!(pack.specifier, "@clay/icons-phosphor-duotone");
    assert!(
        first
            .op_records
            .iter()
            .any(|record| *record == "icons:@clay/icons-phosphor-duotone:22:v1"),
        "modular import summary must reach configuration evaluation"
    );

    // Unchanged configuration re-load: the serialized reload path must not
    let second = runtime
        .load_configuration_from_root(root)
        .await
        .expect("unchanged configuration re-load must succeed");
    let repack = second
        .active_icon_pack
        .as_ref()
        .expect("active icon pack persists across an unchanged re-load");
    assert_eq!(repack.specifier, "@clay/icons-phosphor-duotone");
    assert!(
        second.op_records.is_empty(),
        "unchanged config must not re-execute or re-send unchanged icon state"
    );
}

#[tokio::test]
async fn plan112_persisted_icon_pack_preference_reapplies_after_restart() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let root = config_fixture("plan112-restart-preference");
    // Restart reproduction: init.js carries no icon lines; the persisted
    // preference (written by a prior session's selection) re-applies after
    // init.js evaluation on every load, so the selection survives restart.
    fs::write(
        root.join("init.js"),
        "// selection comes from preferences\n",
    )
    .unwrap();
    fs::write(
        root.join("preferences.json"),
        serde_json::json!({ "iconPack": "@clay/icons-phosphor-duotone" }).to_string(),
    )
    .unwrap();

    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(root)
        .await
        .expect("persisted iconPack preference must load");

    let pack = result
        .active_icon_pack
        .expect("persisted selection reapplied after restart");
    assert_eq!(pack.specifier, "@clay/icons-phosphor-duotone");
}

#[tokio::test]
async fn plan112_zero_icon_configuration_keeps_bundled_fallback_without_snapshot() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(plan112_icon_fixture("default"))
        .await
        .expect("zero-icon configuration must load");

    assert!(
        result.active_icon_pack.is_none(),
        "no selection must emit no icon-pack snapshot; the bundled Regular subset stays active"
    );
}

#[tokio::test]
async fn plan112_selecting_unloaded_third_party_pack_fails_closed_to_fallback() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let result = ClayJsRuntimeService::default()
        .load_configuration_from_root(plan112_icon_fixture("deny"))
        .await
        .expect("a denied selection is a sanitized diagnostic, not a configuration failure");

    assert!(
        result.active_icon_pack.is_none(),
        "a failed selection must leave no active icon-pack snapshot (bundled fallback stays active)"
    );
    assert!(
        result
            .op_records
            .iter()
            .any(|record| record.starts_with("denied:Error: theme.load_failed")),
        "the sanitized load_failed rejection must surface to configuration evaluation"
    );
    assert!(
        !result
            .op_records
            .iter()
            .any(|record| record.contains("unexpected-success")),
        "an unloaded third-party pack must never activate"
    );
}
