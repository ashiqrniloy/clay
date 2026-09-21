use super::*;

/// Plan 127 task 6: a registration left behind by a revoked/disabled package is
/// refused identically at every lane. The host gate authenticates the command's
/// package identity before the command reaches a lane, so the refusal is typed
/// (`Revoked`) and costs no isolate work. Baseline with the gate disabled: both
/// lanes already failed closed, but only inside the isolate, through whichever
/// op the generated provider module happened to call
/// (`Runtime("Error: packages.package_not_enabled ...")`) — neither typed nor
/// lane-independent, and only reachable while that generated module keeps
/// checking provenance.
#[tokio::test]
async fn revoked_package_commands_refused_per_lane() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let providers = lane_provider_pair(&service, "lanerevoke").await;
    let [general_provider, latency_provider] = providers.clone();
    let (request, window) = lane_completion_input("lanerevoke", 1, "lane");

    // Both lanes serve while their package is enabled.
    for (provider, expected) in [
        (general_provider.clone(), "inline"),
        (latency_provider.clone(), "module"),
    ] {
        let result = service
            .invoke_completion_provider(provider, request.clone(), window.clone())
            .await
            .expect("enabled packages serve both lanes");
        assert_eq!(result.items[0].label, expected);
    }
    let workers_before = service.workers_started();

    // Production revocation sequence (`clay package revoke <name>`): the
    // approval is revoked and the package disabled. The runtime keeps the
    // coordinator's registration, which must now refuse in every lane.
    {
        let op_state = service.test_op_state();
        let mut package_service = op_state
            .package_service()
            .lock()
            .expect("package service mutex poisoned");
        for package in ["@vendor/lanerevoke", "@vendor/lanerevokelat"] {
            package_service
                .revoke_package_approval(package)
                .expect("approval revoke");
            package_service.disable(package).expect("package disable");
        }
    }
    for (provider, expected_package, lane) in [
        (general_provider, "@vendor/lanerevoke", "general"),
        (latency_provider, "@vendor/lanerevokelat", "latency"),
    ] {
        let error = service
            .invoke_completion_provider(provider, request.clone(), window.clone())
            .await
            .expect_err("revoked package must not serve completions");
        assert!(
            matches!(
                &error,
                ClayRuntimeError::Revoked { package, version }
                    if package == expected_package && version == "0.1.0"
            ),
            "{lane} lane must refuse with the typed revocation error, got {error:?}"
        );
    }
    assert_eq!(
        service.workers_started(),
        workers_before,
        "a refusal is host-side: no lane may be replaced"
    );
}

/// Plan 127 task 6: a trusted reload rebuilds the trusted domain's lanes and
/// shares the third-party domain's lanes untouched — both the general-lane
/// (registry) and latency-lane (module) registrations keep serving from their
/// original isolates, without starting or replacing a worker.
#[tokio::test]
async fn reload_shares_third_party_lanes_untouched() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();
    let providers = lane_provider_pair(&service, "lanereload").await;
    let third_party_generation_before =
        service.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty);

    let reloaded = ClayJsRuntimeService::production_reload(&service);
    assert_eq!(
        reloaded.domain_generation(crate::packages::bundled::RuntimeDomain::ThirdParty),
        third_party_generation_before,
        "trusted reload must not replace the third-party domain"
    );
    assert_eq!(
        reloaded.workers_started(),
        crate::perf::budgets::JS_RUNTIME_LANES_PER_DOMAIN as u64,
        "reload rebuilds exactly the trusted domain's lanes through one construction path"
    );

    // Reload commit re-registers the survived third-party payload; both lanes
    // must still answer from their untouched isolates.
    let snapshot = reloaded.third_party_registrations_snapshot();
    assert_eq!(snapshot.js_completion_providers.len(), providers.len());
    let oracle = crate::server::completion::CompletionCoordinator::new();
    reloaded
        .register_completion_providers(&oracle, 5, &snapshot)
        .expect("surviving third-party registrations re-register after reload");
    let (request, window) = lane_completion_input("lanereload", 2, "lane");
    for (provider, expected) in providers.into_iter().zip(["inline", "module"]) {
        let result = reloaded
            .invoke_completion_provider(provider, request.clone(), window.clone())
            .await
            .expect("surviving third-party lanes serve after a trusted reload");
        assert_eq!(result.items[0].label, expected);
    }
    assert_eq!(
        reloaded.workers_started(),
        crate::perf::budgets::JS_RUNTIME_LANES_PER_DOMAIN as u64,
        "serving both surviving lanes must not start a worker"
    );
}

