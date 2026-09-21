use super::*;

#[tokio::test]
async fn js_parse_handler_timeout_uses_registered_budget() {
    let service = ClayJsRuntimeService::default();
    let evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@clay/loop",
            "loop",
            &["parse-document"],
            serde_json::json!({}),
        ),
        vec![crate::packages::permissions::PackagePermission::ParseDocument],
        r#"
            import { serverRegisterParseHandler } from "clay:parse";
            const parser = { parse() { while (true) {} } };
            serverRegisterParseHandler({
              mode: "loop",
              parseUnit: "line-group",
              timeoutMs: 50,
              module: parser,
              exportName: "parse"
            });
            "#,
    )
    .await
    .expect("malicious handler registration itself should be bounded metadata work");
    let registration = evaluation
        .js_parse_handlers
        .first()
        .expect("handler registered")
        .clone();
    let notification = ParseEditNotification {
        document_id: 1,
        document_version: 1,
        behavior_version: 1,
        package_prefix: "loop".to_string(),
        mode_id: "loop".to_string(),
        viewport: ParseByteRange::new(0, 4),
        invalidated_ranges: vec![ParseByteRange::new(0, 4)],
        accepted_edit: None,
        parse_windows: Vec::new(),
        memory_budget: None,
        trace_id: None,
        request_id: None,
    };
    let started = std::time::Instant::now();
    let error = service
        .invoke_parse_handler(registration, notification)
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Timeout));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "registered handler timeout budget should beat global 5s guard"
    );
    assert_eq!(error.diagnostic().code, "runtime.timeout");
}

#[tokio::test]
async fn runtime_boundary_does_not_expose_platform_authorities() {
    let service = ClayJsRuntimeService::default();
    let result = service
        .evaluate_controlled_module(
            r#"
            const exposed = [
              ["fetch", typeof fetch],
              ["WebSocket", typeof WebSocket],
              ["Worker", typeof Worker],
              ["process", typeof process],
              ["require", typeof require],
              ["Deno.readTextFile", typeof Deno.readTextFile],
              ["Deno.Command", typeof Deno.Command],
            ].filter(([, type]) => type !== "undefined");
            Deno.core.ops.op_clay_runtime_record(JSON.stringify(exposed));
            "#,
        )
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["[]"]);
}

#[tokio::test]
async fn js_runtime_infinite_loop_is_terminated_with_timeout() {
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
    let start = std::time::Instant::now();
    let error = service
        .evaluate_controlled_module(r#"while (true) {}"#)
        .await
        .unwrap_err();
    let elapsed = start.elapsed();

    assert!(
        matches!(error, ClayRuntimeError::Timeout),
        "expected Timeout, got {error:?}"
    );
    assert!(
        elapsed < Duration::from_millis(1000),
        "timeout test should finish quickly, took {elapsed:?}"
    );
    assert_eq!(
        error.diagnostic().code,
        "runtime.timeout",
        "timeout should surface the runtime.timeout diagnostic"
    );
    // Timed-out evaluations are not counted as successful completions.
    assert_eq!(service.evaluation_count(), 0);
}

#[tokio::test]
async fn js_runtime_timeout_recovery_uses_fresh_worker() {
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
    let error = service
        .evaluate_controlled_module(
            r#"
            globalThis.__clayRecoveryMarker = "stale";
            while (true) {}
            "#,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, ClayRuntimeError::Timeout));

    let result = service
        .evaluate_controlled_module(
            r#"
            Deno.core.ops.op_clay_runtime_record(typeof globalThis.__clayRecoveryMarker);
            "#,
        )
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["undefined"]);
    assert_eq!(service.evaluation_count(), 1);
}

#[tokio::test]
async fn js_runtime_heap_growth_is_terminated_with_heap_limit_diagnostic() {
    let service =
        ClayJsRuntimeService::with_timeout_and_heap_limit(Duration::from_secs(3), 8 * 1024 * 1024);
    let error = service
        .evaluate_controlled_module(
            r#"
            const values = [];
            while (true) {
              values.push({ text: "Hello", number: values.length });
            }
            "#,
        )
        .await
        .unwrap_err();

    assert!(
        matches!(error, ClayRuntimeError::HeapLimit),
        "expected heap limit, got {error:?}"
    );
    assert_eq!(error.diagnostic().code, "runtime.heap_limit");
    assert_eq!(service.evaluation_count(), 0);
}

#[tokio::test]
async fn js_runtime_heap_limit_recovery_uses_fresh_worker() {
    let service =
        ClayJsRuntimeService::with_timeout_and_heap_limit(Duration::from_secs(3), 8 * 1024 * 1024);
    let error = service
        .evaluate_controlled_module(
            r#"
            globalThis.__clayRecoveryMarker = "stale";
            const values = [];
            while (true) {
              values.push({ text: "Hello", number: values.length });
            }
            "#,
        )
        .await
        .unwrap_err();
    assert!(matches!(error, ClayRuntimeError::HeapLimit));

    let result = service
        .evaluate_controlled_module(
            r#"
            Deno.core.ops.op_clay_runtime_record(typeof globalThis.__clayRecoveryMarker);
            "#,
        )
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["undefined"]);
    assert_eq!(service.evaluation_count(), 1);
}

