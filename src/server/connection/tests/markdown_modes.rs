use super::*;

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
    crate::server::runtime_state::apply_runtime_outputs(&evaluation, 1, &behavior, &sdui).await;
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
    let messages = super::super::open_document_followup_messages(
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
        super::super::schedule_parse_window(
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
        super::super::schedule_parse_window(
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
        super::super::schedule_parse_window(
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
    let source = include_str!("../mod.rs");
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
    let activation = super::super::classify_open_document(
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
    let first = super::super::classify_open_document(
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

    let second = super::super::classify_open_document(
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
        super::super::classify_open_document(
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
    super::super::classify_open_document(
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
    super::super::classify_open_document(
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