/// Plan 127 task 6: the installed op inventory of a running lane is the
/// domain's op set — never a lane-specific one. Enumerated from inside each of
/// the four live isolates through the real dispatch path, so it is ground
/// truth for the running lanes rather than a claim about the wiring code.
#[tokio::test]
async fn lanes_share_their_domain_op_set() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    use crate::packages::bundled::RuntimeDomain;

    async fn lane_op_names(
        service: &ClayJsRuntimeService,
        domain: RuntimeDomain,
        lane: RuntimeLane,
    ) -> Vec<String> {
        let (response, receiver) = tokio::sync::oneshot::channel();
        let evaluation = service
            .dispatch_to_domain(
                domain,
                lane,
                super::super::worker::RuntimeCommand::Evaluate {
                    entry: RuntimeEntry::ControlledSource(
                        "Deno.core.ops.op_clay_runtime_record(Object.keys(Deno.core.ops).sort().join(','));"
                            .to_string(),
                    ),
                    workspace: None,
                    runtime_document_id: 1,
                    package_context: None,
                    metric: "runtime.lane_op_names",
                    response,
                },
                receiver,
            )
            .await
            .expect("every lane accepts a controlled op-inventory probe");
        evaluation
            .op_records
            .first()
            .expect("op inventory probe records its result")
            .split(',')
            .map(str::to_string)
            .collect()
    }

    let service = ClayJsRuntimeService::default();
    let trusted_general =
        lane_op_names(&service, RuntimeDomain::Trusted, RuntimeLane::General).await;
    let trusted_latency =
        lane_op_names(&service, RuntimeDomain::Trusted, RuntimeLane::Latency).await;
    let third_party_general =
        lane_op_names(&service, RuntimeDomain::ThirdParty, RuntimeLane::General).await;
    let third_party_latency =
        lane_op_names(&service, RuntimeDomain::ThirdParty, RuntimeLane::Latency).await;

    let trusted_only: Vec<&String> = trusted_general
        .iter()
        .filter(|op| !third_party_general.contains(op))
        .collect();
    println!(
        "PLAN127_LANES trusted_ops={} third_party_ops={} trusted_only={}",
        trusted_general.len(),
        third_party_general.len(),
        trusted_only.len()
    );
    assert!(!trusted_general.is_empty() && !third_party_general.is_empty());
    assert_eq!(
        trusted_general, trusted_latency,
        "both trusted lanes must install the trusted domain's op set"
    );
    assert_eq!(
        third_party_general, third_party_latency,
        "both third-party lanes must install the third-party domain's op set"
    );
    for op in [
        "op_clay_runtime_ping",
        "op_clay_configuration_get_state",
        "op_clay_documents_open_document",
        "op_clay_packages_load_package",
        "op_clay_packages_load_in_package_domain",
        "op_clay_language_server_authorize",
        "op_clay_modes_classify_document",
        "op_clay_theme_set_theme",
    ] {
        assert!(
            trusted_general.iter().any(|name| name == op),
            "trusted lane must install {op}"
        );
        assert!(
            !third_party_latency.iter().any(|name| name == op),
            "third-party latency lane must not install {op}"
        );
    }
    assert_eq!(
        service.workers_started(),
        2 * crate::perf::budgets::JS_RUNTIME_LANES_PER_DOMAIN as u64,
        "one construction path starts every lane of both domains"
    );
}