/// Plan 127 P5: v8 latches whatever limit a near-heap callback returns, so an
/// evaluation that brushed the cap but still finished used to leave the isolate
/// — and every later evaluation — running with a doubled (and re-doubling)
/// package memory ceiling. The worker must snap the limit back to the
/// configured budget before the next evaluation runs.
#[test]
fn near_heap_limit_recovers_with_original_cap() {
    let heap_limit_bytes = 8 * 1024 * 1024;
    let ratchet_limit_bytes = heap_limit_bytes * 64;
    let op_state = Arc::new(crate::server::ops::ClayOpState::new_for_document(
        Arc::new(Mutex::new(WorkspaceState::new())),
        1,
    ));
    let main_specifier = ModuleSpecifier::parse(CONTROLLED_MAIN_SPECIFIER).unwrap();
    let loader = Rc::new(ClayModuleLoader::new(
        main_specifier,
        None,
        None,
        op_state.load_entry_allowlist(),
        crate::packages::bundled::RuntimeDomain::Trusted,
    ));
    let (mut runtime, heap_limit_hit) = create_js_runtime(
        Arc::clone(&op_state),
        Rc::clone(&loader),
        heap_limit_bytes,
        crate::packages::bundled::RuntimeDomain::Trusted,
    );
    // The worker drives evaluations on its own current-thread runtime; this test
    // runs outside one so it does the same.
    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test JS runtime tokio runtime must build");

    // Synthetic near-heap event: report the hit and raise the limit instead of
    // terminating, so a package that brushes the cap and still completes leaves
    // the isolate exactly as production would after such a miss.
    let ratchet = Arc::new(std::sync::Mutex::new(Vec::new()));
    let ratchet_records = Arc::clone(&ratchet);
    runtime.add_near_heap_limit_callback(move |current_limit, initial_limit| {
        ratchet_records
            .lock()
            .expect("ratchet record mutex poisoned")
            .push((current_limit, initial_limit));
        ratchet_limit_bytes
    });

    let evaluate = |runtime: &mut deno_core::JsRuntime, script: &str, evaluation_id: u64| {
        let loaded = prepare_runtime_entry(
            RuntimeEntry::ControlledSource(script.to_string()),
            evaluation_id,
        )
        .expect("controlled entry must prepare");
        loader.set_entry(
            loaded.main_specifier.clone(),
            loaded.main_source.clone(),
            loaded.configuration.clone(),
        );
        tokio_runtime.block_on(evaluate_loaded_module(
            runtime,
            &op_state,
            loaded,
            Duration::from_millis(JS_RUNTIME_EVALUATION_TIMEOUT_MS),
            false,
            &heap_limit_hit,
        ))
    };

    // Evaluation 1 allocates past the cap, so the callback fires and raises the
    // limit while the evaluation continues and completes.
    let (response, mut received) = tokio::sync::oneshot::channel();
    let poisoned = run_bounded_evaluation(
        &mut runtime,
        &op_state,
        heap_limit_bytes,
        &heap_limit_hit,
        None,
        |runtime| {
            evaluate(
                runtime,
                r#"
                const blocks = [];
                for (let i = 0; i < 400000; i++) {
                  blocks.push({ text: "Hello", number: i });
                }
                globalThis.__clayHeapBlocks = null;
                "#,
                1,
            )
        },
        response,
    );
    received
        .try_recv()
        .expect("worker must respond")
        .expect("near-miss evaluation must complete");
    let (ratchet_observed, used_after_recovery) = {
        let records = ratchet
            .lock()
            .expect("ratchet record mutex poisoned")
            .clone();
        let used = runtime.v8_isolate().get_heap_statistics().used_heap_size();
        assert!(
            !poisoned,
            "a brushed-but-completed evaluation must not poison the worker: {records:?}"
        );
        assert!(
            !records.is_empty(),
            "the synthetic near-heap event must fire"
        );
        assert_eq!(
            records[0].0, records[0].1,
            "the first near-heap hit must be measured against the initial cap"
        );
        (records, used)
    };

    // Evaluation 2 probes whichever limit is now in effect: the callback reports
    // the limit v8 hands it, terminates, and flags the hit like production does.
    let probe = Arc::new(std::sync::Mutex::new(Vec::new()));
    let probe_records = Arc::clone(&probe);
    let probe_flag = Arc::clone(&heap_limit_hit);
    let probe_terminate = runtime.v8_isolate().thread_safe_handle();
    runtime.add_near_heap_limit_callback(move |current_limit, _initial_limit| {
        probe_records
            .lock()
            .expect("probe record mutex poisoned")
            .push(current_limit);
        probe_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        probe_terminate.terminate_execution();
        current_limit.saturating_mul(2)
    });

    let (response, mut received) = tokio::sync::oneshot::channel();
    let poisoned = run_bounded_evaluation(
        &mut runtime,
        &op_state,
        heap_limit_bytes,
        &heap_limit_hit,
        None,
        |runtime| {
            evaluate(
                runtime,
                r#"
                const blocks = [];
                for (let i = 0; i < 1000000; i++) {
                  blocks.push({ text: "Hello", number: i });
                }
                globalThis.__clayHeapBlocks = blocks;
                "#,
                2,
            )
        },
        response,
    );
    let second = received.try_recv().expect("worker must respond");
    let probe_observed = probe.lock().expect("probe record mutex poisoned").clone();
    let recovered_cap = probe_observed.first().copied().unwrap_or(0);
    println!(
        "PLAN127_HEAP configured={heap_limit_bytes} ratchet={ratchet_limit_bytes} ratchet_hits={ratchet_observed:?} used_after_recovery={used_after_recovery} recovered_cap={recovered_cap}"
    );
    assert!(
        !probe_observed.is_empty(),
        "the evaluation after the near-miss must still trip the cap: {second:?}"
    );
    assert!(
        poisoned && matches!(second, Err(ClayRuntimeError::HeapLimit)),
        "growing past the recovered cap must poison the worker: {second:?}"
    );
    assert!(
        recovered_cap <= ratchet_limit_bytes / 8,
        "the evaluation after the near-miss must not inherit the {ratchet_limit_bytes} byte ratchet: {probe_observed:?}"
    );
    assert!(
        recovered_cap <= used_after_recovery * 2,
        "the recovered cap must track the live heap ({used_after_recovery} bytes), not the \\
         {ratchet_limit_bytes} byte ratchet: {probe_observed:?}"
    );
}

#[tokio::test]
async fn js_runtime_short_timeout_does_not_break_fast_evaluation() {
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
    let result = service
        .evaluate_controlled_module(
            r#"
            Deno.core.ops.op_clay_runtime_record("fast");
            "#,
        )
        .await
        .unwrap();

    assert_eq!(result.op_records, vec!["fast"]);
    assert_eq!(service.evaluation_count(), 1);
}

#[tokio::test]
async fn js_runtime_rejects_unsafe_or_unknown_imports() {
    let service = ClayJsRuntimeService::default();
    let error = service
        .evaluate_controlled_module(r#"import "https://example.invalid/module.js";"#)
        .await
        .unwrap_err();

    assert!(matches!(error, ClayRuntimeError::Runtime(_)));
    assert!(error.to_string().contains("runtime.invalid_import"));
    assert_eq!(service.evaluation_count(), 0);
}