/// Plan 127 P1 acceptance: a slow general-lane command must not head-of-line
/// block latency-lane completions. The parse handler busy-loop holds the
/// domain's general lane; the module-backed completion provider is served by
/// the latency lane and answers while that handler is still running.
#[tokio::test]
async fn latency_lane_unblocked_by_busy_general_lane() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "lanehol";
    let busy_ms = 500u64;
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(2_000));
    let (_, completion_registration) = module_backed_completion_provider(
        &service,
        "@vendor/lanehol",
        prefix,
        r#"
        export function provideCompletion(_request, window) {
          return {
            status: "ok",
            items: [{ label: "lane", insertText: "lane", detail: window.text }]
          };
        }
        "#,
    )
    .await;
    // Inline (token-backed) parse handler: it keeps the general lane busy and
    // stays on that lane by design.
    let parse_evaluation = evaluate_as_package(
        &service,
        test_package_json(
            "@clay/lanehol-parser",
            "laneholparse",
            &["parse-document"],
            serde_json::json!({}),
        ),
        vec![crate::packages::permissions::PackagePermission::ParseDocument],
        &format!(
            r#"
            import {{ serverRegisterParseHandler }} from "clay:parse";
            serverRegisterParseHandler({{
              mode: "laneholparse",
              runtimeBridge: true,
              timeoutMs: 2_000,
              module: {{ default: async (notification) => {{
                const start = Date.now();
                while (Date.now() - start < {busy_ms}) {{}}
                return {{ viewport: notification.viewport }};
              }} }}
            }});
            "#
        ),
    )
    .await
    .expect("inline busy parse handler registers");
    let parse_registration = parse_evaluation
        .js_parse_handlers
        .first()
        .cloned()
        .expect("busy parse handler registered");
    assert_eq!(
        service.registration_domain(&parse_registration.package),
        service.registration_domain(&completion_registration.package),
        "the baseline scenario keeps both commands in one trust domain"
    );
    let parse_notification = ParseEditNotification {
        document_id: 7,
        document_version: 3,
        behavior_version: 1,
        package_prefix: "laneholparse".to_string(),
        mode_id: "laneholparse".to_string(),
        viewport: ParseByteRange::new(0, 2),
        invalidated_ranges: vec![ParseByteRange::new(0, 2)],
        accepted_edit: None,
        parse_windows: Vec::new(),
        memory_budget: None,
        trace_id: None,
        request_id: None,
    };

    // Warm the latency lane (module import) and take an idle baseline.
    let mut idle_us = Vec::new();
    for request_id in 0..10u64 {
        let (request, window) = lane_completion_input(prefix, request_id, "fn");
        let start = Instant::now();
        let result = service
            .invoke_completion_provider(completion_registration.clone(), request, window)
            .await
            .expect("idle completion");
        idle_us.push(start.elapsed().as_micros());
        assert_eq!(result.items[0].label, "lane");
    }
    idle_us.sort_unstable();
    let idle_median_us = idle_us[idle_us.len() / 2];

    // Hold the general lane for `busy_ms`, then issue one completion 50 ms in:
    // at least 450 ms of the busy loop remain if the lanes shared a worker.
    let parse_task = tokio::spawn({
        let service = service.clone();
        let registration = parse_registration.clone();
        let notification = parse_notification.clone();
        async move {
            service
                .invoke_parse_handler(registration, notification)
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let workers_before_completion = service.workers_started();
    let (request, window) = lane_completion_input(prefix, 50, "fn");
    let start = Instant::now();
    let completion = service
        .invoke_completion_provider(completion_registration.clone(), request, window)
        .await
        .expect("latency-lane completion");
    let busy_completion_us = start.elapsed().as_micros();
    assert_eq!(completion.items[0].label, "lane");
    assert_eq!(
        service.workers_started(),
        workers_before_completion,
        "a slow general-lane parse must not replace either lane"
    );
    assert!(
        busy_completion_us < 250_000,
        "completion must not queue behind the busy general lane: {busy_completion_us} us \
         (busy {busy_ms} ms, idle median {idle_median_us} us)"
    );
    let parse_result = parse_task.await.expect("parse task join");
    assert!(
        parse_result.is_ok(),
        "busy parse must complete under its timeout: {parse_result:?}"
    );
    eprintln!(
        "PLAN127_LANE busy_ms={busy_ms} idle_median_us={idle_median_us} \
         busy_completion_us={busy_completion_us} workers_started={}",
        service.workers_started()
    );
}

/// Plan 127 P1: a timeout poison on the latency lane replaces ONLY that
/// lane's worker. The general lane keeps serving, the domain generation
/// (registration ownership metadata) is untouched, and the replaced lane
/// rematerializes the module-backed handler by import — no evaluation replay.
#[tokio::test]
async fn lane_poison_replaces_only_that_lane() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "lanepoison";
    let service = ClayJsRuntimeService::with_timeout(Duration::from_millis(150));
    let (_, registration) = module_backed_completion_provider(
        &service,
        "@vendor/lanepoison",
        prefix,
        r#"
        export function provideCompletion(_request, window) {
          if (window.text === "loop") {
            while (true) {}
          }
          return { status: "ok", items: [{ label: "alive", insertText: "alive" }] };
        }
        "#,
    )
    .await;
    let domain = service.registration_domain(&registration.package);
    let baseline_workers = service.workers_started();
    let baseline_generation = service.domain_generation(domain);

    // Latency-lane timeout: replacement is lazy, so nothing starts yet.
    let (request, window) = lane_completion_input(prefix, 1, "loop");
    let timed_out = service
        .invoke_completion_provider(registration.clone(), request, window)
        .await;
    assert!(
        matches!(timed_out, Err(ClayRuntimeError::Timeout)),
        "runaway provider must time out, got {timed_out:?}"
    );
    assert_eq!(service.workers_started(), baseline_workers);

    // The general lane is untouched: it still serves and starts no worker.
    service
        .evaluate_third_party_module("Deno.core.ops.op_clay_runtime_record('general');")
        .await
        .expect("general lane serves after a latency-lane timeout");
    assert_eq!(
        service.workers_started(),
        baseline_workers,
        "general lane must not be replaced"
    );
    assert_eq!(
        service.domain_generation(domain),
        baseline_generation,
        "latency poison must not bump the domain generation"
    );

    // Next latency dispatch replaces only the latency lane; the module-backed
    // handler re-imports from the shared allowlist.
    let (request, window) = lane_completion_input(prefix, 2, "alive");
    let result = service
        .invoke_completion_provider(registration, request, window)
        .await
        .expect("latency lane serves after replacing only itself");
    assert_eq!(result.items[0].label, "alive");
    assert_eq!(service.workers_started(), baseline_workers + 1);
    assert_eq!(service.domain_generation(domain), baseline_generation);

    // General lane still alive without replacement.
    service
        .evaluate_third_party_module("Deno.core.ops.op_clay_runtime_record('general');")
        .await
        .expect("general lane still serves after the latency replacement");
    assert_eq!(service.workers_started(), baseline_workers + 1);
}

/// Plan 127 P1 / Plan 061 parity: a THIRD-PARTY latency lane gets the same
/// third-party extension set as its general lane — trusted-only bridge ops are
/// absent — and it is a separate isolate (general-lane globals invisible).
#[tokio::test]
async fn third_party_lane_denies_trusted_ops() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "lanedenial";
    let service = ClayJsRuntimeService::default();
    let (_, registration) = module_backed_completion_provider(
        &service,
        "@vendor/lanedenial",
        prefix,
        r#"
        export function provideCompletion(_request, _window) {
          const ops = globalThis.Deno?.core?.ops ?? {};
          return {
            status: "ok",
            items: [{
              label: `trustedBridge=${typeof ops.op_clay_packages_load_in_package_domain !== "undefined"};generalGlobal=${globalThis.__laneProbe === "general"}`,
              insertText: "lane"
            }]
          };
        }
        "#,
    )
    .await;
    let domain = service.registration_domain(&registration.package);
    assert_eq!(domain, crate::packages::bundled::RuntimeDomain::ThirdParty);
    // Mark the general-lane isolate; the latency lane must not see this global.
    service
        .evaluate_third_party_module("globalThis.__laneProbe = 'general';")
        .await
        .expect("general lane accepts the probe global");
    let (request, window) = lane_completion_input(prefix, 1, "fn");
    let result = service
        .invoke_completion_provider(registration, request, window)
        .await
        .expect("third-party latency-lane completion");
    assert_eq!(
        result.items[0].label, "trustedBridge=false;generalGlobal=false",
        "third-party latency lane must keep the third-party op set and stay isolate-separate"
    );
}

/// Plan 127 P1: lane count and per-lane heap ceilings are server-owned
/// budgets, and the latency lane never exceeds the configured service ceiling
/// (synthetic small-heap services stay bounded on every lane).
#[test]
fn lane_heap_limits_stay_within_the_configured_budget() {
    use crate::perf::budgets::{
        JS_RUNTIME_HEAP_LIMIT_BYTES, JS_RUNTIME_LANES_PER_DOMAIN,
        JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES,
    };

    assert_eq!(
        super::super::RuntimeLane::ALL.len(),
        JS_RUNTIME_LANES_PER_DOMAIN
    );
    assert_eq!(
        super::super::lane_heap_limit_bytes(JS_RUNTIME_HEAP_LIMIT_BYTES, RuntimeLane::General),
        JS_RUNTIME_HEAP_LIMIT_BYTES
    );
    assert_eq!(
        super::super::lane_heap_limit_bytes(JS_RUNTIME_HEAP_LIMIT_BYTES, RuntimeLane::Latency),
        JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES
    );
    const { assert!(JS_RUNTIME_LATENCY_LANE_HEAP_LIMIT_BYTES < JS_RUNTIME_HEAP_LIMIT_BYTES) };
    assert_eq!(
        super::super::lane_heap_limit_bytes(4 * 1024, RuntimeLane::Latency),
        4 * 1024,
        "a configured ceiling below the latency budget must win"
    );
}

/// Plan 127 P1: a language-intelligence provider that declares its
/// `moduleSpecifier` runs on the domain's latency lane (module import), same
/// as completion. Covers the LI op's specifier validation and the LI
/// evaluation branch; inline `module: {...}` registrations keep the existing
/// general-lane path.
#[tokio::test]
async fn language_intelligence_module_specifier_serves_from_latency_lane() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "laneli";
    let service = ClayJsRuntimeService::default();
    let root = config_fixture("lane-li-provider").join(prefix);
    write_loadable_package(
        &root,
        r#"
        import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
        import * as providerModule from "./intel.js";
        export default function load() {
          serverRegisterLanguageIntelligenceProvider({
            provider: {
              id: "laneli.intelligence",
              features: ["hover"],
              timeoutMs: 500
            },
            module: providerModule,
            moduleSpecifier: import.meta.resolve("./intel.js")
          });
        }
        "#,
    );
    fs::write(
        root.join("dist/intel.js"),
        r#"
        export function provideLanguageIntelligence(request, window) {
          return {
            status: "ok",
            markdown: `lane:${request.feature}:${window.text}`,
            range: { byteStart: 0, byteEnd: window.text.length }
          };
        }
        "#,
    )
    .expect("write module-backed language provider");
    let package_json = test_package_json(
        "@vendor/laneli",
        prefix,
        &["parse-document"],
        serde_json::json!({}),
    );
    let approved = vec![crate::packages::permissions::PackagePermission::ParseDocument];
    ensure_synthetic_package_enabled(&service, package_json.clone(), approved.clone(), None);
    let load_specifier = "clay://packages/@vendor/laneli/dist/load.js";
    service
        .test_op_state()
        .load_entry_allowlist()
        .record_for_package(
            load_specifier,
            root.join("dist/load.js"),
            root.clone(),
            Some("@vendor/laneli"),
        );
    let evaluation = evaluate_as_package(
        &service,
        package_json,
        approved,
        &format!("const m = await import({load_specifier:?}); await m.default();"),
    )
    .await
    .expect("module-backed language provider fixture load");
    let registration = evaluation
        .js_language_intelligence_providers
        .first()
        .cloned()
        .expect("fixture must register a JS language-intelligence provider");
    assert!(
        registration.module_specifier.is_some(),
        "fixture registration must carry the resolved module specifier"
    );
    assert_eq!(
        RuntimeLane::for_provider(registration.module_specifier.as_deref()),
        RuntimeLane::Latency
    );

    let request = crate::protocol::LanguageIntelligenceRequest {
        request_id: 7,
        client_id: 1,
        document_id: 1,
        document_version: 1,
        behavior_version: 1,
        cursor_byte_offset: 0,
        feature: crate::protocol::LanguageIntelligenceFeature::Hover,
        provider_generation: 1,
    };
    let window = crate::server::language_intelligence::LanguageIntelligenceDocumentWindow {
        document_id: 1,
        document_version: 1,
        behavior_version: 1,
        byte_start: 0,
        byte_end: 4,
        text: "fn()".to_string(),
        active_mode: prefix.to_string(),
    };
    let result = service
        .invoke_language_intelligence_provider(registration, request, window)
        .await
        .expect("latency-lane language-intelligence provider");
    crate::server::language_intelligence::validate_result(&result).unwrap();
    assert_eq!(
        result.status,
        crate::protocol::LanguageIntelligenceStatus::Ok
    );
    match result.payload {
        crate::protocol::LanguageIntelligencePayload::Hover(hover) => {
            assert_eq!(hover.markdown, "lane:hover:fn()");
            assert_eq!(hover.range, Some(crate::protocol::TextByteRange::new(0, 4)));
        }
        other => panic!("expected hover payload, got {other:?}"),
    }
}

/// Plan 127 P1 security gate: `moduleSpecifier` must resolve to a loaded
/// module owned by the registering package (same rule as document analyzers);
/// a foreign or unloaded specifier is rejected before any latency lane sees it.
#[tokio::test]
async fn completion_and_language_providers_reject_unowned_module_specifier() {
    let service = ClayJsRuntimeService::default();
    for (api_prefix, source) in [
        (
            "unownedc",
            r#"
            import { serverRegisterCompletionProvider } from "clay:completion";
            serverRegisterCompletionProvider({
              module: { provideCompletion: async () => ({ status: "ok", items: [] }) },
              moduleSpecifier: "clay://packages/other/worker.js"
            });
            "#,
        ),
        (
            "unownedl",
            r#"
            import { serverRegisterLanguageIntelligenceProvider } from "clay:language";
            serverRegisterLanguageIntelligenceProvider({
              provider: { id: "unownedl.intelligence", features: ["hover"] },
              module: { provideLanguageIntelligence: async () => ({ status: "ok" }) },
              moduleSpecifier: "clay://packages/other/worker.js"
            });
            "#,
        ),
    ] {
        let package_json = test_package_json(
            &format!("@vendor/{api_prefix}"),
            api_prefix,
            &["completion-provider", "parse-document"],
            serde_json::json!({
                "completionProviders": [{
                    "id": format!("{api_prefix}.provider"),
                    "triggerCharacters": ["."],
                    "budgets": { "timeoutMs": 50, "maxItems": 8 }
                }]
            }),
        );
        let approved = vec![
            crate::packages::permissions::PackagePermission::CompletionProvider,
            crate::packages::permissions::PackagePermission::ParseDocument,
        ];
        let error = evaluate_as_package(&service, package_json, approved, source)
            .await
            .expect_err("unowned moduleSpecifier must be rejected");
        assert!(
            error
                .to_string()
                .contains("moduleSpecifier must resolve to a loaded module owned by the package"),
            "unexpected moduleSpecifier rejection: {error}"
        );
    }
}

/// Plan 127 P2 acceptance: a typing burst on one document must not grow the
/// lane backlog. Every newer request replaces the older undelivered one for the
/// same work key, older callers learn `Superseded` (never a stale result), and
/// the newest request is served.
#[tokio::test]
async fn queue_bounded_under_flood() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "queueflood";
    let service = ClayJsRuntimeService::default();
    let (_, registration) = module_backed_completion_provider(
        &service,
        "@vendor/queueflood",
        prefix,
        PLAN127_HOLDING_PROVIDER,
    )
    .await;
    let flood = 40_u64;
    let documents = vec![7_u64; flood as usize];
    let outcomes = plan127_completion_burst(&service, &registration, prefix, &documents).await;
    // Synthetic `@vendor/...` packages are adopted third-party packages, so
    // their module-backed provider runs on the third-party latency lane.
    let stats = service.lane_queue_stats(
        crate::packages::bundled::RuntimeDomain::ThirdParty,
        RuntimeLane::Latency,
    );
    assert!(stats.capacity > 4, "budget too small to flood: {stats:?}");
    assert!(
        stats.peak_pending <= 2,
        "one-document flood must collapse onto a single queue slot: {stats:?}"
    );
    assert_eq!(
        stats.evicted, 0,
        "same-key supersede, not capacity eviction: {stats:?}"
    );
    assert!(
        stats.superseded >= flood - 2,
        "newest wins and older undelivered work is dropped: {stats:?}"
    );
    assert_eq!(
        plan127_superseded_count(&outcomes),
        stats.superseded,
        "every dropped command answers Superseded to its caller"
    );
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.is_ok() || matches!(outcome, Err(ClayRuntimeError::Superseded))),
        "dropped work must not surface as another error: {outcomes:?}"
    );
    let labels = plan127_ok_labels(&outcomes);
    assert!(
        labels.contains(&format!("q{flood}")),
        "newest request must be served: {labels:?}"
    );
    assert_eq!(
        labels.len() as u64,
        flood - stats.superseded,
        "served + superseded must account for every request"
    );
    println!(
        "PLAN127_QUEUE_FLOOD requests={flood} peak_pending={} capacity={} superseded={} evicted={} served={}",
        stats.peak_pending,
        stats.capacity,
        stats.superseded,
        stats.evicted,
        labels.len()
    );
    let _ = fs::remove_dir_all(config_fixture("lane-module-provider"));
}

/// Plan 127 P2 acceptance: queue space that exists is never taken from a
/// distinct document — only same-key work is superseded.
#[tokio::test]
async fn distinct_documents_never_superseded() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "queuedocs";
    let service = ClayJsRuntimeService::default();
    let (_, registration) = module_backed_completion_provider(
        &service,
        "@vendor/queuedocs",
        prefix,
        PLAN127_HOLDING_PROVIDER,
    )
    .await;
    let documents: Vec<u64> = (0..8).map(|index| 100 + index).collect();
    let outcomes = plan127_completion_burst(&service, &registration, prefix, &documents).await;
    // Synthetic `@vendor/...` packages are adopted third-party packages, so
    // their module-backed provider runs on the third-party latency lane.
    let stats = service.lane_queue_stats(
        crate::packages::bundled::RuntimeDomain::ThirdParty,
        RuntimeLane::Latency,
    );
    assert_eq!(
        stats.superseded, 0,
        "distinct documents must never supersede each other: {stats:?}"
    );
    assert_eq!(stats.evicted, 0, "queue space existed: {stats:?}");
    assert!(
        stats.peak_pending >= documents.len() - 2,
        "distinct documents must actually queue behind the busy lane: {stats:?}"
    );
    assert_eq!(
        plan127_ok_labels(&outcomes).len(),
        documents.len(),
        "every distinct document must be served: {outcomes:?}"
    );
    println!(
        "PLAN127_QUEUE_DISTINCT documents={} peak_pending={} capacity={} superseded={} evicted={}",
        documents.len(),
        stats.peak_pending,
        stats.capacity,
        stats.superseded,
        stats.evicted
    );
    let _ = fs::remove_dir_all(config_fixture("lane-module-provider"));
}

/// Plan 127 P2 acceptance: past the supersedable capacity the mailbox stays
/// bounded by evicting the oldest undelivered supersedable work (stale-first),
/// never by queueing without limit and never by rejecting the newest request.
#[tokio::test]
async fn queue_evicts_oldest_at_capacity() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let prefix = "queuefull";
    let capacity = crate::perf::budgets::JS_RUNTIME_SUPERSEDABLE_QUEUE_CAPACITY;
    let service = ClayJsRuntimeService::default();
    let (_, registration) = module_backed_completion_provider(
        &service,
        "@vendor/queuefull",
        prefix,
        PLAN127_HOLDING_PROVIDER,
    )
    .await;
    let documents: Vec<u64> = (0..capacity as u64 + 8).map(|index| 200 + index).collect();
    let outcomes = plan127_completion_burst(&service, &registration, prefix, &documents).await;
    // Synthetic `@vendor/...` packages are adopted third-party packages, so
    // their module-backed provider runs on the third-party latency lane.
    let stats = service.lane_queue_stats(
        crate::packages::bundled::RuntimeDomain::ThirdParty,
        RuntimeLane::Latency,
    );
    assert!(
        stats.peak_pending <= capacity,
        "mailbox must stay bounded by its budget: {stats:?}"
    );
    assert_eq!(
        stats.superseded, 0,
        "distinct documents do not supersede: {stats:?}"
    );
    assert!(
        stats.evicted >= documents.len() as u64 - capacity as u64 - 3,
        "overflow must drop oldest supersedable work: {stats:?}"
    );
    assert_eq!(
        plan127_superseded_count(&outcomes),
        stats.evicted,
        "evicted callers answer Superseded"
    );
    assert_eq!(
        plan127_ok_labels(&outcomes).len() as u64,
        documents.len() as u64 - stats.evicted,
        "every request is served or evicted"
    );
    let labels = plan127_ok_labels(&outcomes);
    assert!(
        labels.contains(&format!("q{}", documents.len())),
        "the newest request must survive capacity pressure: {labels:?}"
    );
    println!(
        "PLAN127_QUEUE_CAPACITY requests={} peak_pending={} capacity={} superseded={} evicted={} served={}",
        documents.len(),
        stats.peak_pending,
        stats.capacity,
        stats.superseded,
        stats.evicted,
        labels.len()
    );
    let _ = fs::remove_dir_all(config_fixture("lane-module-provider"));
}

/// Plan 132: lane channel capacities are transport contract, not an
/// implementation detail — a lane that silently grows its buffer changes
/// which overflow a slow connection sees. 16 for the advisory editor-command
/// lane, 4 for each state lane.
#[tokio::test]
async fn lane_channel_capacities_are_preserved() {
    let _runtime_guard = crate::server::JS_RUNTIME_TEST_LOCK.lock().await;
    let service = ClayJsRuntimeService::default();

    let mut commands = service.subscribe_editor_commands();
    for index in 0..17 {
        service
            .editor_commands
            .publish(crate::protocol::EditorCommandRequest {
                command_id: format!("editor.fixture{index}"),
                package_prefix: "fixture".to_string(),
                mode_id: "clay.default".to_string(),
            });
    }
    assert!(
        matches!(
            commands.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(1))
        ),
        "editor-command lane buffers exactly 16"
    );

    let mut carets = service.subscribe_caret_styles();
    let mut layouts = service.subscribe_editor_layout();
    let mut preferences = service.subscribe_shell_preferences();
    for _ in 0..5 {
        service.caret_styles.publish(None);
        service.editor_layouts.publish(None);
        service
            .shell_preferences
            .publish(crate::protocol::ShellPreferences {
                pane_focus_policy: "click".to_string(),
            });
    }
    // Five publishes into a capacity-4 lane drop exactly one message; a lane
    // that grew its buffer would not report `Lagged` at all.
    assert!(matches!(
        carets.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(1))
    ));
    assert!(matches!(
        layouts.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(1))
    ));
    assert!(matches!(
        preferences.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(1))
    ));
}
